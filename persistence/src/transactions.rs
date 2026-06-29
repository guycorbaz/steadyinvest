//! Transaction ledger — the **recorded-sell** slice (Story 4.7, FR46/FR47).
//!
//! The `transactions` table was frozen in the v1 DDL (Story 1.10) with FR39's fields
//! (`occurred_at`, `quantity`, `unit_price`, `fees`, `currency`); the v4 migration (Story 4.7) adds
//! a `kind` discriminator and an optional `rationale`. Story 4.7 writes **one SELL row** when the
//! user chooses to sell on a neutral trigger — it does **not** build the full buy/sell ledger
//! (partial sells, weighted-average cost basis, edit/delete): that is **Epic 6 / Story 6.3**.
//!
//! A sell is an **event**, not an idempotent upsert — [`Journal::record_sell`] always inserts and
//! bumps `journal_meta.logical_version` (NFR-R2). Decimals are the canonical TEXT spellings the app
//! validated (never REAL — NFR-C1). Ids/timestamps come from the app's injected `IdGen`/`Clock`
//! (ADD15).

use crate::error::Result;
use crate::journal::Journal;
use steadyinvest_contract::Timestamp;
use uuid::Uuid;

/// The `kind` value of a recorded sell (Story 4.7). Buys are Epic 6.
pub const KIND_SELL: &str = "sell";

/// One recorded transaction row. Story 4.7 only writes/read-backs sells; the decimal fields are the
/// canonical TEXT spellings. `rationale` is the optional free-text reason (`None` when blank).
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Record one **SELL** transaction (Story 4.7, FR46/FR47): the user chose to sell on a neutral
    /// trigger. Inserts a `kind = "sell"` row carrying the holding's quantity, the sale `unit_price`,
    /// `fees` (0 in Epic 4 — the fees workflow is Epic 6), the reference `currency` (FR63), and an
    /// optional `rationale`. Always a write (a sale is an event) → bumps the logical version. The
    /// caller is expected to then remove the holding from the active register (`delete_holding`).
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
        tx.execute(
            "UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1",
            [],
        )?;
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

fn parse_uuid(text: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(text).map_err(|e| crate::error::Error::CorruptPayload {
        detail: format!("{field} {text:?} is not a valid UUID: {e}"),
    })
}
