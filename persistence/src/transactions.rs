//! Transaction ledger — the **FR39 buy/sell ledger** (Story 6.3; the 4.7 recorded-sell slice
//! generalized).
//!
//! The `transactions` table was frozen in the v1 DDL (Story 1.10) with FR39's fields
//! (`occurred_at`, `quantity`, `unit_price`, `fees`, `currency`); the v4 migration (Story 4.7)
//! added a `kind` discriminator and an optional `rationale`. Story 6.3 makes it the full ledger:
//! buy rows ([`KIND_BUY`]), **partial sells**, edit/delete — MIGRATION-FREE (`user_version` stays
//! 6). [`Journal::record_sell`] stays as the historical **whole-position** sell path (Story 4.7,
//! FR46/FR47) with its `occurred_at = now` behavior; the 6.3 writers take a caller-normalized
//! `occurred_at` (FR39's "date", distinct from the row's `created_at` clock stamp).
//!
//! Every 6.3 writer is an **atomic compound write** (the 4.7 lesson): the ledger row AND the
//! caller-computed holding aggregate (`holdings.quantity`/`purchase_price` — the materialized
//! weighted-average, AC2) land in ONE transaction with exactly one `logical_version` bump
//! (NFR-R2); a no-op writes nothing and bumps nothing (Epic-3 C4). Persistence performs **no
//! arithmetic** — the app replays the ledger through `core::risk::ledger` and passes the
//! recomputed aggregates as canonical TEXT (never REAL — NFR-C1), keeping this layer
//! calc-agnostic (the 6.2 currency-agnostic parallel). Ids/timestamps come from the app's
//! injected `IdGen`/`Clock` (ADD15).

use crate::error::Result;
use crate::journal::Journal;
use crate::util::{bump_logical_version, parse_uuid};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use steadyinvest_contract::Timestamp;
use uuid::Uuid;

/// The `kind` value of a recorded sell (Story 4.7). Since Story 6.3 also written by
/// [`Journal::record_partial_sell`].
pub const KIND_SELL: &str = "sell";

/// The `kind` value of a buy row (Story 6.3, FR39) — including the materialized opening position of
/// a pre-6.3 holding (AC5).
pub const KIND_BUY: &str = "buy";

/// One ledger row to write (Story 6.3, FR39) — all decimals are the canonical TEXT spellings the
/// app validated; `occurred_at` is the caller-normalized RFC3339 event date (FR39's "date",
/// distinct from the row's `created_at` clock stamp).
pub struct LedgerEntry<'a> {
    pub id: Uuid,
    pub occurred_at: &'a str,
    pub quantity: &'a str,
    pub unit_price: &'a str,
    pub fees: &'a str,
    pub currency: &'a str,
    pub rationale: Option<&'a str>,
}

impl LedgerEntry<'_> {
    /// The read-back [`TransactionItem`] for this entry as just inserted (avoids a re-SELECT).
    fn to_item(&self, holding_id: Uuid, kind: &str, now: &Timestamp) -> TransactionItem {
        TransactionItem {
            id: self.id,
            holding_id,
            occurred_at: Timestamp(self.occurred_at.to_string()),
            quantity: self.quantity.to_string(),
            unit_price: self.unit_price.to_string(),
            fees: self.fees.to_string(),
            currency: self.currency.to_string(),
            kind: Some(kind.to_string()),
            rationale: self.rationale.map(str::to_string),
            created_at: now.clone(),
        }
    }
}

/// Whether a `transactions` row with this id exists AND belongs to `holding_id` — the edit/delete
/// pre-check that keeps an absent id a true no-op (nothing written, no bump) before any
/// opening-row materialization. The ownership half (2026-07-02 review, HIGH): these writers
/// rewrite the **caller-supplied** holding's aggregate, so a row id paired with the wrong holding
/// must be the same typed no-op (`Ok(false)`) — never an edit of holding A's row combined with a
/// rewrite of holding B's aggregate (silent, versioned corruption).
fn transaction_belongs(tx: &rusqlite::Transaction<'_>, id: Uuid, holding_id: Uuid) -> Result<bool> {
    let hit: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM transactions WHERE id = ?1 AND holding_id = ?2",
            rusqlite::params![id.to_string(), holding_id.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    Ok(hit.is_some())
}

/// INSERT one ledger row inside the caller's transaction — shared by every Story-6.3 compound
/// writer (the opening-position materialization included). `created_at = now` (the clock stamp);
/// `occurred_at` comes from the entry (the FR39 event date).
fn insert_ledger_row(
    tx: &rusqlite::Transaction<'_>,
    holding_id: Uuid,
    entry: &LedgerEntry,
    kind: &str,
    now: &Timestamp,
) -> Result<()> {
    tx.execute(
        "INSERT INTO transactions
             (id, holding_id, occurred_at, quantity, unit_price, fees, currency,
              kind, rationale, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            entry.id.to_string(),
            holding_id.to_string(),
            entry.occurred_at,
            entry.quantity,
            entry.unit_price,
            entry.fees,
            entry.currency,
            kind,
            entry.rationale,
            now.0,
        ],
    )?;
    Ok(())
}

/// One recorded transaction row — a sell (Story 4.7) or, since Story 6.3, a buy (FR39, incl. the
/// materialized opening position). The decimal fields are the canonical TEXT spellings. `kind` is
/// `Option` because the column is v4-nullable — no writer ever produced NULL, but a replay treats
/// it as a sell defensively (the only pre-6.3 writer sold). `rationale` is the optional free-text
/// reason (`None` when blank). `Serialize`/`Deserialize` so the whole-journal export (Story 5.3)
/// carries it verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionItem {
    pub id: Uuid,
    pub holding_id: Uuid,
    pub occurred_at: Timestamp,
    pub quantity: String,
    pub unit_price: String,
    pub fees: String,
    pub currency: String,
    pub kind: Option<String>,
    pub rationale: Option<String>,
    pub created_at: Timestamp,
}

impl Journal {
    /// Record one **SELL** and retire its holding **atomically** (Story 4.7, FR46/FR47): the user
    /// chose to sell on a neutral trigger. In a **single transaction**, inserts a `kind = "sell"` row
    /// (the holding's quantity, the sale `unit_price`, `fees` = 0 in Epic 4, the reference `currency`
    /// FR63, an optional `rationale`) **and** soft-deletes the holding (stamps `holdings.sold_at`, so
    /// it leaves the active register while staying a live FK referent for the sell row), then bumps the
    /// logical version once. One transaction is essential: a separate INSERT-then-mark would leave a
    /// committed sell row with the holding still active (re-sellable) if the second write failed —
    /// especially on a sync-sensitive store. The full ledger (partial sells, cost basis) is Epic 6.
    #[allow(clippy::too_many_arguments)]
    pub fn record_sell(
        &mut self,
        id: Uuid,
        holding_id: Uuid,
        quantity: &str,
        unit_price: &str,
        fees: &str,
        currency: &str,
        rationale: Option<&str>,
        now: &Timestamp,
    ) -> Result<TransactionItem> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO transactions
                 (id, holding_id, occurred_at, quantity, unit_price, fees, currency,
                  kind, rationale, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?3)",
            rusqlite::params![
                id.to_string(),
                holding_id.to_string(),
                now.0,
                quantity,
                unit_price,
                fees,
                currency,
                KIND_SELL,
                rationale,
            ],
        )?;
        // Soft-delete the holding in the SAME transaction (atomic with the sell row). `sold_at IS
        // NULL` guards a re-sell: a holding already retired is not stamped twice.
        tx.execute(
            "UPDATE holdings SET sold_at = ?2 WHERE id = ?1 AND sold_at IS NULL",
            rusqlite::params![holding_id.to_string(), now.0],
        )?;
        bump_logical_version(&tx)?;
        tx.commit()?;
        Ok(TransactionItem {
            id,
            holding_id,
            occurred_at: now.clone(),
            quantity: quantity.to_string(),
            unit_price: unit_price.to_string(),
            fees: fees.to_string(),
            currency: currency.to_string(),
            kind: Some(KIND_SELL.to_string()),
            rationale: rationale.map(str::to_string),
            created_at: now.clone(),
        })
    }

    /// Record one **BUY** and land the caller-computed holding aggregate **atomically** (Story 6.3,
    /// FR39). In a single transaction: (a) when `opening` is `Some`, first materializes the opening
    /// position of a pre-6.3 holding as a `kind = "buy"` row (AC5 — dated `holdings.created_at` by
    /// the caller, so the ledger is self-contained and auditable from its first 6.3 mutation on);
    /// (b) inserts `entry` as a `kind = "buy"` row; (c) writes the recomputed weighted-average
    /// aggregate (`new_quantity`/`new_avg_cost`, fees included — Appendix A, computed by the app via
    /// `core::risk::ledger`, never here); then bumps the logical version once. Returns the inserted
    /// [`TransactionItem`] for `entry`.
    ///
    /// `sold_at` is deliberately untouched: buying on a RETIRED holding is refused at the app
    /// gate (`state::ledger::record_buy_for` — re-entering is a new position), so this writer
    /// never has to arbitrate a resurrection it cannot validate.
    pub fn record_buy(
        &mut self,
        holding_id: Uuid,
        opening: Option<&LedgerEntry>,
        entry: &LedgerEntry,
        new_quantity: &str,
        new_avg_cost: &str,
        now: &Timestamp,
    ) -> Result<TransactionItem> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        if let Some(open) = opening {
            insert_ledger_row(&tx, holding_id, open, KIND_BUY, now)?;
        }
        insert_ledger_row(&tx, holding_id, entry, KIND_BUY, now)?;
        tx.execute(
            "UPDATE holdings SET quantity = ?2, purchase_price = ?3 WHERE id = ?1",
            rusqlite::params![holding_id.to_string(), new_quantity, new_avg_cost],
        )?;
        bump_logical_version(&tx)?;
        tx.commit()?;
        Ok(entry.to_item(holding_id, KIND_BUY, now))
    }

    /// Record one **SELL of part (or all) of a position** and land the caller-computed remaining
    /// quantity **atomically** (Story 6.3, FR39). In a single transaction: the optional `opening`
    /// materialization (AC5, a `kind = "buy"` row), the `kind = "sell"` row for `entry`, then the
    /// holding update — when `remaining_quantity == "0"` the position empties and the holding is
    /// **retired** (the 4.7 `sold_at` stamp; quantity is also written so the aggregate stays
    /// truthful), otherwise the reduced quantity is written and `sold_at` is cleared (a partial sell
    /// keeps/returns the holding active). `purchase_price` is **not** touched: a sell never
    /// re-averages the weighted-average cost (Appendix A). One version bump. [`Journal::record_sell`]
    /// stays as the 4.7 whole-position path.
    ///
    /// PRECONDITION (calc-agnostic layer, no parsing here): `remaining_quantity` must be the
    /// **canonical** decimal spelling — an empty position is exactly `"0"` (the app normalizes via
    /// `Decimal::normalize`); `"0.0"`/`"0.00"` would skip the retire branch.
    pub fn record_partial_sell(
        &mut self,
        holding_id: Uuid,
        opening: Option<&LedgerEntry>,
        entry: &LedgerEntry,
        remaining_quantity: &str,
        now: &Timestamp,
    ) -> Result<TransactionItem> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        if let Some(open) = opening {
            insert_ledger_row(&tx, holding_id, open, KIND_BUY, now)?;
        }
        insert_ledger_row(&tx, holding_id, entry, KIND_SELL, now)?;
        if remaining_quantity == "0" {
            // The 4.7 retire semantics: `sold_at IS NULL` guards a double stamp on a re-sell.
            tx.execute(
                "UPDATE holdings SET quantity = ?2, sold_at = ?3 WHERE id = ?1 AND sold_at IS NULL",
                rusqlite::params![holding_id.to_string(), remaining_quantity, now.0],
            )?;
        } else {
            tx.execute(
                "UPDATE holdings SET quantity = ?2, sold_at = NULL WHERE id = ?1",
                rusqlite::params![holding_id.to_string(), remaining_quantity],
            )?;
        }
        bump_logical_version(&tx)?;
        tx.commit()?;
        Ok(entry.to_item(holding_id, KIND_SELL, now))
    }

    /// Edit a ledger row's `occurred_at`/`quantity`/`unit_price`/`fees`/`rationale` and rewrite the
    /// recomputed holding aggregate **atomically** (Story 6.3, FR39). `kind` and `currency` are NOT
    /// editable: `kind` is the row's identity (a buy edited into a sell would be a different event,
    /// not a correction), and `currency` is pinned to the holding's own — no mixed-currency ledger
    /// rows, no FX (FR28; FX is Story 6.5).
    ///
    /// An absent `id` returns `Ok(false)` with nothing written (checked BEFORE the optional
    /// `opening` insert, so a stale edit never materializes an opening row). Otherwise, in one
    /// transaction: the optional `opening` materialization (AC5), the guarded row UPDATE, the
    /// guarded holding UPDATE (`quantity`/`purchase_price`/`sold_at = retired_at` — `None` clears
    /// the retire stamp, re-activating the holding when the recomputed remaining quantity is
    /// positive). An identical-values edit is a true no-op (`IS NOT` guards: no write, no bump —
    /// Epic-3 C4); the version bumps exactly once when ANY executed statement changed a row (the
    /// opening insert counts).
    #[allow(clippy::too_many_arguments)]
    pub fn update_transaction(
        &mut self,
        id: Uuid,
        holding_id: Uuid,
        opening: Option<&LedgerEntry>,
        occurred_at: &str,
        quantity: &str,
        unit_price: &str,
        fees: &str,
        rationale: Option<&str>,
        new_quantity: &str,
        new_avg_cost: &str,
        retired_at: Option<&str>,
        now: &Timestamp,
    ) -> Result<bool> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        // Pre-check existence FIRST: an absent id must be a TRUE no-op (nothing written, no bump)
        // even when the caller passed an `opening` — a stale edit never materializes a row.
        if !transaction_belongs(&tx, id, holding_id)? {
            return Ok(false);
        }
        let mut changed = false;
        if let Some(open) = opening {
            insert_ledger_row(&tx, holding_id, open, KIND_BUY, now)?;
            changed = true;
        }
        // NULL-safe `IS NOT` guards keep an identical-values edit a true no-op (Epic-3 C4).
        let row_changed = tx.execute(
            "UPDATE transactions
                SET occurred_at = ?2, quantity = ?3, unit_price = ?4, fees = ?5, rationale = ?6
              WHERE id = ?1
                AND (occurred_at IS NOT ?2 OR quantity IS NOT ?3 OR unit_price IS NOT ?4
                     OR fees IS NOT ?5 OR rationale IS NOT ?6)",
            rusqlite::params![
                id.to_string(),
                occurred_at,
                quantity,
                unit_price,
                fees,
                rationale
            ],
        )?;
        let holding_changed = tx.execute(
            "UPDATE holdings SET quantity = ?2, purchase_price = ?3, sold_at = ?4
              WHERE id = ?1
                AND (quantity IS NOT ?2 OR purchase_price IS NOT ?3 OR sold_at IS NOT ?4)",
            rusqlite::params![
                holding_id.to_string(),
                new_quantity,
                new_avg_cost,
                retired_at
            ],
        )?;
        if changed || row_changed > 0 || holding_changed > 0 {
            bump_logical_version(&tx)?;
        }
        tx.commit()?;
        Ok(true)
    }

    /// Delete a ledger row and rewrite the recomputed holding aggregate **atomically** (Story 6.3,
    /// FR39). An absent `id` returns `Ok(false)` with nothing written — checked BEFORE the optional
    /// `opening` insert, so deleting a stale row never materializes an opening row. Otherwise, in
    /// one transaction: the optional `opening` materialization (AC5), the DELETE, the guarded
    /// holding UPDATE (`quantity`/`purchase_price`/`sold_at = retired_at`). Deleting the sell that
    /// retired a holding is expressed by the caller passing `retired_at = None` plus the restored
    /// quantity — the holding **un-retires** (`sold_at` back to NULL, back in the active register).
    /// One version bump per applied deletion.
    #[allow(clippy::too_many_arguments)]
    pub fn delete_transaction(
        &mut self,
        id: Uuid,
        holding_id: Uuid,
        opening: Option<&LedgerEntry>,
        new_quantity: &str,
        new_avg_cost: &str,
        retired_at: Option<&str>,
        now: &Timestamp,
    ) -> Result<bool> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        // Pre-check existence AND ownership FIRST (unconditionally — 2026-07-02 review, HIGH): an
        // absent id, or a row belonging to another holding, must be a TRUE no-op (nothing written,
        // no bump, no opening materialized) — never a deletion of holding A's row combined with a
        // rewrite of holding B's aggregate.
        if !transaction_belongs(&tx, id, holding_id)? {
            return Ok(false);
        }
        if let Some(open) = opening {
            insert_ledger_row(&tx, holding_id, open, KIND_BUY, now)?;
        }
        // Ownership re-asserted in the DELETE itself (belt and braces with the pre-check).
        tx.execute(
            "DELETE FROM transactions WHERE id = ?1 AND holding_id = ?2",
            rusqlite::params![id.to_string(), holding_id.to_string()],
        )?;
        tx.execute(
            "UPDATE holdings SET quantity = ?2, purchase_price = ?3, sold_at = ?4
              WHERE id = ?1
                AND (quantity IS NOT ?2 OR purchase_price IS NOT ?3 OR sold_at IS NOT ?4)",
            rusqlite::params![
                holding_id.to_string(),
                new_quantity,
                new_avg_cost,
                retired_at
            ],
        )?;
        bump_logical_version(&tx)?;
        tx.commit()?;
        Ok(true)
    }

    /// Every transaction in the journal, oldest first (deterministic: `occurred_at` then `id`) —
    /// across all holdings. The whole-journal export (Story 5.3) reads the complete ledger; per-holding
    /// reads use [`Self::list_transactions`].
    pub fn list_all_transactions(&self) -> Result<Vec<TransactionItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, holding_id, occurred_at, quantity, unit_price, fees, currency,
                    kind, rationale, created_at
             FROM transactions ORDER BY occurred_at, id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, String>(9)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (
                id_text,
                holding_text,
                occurred,
                quantity,
                unit_price,
                fees,
                currency,
                kind,
                rationale,
                created,
            ) = row?;
            out.push(TransactionItem {
                id: parse_uuid(&id_text, "transactions.id")?,
                holding_id: parse_uuid(&holding_text, "transactions.holding_id")?,
                occurred_at: Timestamp(occurred),
                quantity,
                unit_price,
                fees,
                currency,
                kind,
                rationale,
                created_at: Timestamp(created),
            });
        }
        Ok(out)
    }

    /// Every transaction recorded against a holding, oldest first (deterministic: `occurred_at` then
    /// `id`). Story 4.7 uses it to read back recorded sells (tests / a later ledger view).
    pub fn list_transactions(&self, holding_id: Uuid) -> Result<Vec<TransactionItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, holding_id, occurred_at, quantity, unit_price, fees, currency,
                    kind, rationale, created_at
             FROM transactions WHERE holding_id = ?1 ORDER BY occurred_at, id",
        )?;
        let rows = stmt.query_map(rusqlite::params![holding_id.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, String>(9)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (
                id_text,
                holding_text,
                occurred,
                quantity,
                unit_price,
                fees,
                currency,
                kind,
                rationale,
                created,
            ) = row?;
            out.push(TransactionItem {
                id: parse_uuid(&id_text, "transactions.id")?,
                holding_id: parse_uuid(&holding_text, "transactions.holding_id")?,
                occurred_at: Timestamp(occurred),
                quantity,
                unit_price,
                fees,
                currency,
                kind,
                rationale,
                created_at: Timestamp(created),
            });
        }
        Ok(out)
    }
}
