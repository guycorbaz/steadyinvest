//! Holdings and portfolio storage (Stories 4.3/6.1/6.2, FR36/FR37/FR38) — the normalized side of
//! the hybrid model.
//!
//! A holding is **typed columns**, not a serde blob: `id`, a `portfolio_id` FK to its portfolio,
//! `security_ticker`, `quantity` and `purchase_price` (exact decimals carried as TEXT, never REAL —
//! NFR-C1), an optional `currency` (Story 6.2, v6 column — NULL on pre-6.2 rows), the trailing-stop
//! pair (`trailing_stop_pct`/`trailing_stop_level`, Story 4.5), the `sold_at` soft-delete marker
//! (Story 4.7), and `created_at`. The `holdings`/`portfolios` DDL was frozen in v1 (Story 1.10);
//! Story 6.2 added the single nullable `currency` column via migration v5→v6.
//!
//! Multi-portfolio (FR37, Story 6.1): one portfolio per bank/account. [`Journal::add_portfolio`] /
//! [`Journal::rename_portfolio`] / [`Journal::delete_portfolio`] manage the list ([`Journal::delete_portfolio`]
//! refuses a portfolio with any holding — even a sold one — and refuses to delete the last one);
//! [`Journal::ensure_portfolio`] keeps its 4.3 role of lazily seeding the first portfolio.
//!
//! Like the rest of the journal, **every mutating call runs in one transaction that also bumps
//! `journal_meta.logical_version`** (NFR-R2) — and a no-op (an edit to identical values, a delete
//! of an absent id, `ensure_portfolio` when the row exists) bumps **nothing** (the Epic-3
//! idempotency lesson: avoid phantom journal revisions on a sync-sensitive store). Ids/timestamps
//! come from the app's injected `IdGen`/`Clock` (ADD15); persistence sources no identity.

use crate::error::{Error, Result};
use crate::journal::Journal;
use crate::util::{bump_logical_version, parse_uuid};
use serde::{Deserialize, Serialize};
use steadyinvest_contract::Timestamp;
use uuid::Uuid;

/// One portfolio (FR36/FR37) — since Story 6.1 the journal holds one per bank/account.
/// `Serialize`/`Deserialize` so the whole-journal export (Story 5.3) carries it verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortfolioItem {
    pub id: Uuid,
    pub name: String,
    pub created_at: Timestamp,
}

/// One holding row (FR36): a security, a quantity and a purchase price in the holding's own currency.
/// `quantity`/`purchase_price` are the canonical decimal **TEXT** spellings (parsed and validated by
/// the app before they reach here). Since Story 6.3 (FR39) a ledger-backed holding's pair is the
/// **materialized aggregate** of its transaction ledger — `purchase_price` then means
/// "weighted-average cost, fees included" (Appendix A), rewritten atomically with every ledger
/// mutation (never derived here: the app computes it via `core::risk::ledger`).
/// `trailing_stop_pct` is Story 4.5's (NULL here).
/// `Serialize`/`Deserialize` so the whole-journal export (Story 5.3) carries it verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HoldingItem {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub security_ticker: String,
    pub quantity: String,
    pub purchase_price: String,
    /// The currency the amounts are denominated in (Story 6.2, FR38) — e.g. `"CHF"`, `"EUR"`. `None`
    /// on a **pre-6.2 holding** (the v6 `ADD COLUMN` left legacy rows NULL); the persistence layer
    /// stays currency-agnostic, so the app coalesces `None` to the reference currency at read. New
    /// writes always carry an explicit currency. Amounts are never mixed/converted (FR28).
    #[serde(default)]
    pub currency: Option<String>,
    pub trailing_stop_pct: Option<String>,
    /// The ratcheted trailing-stop **level** (a price, Story 4.5 / FR42) — `None` when no stop set.
    /// Persisted (v3 column) because the ratchet's high-water mark can't be re-derived from the
    /// latest price alone. The app computes it via `core::risk::ratchet_trailing_stop`.
    pub trailing_stop_level: Option<String>,
    /// The soft-delete marker (Story 4.7 / FR47): `Some(timestamp)` when the holding was sold and
    /// retired from the active register, `None` when still held. The active-register reads
    /// ([`Journal::list_holdings`]) filter it out, but the whole-journal export (Story 5.3) must carry
    /// it — a sold holding stays a live FK referent for its sell transaction.
    pub sold_at: Option<String>,
    pub created_at: Timestamp,
}

impl Journal {
    /// Ensure the single portfolio exists (FR36). **Idempotent**: if any portfolio row is already
    /// present, returns it and writes nothing (no insert, no version bump — the idempotency
    /// lesson). Otherwise inserts the given singleton and bumps the logical version. Single-portfolio
    /// by construction: callers pass one stable id/name and never create a second.
    pub fn ensure_portfolio(
        &mut self,
        id: Uuid,
        name: &str,
        created_at: &Timestamp,
    ) -> Result<PortfolioItem> {
        self.check_writable()?;
        if let Some(existing) = self.first_portfolio()? {
            return Ok(existing);
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO portfolios (id, name, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![id.to_string(), name, created_at.0],
        )?;
        bump_logical_version(&tx)?;
        tx.commit()?;
        Ok(PortfolioItem {
            id,
            name: name.to_string(),
            created_at: created_at.clone(),
        })
    }

    /// The single portfolio if one exists (the lowest `id` for determinism, though there is one).
    pub fn first_portfolio(&self) -> Result<Option<PortfolioItem>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, created_at FROM portfolios ORDER BY id LIMIT 1")?;
        let mut rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        match rows.next() {
            Some(row) => {
                let (id_text, name, created_at) = row?;
                Ok(Some(PortfolioItem {
                    id: parse_uuid(&id_text, "portfolios.id")?,
                    name,
                    created_at: Timestamp(created_at),
                }))
            }
            None => Ok(None),
        }
    }

    /// Add a holding to a portfolio (FR36) in its own `currency` (Story 6.2, FR38 — e.g. `"CHF"`).
    /// `trailing_stop_pct` is left NULL (Story 4.5). Bumps the logical version. Returns the inserted
    /// [`HoldingItem`]. The currency is stored verbatim (the app validates it against its allow-list
    /// before it reaches here); amounts stay native — no FX conversion (FR28).
    #[allow(clippy::too_many_arguments)]
    pub fn add_holding(
        &mut self,
        id: Uuid,
        portfolio_id: Uuid,
        security_ticker: &str,
        quantity: &str,
        purchase_price: &str,
        currency: &str,
        created_at: &Timestamp,
    ) -> Result<HoldingItem> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO holdings
                 (id, portfolio_id, security_ticker, quantity, purchase_price,
                  currency, trailing_stop_pct, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)",
            rusqlite::params![
                id.to_string(),
                portfolio_id.to_string(),
                security_ticker,
                quantity,
                purchase_price,
                currency,
                created_at.0,
            ],
        )?;
        bump_logical_version(&tx)?;
        tx.commit()?;
        Ok(HoldingItem {
            id,
            portfolio_id,
            security_ticker: security_ticker.to_string(),
            quantity: quantity.to_string(),
            purchase_price: purchase_price.to_string(),
            currency: Some(currency.to_string()),
            trailing_stop_pct: None,
            trailing_stop_level: None,
            sold_at: None,
            created_at: created_at.clone(),
        })
    }

    /// Every portfolio row, ordered by `id` (deterministic). Single-portfolio in v1 (FR36), but the
    /// whole-journal export (Story 5.3) reads them generically.
    pub fn list_portfolios(&self) -> Result<Vec<PortfolioItem>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, created_at FROM portfolios ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id_text, name, created_at) = row?;
            out.push(PortfolioItem {
                id: parse_uuid(&id_text, "portfolios.id")?,
                name,
                created_at: Timestamp(created_at),
            });
        }
        Ok(out)
    }

    /// Every holding in the journal **including soft-deleted (sold) ones**, ordered by `created_at`
    /// then `id` (deterministic). Unlike [`Self::list_holdings`] (the active register, which filters
    /// `sold_at IS NULL`), this is the **complete** read the whole-journal export needs (Story 5.3): a
    /// sold holding must be carried so its sell transaction keeps a live FK referent on import.
    pub fn list_all_holdings(&self) -> Result<Vec<HoldingItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, portfolio_id, security_ticker, quantity, purchase_price,
                    currency, trailing_stop_pct, trailing_stop_level, sold_at, created_at
             FROM holdings ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, String>(9)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (
                id_text,
                portfolio_text,
                security_ticker,
                quantity,
                purchase_price,
                currency,
                stop_pct,
                stop_level,
                sold_at,
                created,
            ) = row?;
            out.push(HoldingItem {
                id: parse_uuid(&id_text, "holdings.id")?,
                portfolio_id: parse_uuid(&portfolio_text, "holdings.portfolio_id")?,
                security_ticker,
                quantity,
                purchase_price,
                currency,
                trailing_stop_pct: stop_pct,
                trailing_stop_level: stop_level,
                sold_at,
                created_at: Timestamp(created),
            });
        }
        Ok(out)
    }

    /// Every **active** holding in a portfolio, ordered by `created_at` then `id` (deterministic) —
    /// the register's list. (No `position` column: holdings are not user-reordered in 4.3.) A holding
    /// sold on a neutral trigger (Story 4.7) has a non-NULL `sold_at` and is **excluded** here — it
    /// stays in the table so its sell transaction's FK keeps a live referent, but it leaves the
    /// active register (and the capital-at-risk source, which reads this list).
    pub fn list_holdings(&self, portfolio_id: Uuid) -> Result<Vec<HoldingItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, portfolio_id, security_ticker, quantity, purchase_price,
                    currency, trailing_stop_pct, trailing_stop_level, created_at
             FROM holdings WHERE portfolio_id = ?1 AND sold_at IS NULL ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map(rusqlite::params![portfolio_id.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, String>(8)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (
                id_text,
                portfolio_text,
                security_ticker,
                quantity,
                purchase_price,
                currency,
                stop_pct,
                stop_level,
                created,
            ) = row?;
            out.push(HoldingItem {
                id: parse_uuid(&id_text, "holdings.id")?,
                portfolio_id: parse_uuid(&portfolio_text, "holdings.portfolio_id")?,
                security_ticker,
                quantity,
                purchase_price,
                currency,
                trailing_stop_pct: stop_pct,
                trailing_stop_level: stop_level,
                sold_at: None,
                created_at: Timestamp(created),
            });
        }
        Ok(out)
    }

    /// Edit a holding's ticker, quantity, purchase price and/or `currency` (FR36 / Story 6.2 FR38).
    /// Changing the **ticker** clears the trailing stop (Story 4.5 review): a stop level seeded from
    /// the OLD security would otherwise persist against the NEW one and — being ratchet-up-only —
    /// could show a permanent false breach. Editing only quantity/price/currency leaves the stop
    /// intact. The `currency` is always written (the app supplies the edit form's selection); a no-op
    /// (all values identical) writes nothing and bumps no version. An absent id is a no-op success.
    ///
    /// Caveat for a **pre-6.2 legacy row** (stored currency NULL): the app's edit form prefills the
    /// coalesced reference currency, so a visually unchanged edit **materializes** that currency
    /// (NULL `IS NOT` the code → a real write, one version bump). Accepted: it is a deliberate user
    /// action on the row, not a phantom read-time rewrite (the class the no-op guard exists for).
    pub fn update_holding(
        &mut self,
        id: Uuid,
        security_ticker: &str,
        quantity: &str,
        purchase_price: &str,
        currency: &str,
    ) -> Result<()> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        // The `CASE … security_ticker IS NOT ?2` reads the OLD ticker (SET exprs see pre-update row),
        // so the stop clears only when the ticker actually changes. `currency IS NOT ?5` in the WHERE
        // keeps an identical-values edit a true no-op even when only the currency would change.
        let changed = tx.execute(
            "UPDATE holdings SET security_ticker = ?2, quantity = ?3, purchase_price = ?4, currency = ?5,
                    trailing_stop_pct = CASE WHEN security_ticker IS NOT ?2 THEN NULL ELSE trailing_stop_pct END,
                    trailing_stop_level = CASE WHEN security_ticker IS NOT ?2 THEN NULL ELSE trailing_stop_level END
             WHERE id = ?1
               AND (security_ticker IS NOT ?2 OR quantity IS NOT ?3 OR purchase_price IS NOT ?4 OR currency IS NOT ?5)",
            rusqlite::params![id.to_string(), security_ticker, quantity, purchase_price, currency],
        )?;
        if changed > 0 {
            bump_logical_version(&tx)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Set (or clear) a holding's trailing-stop **parameter + ratcheted level** (Story 4.5, FR42).
    /// Both fields are written together — the app computes the ratcheted `level` from `pct` via
    /// `core::risk::ratchet_trailing_stop`; `None`/`None` clears the stop. A no-op (identical values,
    /// NULL-safe via `IS NOT`) writes nothing and bumps no version (C4 — avoidable writes are
    /// suppressed under Synology sync); an absent id is an idempotent no-op success.
    pub fn set_trailing_stop(
        &mut self,
        id: Uuid,
        pct: Option<&str>,
        level: Option<&str>,
    ) -> Result<()> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        let changed = tx.execute(
            "UPDATE holdings SET trailing_stop_pct = ?2, trailing_stop_level = ?3
             WHERE id = ?1
               AND (trailing_stop_pct IS NOT ?2 OR trailing_stop_level IS NOT ?3)",
            rusqlite::params![id.to_string(), pct, level],
        )?;
        if changed > 0 {
            bump_logical_version(&tx)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Remove a holding (FR36). One transaction; bumps the version only on a real removal (an
    /// absent id is an idempotent no-op). Refuses (typed [`Error::CorruptPayload`]-free, neutral
    /// [`Error::HoldingHasTransactions`]) while transaction rows still reference the holding —
    /// their FK would otherwise surface as a raw SQLite error (a sell soft-deletes instead, so
    /// this path only becomes reachable when Story 6.3 adds hard deletes/buy rows).
    pub fn delete_holding(&mut self, id: Uuid) -> Result<()> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        let referenced: i64 = tx.query_row(
            "SELECT COUNT(*) FROM transactions WHERE holding_id = ?1",
            rusqlite::params![id.to_string()],
            |r| r.get(0),
        )?;
        if referenced > 0 {
            return Err(Error::HoldingHasTransactions);
        }
        let removed = tx.execute(
            "DELETE FROM holdings WHERE id = ?1",
            rusqlite::params![id.to_string()],
        )?;
        if removed > 0 {
            bump_logical_version(&tx)?;
        }
        tx.commit()?;
        Ok(())
    }

    // ── Story 6.1 — multiple portfolios (FR37). The CRUD Story 4.3 deferred; the table + the
    // `holdings.portfolio_id` FK were frozen in v1, so this is typed CRUD, no migration. ──

    /// Add a named portfolio (FR37) — "one per bank/account". Inserts the row and bumps the logical
    /// version. (Unlike [`Self::ensure_portfolio`], which is the single-portfolio bootstrap, this
    /// always inserts — the caller mints a fresh id.)
    pub fn add_portfolio(
        &mut self,
        id: Uuid,
        name: &str,
        created_at: &Timestamp,
    ) -> Result<PortfolioItem> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO portfolios (id, name, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![id.to_string(), name, created_at.0],
        )?;
        bump_logical_version(&tx)?;
        tx.commit()?;
        Ok(PortfolioItem {
            id,
            name: name.to_string(),
            created_at: created_at.clone(),
        })
    }

    /// Rename a portfolio (FR37). Idempotent: a rename to the **identical** name (or an absent id)
    /// writes nothing and bumps **nothing** (the Epic-3 C4 no-op guard). Returns whether a row changed.
    pub fn rename_portfolio(&mut self, id: Uuid, name: &str) -> Result<bool> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        // `name = ?2 AND name <> ?2` via the WHERE clause makes an identical-name rename a true no-op.
        let changed = tx.execute(
            "UPDATE portfolios SET name = ?2 WHERE id = ?1 AND name <> ?2",
            rusqlite::params![id.to_string(), name],
        )?;
        if changed > 0 {
            bump_logical_version(&tx)?;
        }
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Delete a portfolio (FR37), **guarded** so the register can never be left inconsistent:
    /// - refuses while ANY holding row (active *or* sold) still references it — the `holdings.
    ///   portfolio_id` FK would otherwise orphan a holding (and its sell transactions), the 4.7
    ///   FK lesson read up front;
    /// - refuses deleting the **last** portfolio — the register always keeps at least one.
    ///
    /// Returns a typed business [`DeletePortfolioOutcome`] for the two refusals (no panic, no raw FK
    /// error surfaced); only [`DeletePortfolioOutcome::Deleted`] writes + bumps the version.
    pub fn delete_portfolio(&mut self, id: Uuid) -> Result<DeletePortfolioOutcome> {
        self.check_writable()?;
        // Both guards run INSIDE the delete's transaction: a second same-process handle (allowed by
        // the 5.5 lock re-entry) inserting a holding between a pre-check and the DELETE would
        // otherwise surface as a raw FK error instead of the typed refusal.
        let tx = self.conn.transaction()?;
        let holdings: i64 = tx.query_row(
            "SELECT COUNT(*) FROM holdings WHERE portfolio_id = ?1",
            rusqlite::params![id.to_string()],
            |r| r.get(0),
        )?;
        if holdings > 0 {
            return Ok(DeletePortfolioOutcome::HasHoldings);
        }
        let total: i64 = tx.query_row("SELECT COUNT(*) FROM portfolios", [], |r| r.get(0))?;
        if total <= 1 {
            return Ok(DeletePortfolioOutcome::LastPortfolio);
        }
        let removed = tx.execute(
            "DELETE FROM portfolios WHERE id = ?1",
            rusqlite::params![id.to_string()],
        )?;
        if removed > 0 {
            bump_logical_version(&tx)?;
            tx.commit()?;
            Ok(DeletePortfolioOutcome::Deleted)
        } else {
            // Absent id — a true no-op (no row, no bump).
            tx.commit()?;
            Ok(DeletePortfolioOutcome::Deleted)
        }
    }
}

/// The result of a guarded [`Journal::delete_portfolio`] — the deletion happened, or it was refused
/// for a named, neutral reason (never a panic, never an orphaned holding FK).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletePortfolioOutcome {
    /// The portfolio was removed (or the id was absent — a no-op deletion).
    Deleted,
    /// Refused: holdings (active or sold) still reference this portfolio.
    HasHoldings,
    /// Refused: it is the last portfolio; the register keeps at least one.
    LastPortfolio,
}
