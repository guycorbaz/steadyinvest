//! Portfolios + the holdings register (Stories 4.3/4.5/4.7/6.1/6.2 — FR36/FR37/FR38/FR42/FR46):
//! the active-portfolio rails (one portfolio per bank/account, guarded delete — never orphan a
//! holding, never drop the last portfolio), holding CRUD with exact-decimal validation (NFR-C1) and
//! the Story-6.2 currency allow-list, the recorded sell (one atomic SELL row + soft delete),
//! trailing stops (explicit re-seed + the up-only price-refresh ratchet), and the per-currency
//! capital-at-risk grouping (amounts stay native, never converted — FX is Story 6.5).

use rust_decimal::Decimal;
use steadyinvest_persistence::{DeletePortfolioOutcome, HoldingItem, PortfolioItem};
use uuid::Uuid;

use super::{
    JournalState, MSG_HOLDING_AMOUNT_OUT_OF_RANGE, MSG_HOLDING_INVALID_CURRENCY,
    MSG_HOLDING_INVALID_NUMBER, MSG_HOLDING_INVALID_STOP, MSG_HOLDING_INVALID_TICKER,
    MSG_LEDGER_BACKED, MSG_NO_JOURNAL, MSG_PORTFOLIO_HAS_HOLDINGS, MSG_PORTFOLIO_INVALID_NAME,
    MSG_PORTFOLIO_LAST, MSG_READ_ONLY_WRITE, watch_error,
};

impl JournalState {
    // ── Story 6.1 — multiple portfolios (FR37): the active-portfolio rails ──

    /// Every portfolio, ordered deterministically (Story 6.1). Empty when no journal / none yet.
    pub fn list_portfolios(&self) -> Vec<PortfolioItem> {
        self.journal
            .as_ref()
            .and_then(|j| j.list_portfolios().ok())
            .unwrap_or_default()
    }

    /// The **active** portfolio (Story 6.1): the user-selected one when it still exists, else the
    /// first (deterministic). `None` only when no portfolio exists yet. A pure read.
    pub fn active_portfolio(&self) -> Option<PortfolioItem> {
        let portfolios = self.list_portfolios();
        if let Some(id) = self.active_portfolio_id
            && let Some(p) = portfolios.iter().find(|p| p.id == id)
        {
            return Some(p.clone());
        }
        portfolios.into_iter().next()
    }

    /// The active portfolio id (for `main.rs` to persist into `AppConfig`). `None` = no portfolio yet.
    pub fn active_portfolio_id(&self) -> Option<Uuid> {
        self.active_portfolio().map(|p| p.id)
    }

    /// Select the active portfolio (Story 6.1). Accepts only an id that currently exists (a stale id
    /// is ignored → the getter falls back to the first). In-memory; `main.rs` persists it.
    pub fn set_active_portfolio(&mut self, id: Uuid) {
        if self.list_portfolios().iter().any(|p| p.id == id) {
            self.active_portfolio_id = Some(id);
        }
    }

    /// Add a named portfolio (Story 6.1, FR37) — "one per bank/account". Validates the name (non-empty)
    /// in the app layer; id/timestamp from the injected sources (ADD15). A fresh portfolio becomes the
    /// active one. Guarded (read-only / no-journal / save-failure → a neutral notice).
    pub fn add_portfolio(&mut self, name: &str) -> Result<Uuid, String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let name = name.trim();
        if name.is_empty() {
            return Err(MSG_PORTFOLIO_INVALID_NAME.to_string());
        }
        let id = self.idgen.new_id();
        let created_at = self.clock.now();
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .add_portfolio(id, name, &created_at)
            .map_err(watch_error)?;
        self.active_portfolio_id = Some(id);
        Ok(id)
    }

    /// Rename a portfolio (Story 6.1). Same name guard. A no-op (identical name) writes nothing.
    pub fn rename_portfolio(&mut self, id: Uuid, name: &str) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let name = name.trim();
        if name.is_empty() {
            return Err(MSG_PORTFOLIO_INVALID_NAME.to_string());
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .rename_portfolio(id, name)
            .map(|_| ())
            .map_err(watch_error)
    }

    /// Delete a portfolio (Story 6.1), surfacing the persistence guards as neutral refusals: a
    /// portfolio with holdings, or the last portfolio, is **not** removed. On a real delete that drops
    /// the active selection, the active id is cleared → the getter falls back to the first.
    pub fn delete_portfolio(&mut self, id: Uuid) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        match journal.delete_portfolio(id).map_err(watch_error)? {
            DeletePortfolioOutcome::Deleted => {
                if self.active_portfolio_id == Some(id) {
                    self.active_portfolio_id = None;
                }
                Ok(())
            }
            DeletePortfolioOutcome::HasHoldings => Err(MSG_PORTFOLIO_HAS_HOLDINGS.to_string()),
            DeletePortfolioOutcome::LastPortfolio => Err(MSG_PORTFOLIO_LAST.to_string()),
        }
    }

    /// The active portfolio, creating the default one if the journal has none yet (the add-holding
    /// path). Mints an id/timestamp **only** when no portfolio exists (ADD15).
    fn active_portfolio_or_default(&mut self) -> Result<PortfolioItem, String> {
        if let Some(p) = self.active_portfolio() {
            return Ok(p);
        }
        self.ensure_default_portfolio()
    }

    // ── Holdings register (Story 4.3, FR36 — scoped to the active portfolio since Story 6.1) ──

    /// The **active** portfolio's holdings, ordered by creation. Empty when no journal / no portfolio
    /// exists yet. A pure read — it never creates the portfolio (that happens on the first add).
    pub fn list_holdings(&self) -> Vec<HoldingItem> {
        let Some(journal) = self.journal.as_ref() else {
            return Vec::new();
        };
        let Some(portfolio) = self.active_portfolio() else {
            return Vec::new();
        };
        journal.list_holdings(portfolio.id).unwrap_or_else(|error| {
            tracing::warn!("list_holdings failed: {error}");
            Vec::new()
        })
    }

    /// The active portfolio's **sold (retired) positions** (issue #84, the « Positions vendues »
    /// section): holdings whose `sold_at` is stamped, most recently sold first (then id — a
    /// deterministic order). Empty when no journal / no portfolio / on a read failure (a display
    /// surface). Their ledger stays readable via [`Self::holding_ledger`] and a re-buy through
    /// [`Self::record_buy_for`] re-opens the position.
    pub fn sold_holdings(&self) -> Vec<HoldingItem> {
        let Some(journal) = self.journal.as_ref() else {
            return Vec::new();
        };
        let Some(portfolio) = self.active_portfolio() else {
            return Vec::new();
        };
        let mut sold: Vec<HoldingItem> = journal
            .list_all_holdings()
            .unwrap_or_else(|error| {
                tracing::warn!("sold_holdings failed: {error}");
                Vec::new()
            })
            .into_iter()
            .filter(|h| h.portfolio_id == portfolio.id && h.sold_at.is_some())
            .collect();
        sold.sort_by(|a, b| b.sold_at.cmp(&a.sold_at).then_with(|| a.id.cmp(&b.id)));
        sold
    }

    /// The active portfolio's **capital-at-risk** + **total invested**, grouped **per currency**
    /// (Story 6.2, FR38 — the honest interim before FX lands in Story 6.5). Holdings now differ in
    /// currency, so summing them into one figure would silently mix currencies (forbidden — FR28).
    /// Instead we group by each holding's **effective currency** (`h.currency`, or `reference_currency`
    /// for a pre-6.2 `None` row) and, for **each** bucket, call the unchanged single-currency
    /// `core::risk::capital_at_risk` / `total_invested`. Returns `(currency, capital_at_risk,
    /// total_invested)` sorted by currency code (deterministic); an empty portfolio yields an empty
    /// vec. There is **no** consolidated global total — cross-currency consolidation needs FX
    /// (Story 6.5 / 6.6). A holding whose persisted TEXT decimals don't parse is skipped (defensive).
    pub fn portfolio_capital_at_risk_by_currency(
        &self,
        reference_currency: &str,
    ) -> Vec<(String, Decimal, Decimal)> {
        car_buckets_by_currency(&self.list_holdings(), reference_currency)
    }

    /// The active portfolio's **un-protected exposure** per currency (issue #61): for each currency,
    /// the count of active holdings with **no trailing stop** and their total invested value. The
    /// honest complement to capital-at-risk — a portfolio with no stops reads "0 % at risk", so this
    /// states plainly how much simply has no stop-loss protection defined. Sorted by currency; only
    /// currencies that have an un-stopped holding appear (empty when every holding is stop-protected).
    pub fn portfolio_unstopped_exposure_by_currency(
        &self,
        reference_currency: &str,
    ) -> Vec<(String, usize, Decimal)> {
        unstopped_exposure_by_currency(&self.list_holdings(), reference_currency)
    }

    /// Ensure the single default portfolio exists and return it (FR36, single-portfolio). Lazily
    /// created with an injected id/timestamp (ADD15) on first use; idempotent thereafter. The id/
    /// timestamp are minted **only when the portfolio is absent** — so a repeat add doesn't burn an
    /// `IdGen` id (which would shift a deterministic test sequence) and the common path is a pure read.
    fn ensure_default_portfolio(&mut self) -> Result<PortfolioItem, String> {
        let journal = self.journal.as_ref().ok_or(MSG_NO_JOURNAL.to_string())?;
        if let Some(existing) = journal.first_portfolio().map_err(watch_error)? {
            return Ok(existing);
        }
        let id = self.idgen.new_id();
        let created_at = self.clock.now();
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .ensure_portfolio(id, DEFAULT_PORTFOLIO_NAME, &created_at)
            .map_err(watch_error)
    }

    /// Add a holding (FR36): a security symbol, a quantity, a purchase price and the `currency` it is
    /// denominated in (Story 6.2, FR38). Validates the symbol (non-empty), the two decimals (exact,
    /// quantity > 0, price ≥ 0) and the currency (a supported allow-list member) **in the app layer**
    /// — persistence stores faithfully, native, never converted (FR28). Id/timestamp from the injected
    /// sources. Guarded (read-only / no-journal / save-failure → a neutral notice).
    pub fn add_holding(
        &mut self,
        ticker: &str,
        quantity: &str,
        purchase_price: &str,
        currency: &str,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let ticker = ticker.trim();
        if ticker.is_empty() {
            return Err(MSG_HOLDING_INVALID_TICKER.to_string());
        }
        if !crate::config::is_supported_currency(currency) {
            return Err(MSG_HOLDING_INVALID_CURRENCY.to_string());
        }
        let (quantity, purchase_price) = validate_holding_amounts(quantity, purchase_price)?;
        let portfolio_id = self.active_portfolio_or_default()?.id;
        let id = self.idgen.new_id();
        let created_at = self.clock.now();
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .add_holding(
                id,
                portfolio_id,
                ticker,
                &quantity,
                &purchase_price,
                currency,
                &created_at,
            )
            .map(|_| ())
            .map_err(watch_error)
    }

    /// Edit a holding's symbol, quantity, purchase price and/or `currency` (FR36 / Story 6.2 FR38).
    /// Same validation as [`Self::add_holding`]. A no-op (identical values) writes nothing.
    ///
    /// Story 6.3 guard (2026-07-02 review, HIGH): once the holding is **ledger-backed** (any
    /// transaction row exists), its quantity/price are the DERIVED weighted-average aggregate and
    /// its currency is stamped on every row — a direct rewrite would silently desynchronize them
    /// from the recorded history ("sell all" would become a partial sell). Changing those three is
    /// refused with [`MSG_LEDGER_BACKED`] (the ledger is the correction surface); the **ticker**
    /// stays editable (it is not ledger-derived).
    pub fn update_holding(
        &mut self,
        id: Uuid,
        ticker: &str,
        quantity: &str,
        purchase_price: &str,
        currency: &str,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let ticker = ticker.trim();
        if ticker.is_empty() {
            return Err(MSG_HOLDING_INVALID_TICKER.to_string());
        }
        if !crate::config::is_supported_currency(currency) {
            return Err(MSG_HOLDING_INVALID_CURRENCY.to_string());
        }
        let (quantity, purchase_price) = validate_holding_amounts(quantity, purchase_price)?;
        let journal = self.journal.as_ref().ok_or(MSG_NO_JOURNAL.to_string())?;
        let ledger_backed = !journal
            .list_transactions(id)
            .map_err(watch_error)?
            .is_empty();
        if ledger_backed {
            let current = journal
                .list_all_holdings()
                .map_err(watch_error)?
                .into_iter()
                .find(|h| h.id == id);
            if let Some(current) = current {
                let currency_changed = current.currency.as_deref().is_some_and(|c| c != currency);
                if current.quantity != quantity
                    || current.purchase_price != purchase_price
                    || currency_changed
                {
                    return Err(MSG_LEDGER_BACKED.to_string());
                }
            }
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .update_holding(id, ticker, &quantity, &purchase_price, currency)
            .map_err(watch_error)
    }

    /// Remove a holding (FR36). Guarded; an absent id is a neutral no-op.
    pub fn delete_holding(&mut self, id: Uuid) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal.delete_holding(id).map_err(watch_error)
    }

    /// Set (or clear) a holding's trailing-stop percentage (Story 4.5, FR42). An empty `pct_input`
    /// clears the stop. Otherwise the pct is validated to `(0, 100)` and the level is **seeded fresh**
    /// from the *reference price* — the matched study's `current_price` if known, else the holding's
    /// `purchase_price` — so the user's chosen pct wins (they may tighten OR loosen the stop). The
    /// ratchet-up-only rule (FR42) governs the **automatic** price-driven trailing
    /// ([`Self::ratchet_trailing_stops_for_study`]), NOT an explicit re-parametrisation — folding the
    /// prior level here would make the displayed pct and level inconsistent (review finding). Both
    /// pct + level persist together (idempotent). Guarded.
    pub fn set_holding_trailing_stop(
        &mut self,
        holding_id: Uuid,
        pct_input: &str,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let pct_input = pct_input.trim();
        if pct_input.is_empty() {
            // Clear the stop (both fields → NULL).
            let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
            return journal
                .set_trailing_stop(holding_id, None, None)
                .map_err(watch_error);
        }
        let pct = Decimal::from_str_exact(pct_input)
            .ok()
            .filter(|p| p.is_sign_positive() && !p.is_zero() && *p < Decimal::ONE_HUNDRED)
            .ok_or(MSG_HOLDING_INVALID_STOP.to_string())?;
        let holding = self
            .list_holdings()
            .into_iter()
            .find(|h| h.id == holding_id)
            .ok_or(MSG_HOLDING_INVALID_STOP.to_string())?;
        let reference_price = self
            // Issue #81: match the study in the holding's own currency (a cross-currency study must
            // not seed this stop level).
            .study_id_for_ticker_in_currency(&holding.security_ticker, holding.currency.as_deref())
            .and_then(|sid| self.get_study(sid))
            .and_then(|s| s.judgment.current_price)
            .map(|m| m.as_decimal())
            .or_else(|| Decimal::from_str_exact(&holding.purchase_price).ok())
            .ok_or(MSG_HOLDING_INVALID_STOP.to_string())?;
        // Seed fresh (no prior level) — an explicit set is the user redefining the stop, not an
        // automatic ratchet, so it may move the level down as well as up.
        let level = steadyinvest_core::risk::ratchet_trailing_stop(None, reference_price, pct);
        // Normalize (drop trailing zeros) so the stored string is canonical — re-computing the same
        // value yields the same string, which keeps the persistence no-op idempotency guard honest.
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .set_trailing_stop(
                holding_id,
                Some(&pct.normalize().to_string()),
                Some(&level.normalize().to_string()),
            )
            .map_err(watch_error)
    }

    /// Ratchet the trailing-stop level of every holding of `study_id`'s ticker against a fresh price
    /// (Story 4.5, FR42) — called after a holdings price refresh fills the study's `current_price`
    /// ([`Self::apply_holding_price`]). Only holdings that **have** a stop set are touched; the
    /// `core::risk` ratchet (and the persistence no-op guard) ensure a falling price writes nothing.
    pub fn ratchet_trailing_stops_for_study(
        &mut self,
        study_id: Uuid,
        price: Decimal,
    ) -> Result<(), String> {
        if self.read_only {
            return Ok(()); // a read-only refresh simply doesn't ratchet — never an error
        }
        let Some((ticker, study_currency)) = self
            .get_study(study_id)
            .map(|s| (s.security_ticker, s.native_currency))
        else {
            return Ok(());
        };
        let targets: Vec<(Uuid, Decimal, Option<Decimal>)> = self
            .list_holdings()
            .into_iter()
            // Issue #81: ratchet only holdings in the study's OWN currency — the study's price is in
            // that currency, so a cross-currency same-ticker holding must not be ratcheted with it. A
            // holding that declares no currency still ratchets (today's behaviour).
            .filter(|h| {
                h.security_ticker.eq_ignore_ascii_case(&ticker)
                    && h.currency
                        .as_deref()
                        .is_none_or(|c| c.eq_ignore_ascii_case(&study_currency))
            })
            .filter_map(|h| {
                let pct = h
                    .trailing_stop_pct
                    .as_deref()
                    .and_then(|s| Decimal::from_str_exact(s).ok())?;
                let prior = h
                    .trailing_stop_level
                    .as_deref()
                    .and_then(|s| Decimal::from_str_exact(s).ok());
                Some((h.id, pct, prior))
            })
            .collect();
        for (id, pct, prior) in targets {
            let level = steadyinvest_core::risk::ratchet_trailing_stop(prior, price, pct);
            let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
            journal
                .set_trailing_stop(
                    id,
                    Some(&pct.normalize().to_string()),
                    Some(&level.normalize().to_string()),
                )
                .map_err(watch_error)?;
        }
        Ok(())
    }
}

/// The display name of the single default portfolio (Story 4.3, FR36). Not user-editable in 4.3
/// (multi-portfolio naming is FR37/Epic 6).
const DEFAULT_PORTFOLIO_NAME: &str = "Portefeuille";

/// Group ACTIVE holdings into per-currency capital-at-risk buckets (Stories 6.2/6.6, FR38/FR44):
/// the ONE grouping both the active-portfolio read and the journal-wide consolidation use — group
/// by effective currency, then call the unchanged single-currency `core::risk` folds once per
/// bucket. Deterministic order (BTreeMap); unparseable stored decimals skipped defensively.
pub(crate) fn car_buckets_by_currency(
    holdings: &[HoldingItem],
    reference_currency: &str,
) -> Vec<(String, Decimal, Decimal)> {
    use std::collections::BTreeMap;
    let mut by_ccy: BTreeMap<String, Vec<steadyinvest_core::risk::PositionRisk>> = BTreeMap::new();
    for h in holdings {
        let Ok(avg_cost) = Decimal::from_str_exact(&h.purchase_price) else {
            continue;
        };
        let Ok(quantity) = Decimal::from_str_exact(&h.quantity) else {
            continue;
        };
        let stop = h
            .trailing_stop_level
            .as_deref()
            .and_then(|s| Decimal::from_str_exact(s).ok());
        by_ccy
            .entry(effective_currency(h, reference_currency))
            .or_default()
            .push(steadyinvest_core::risk::PositionRisk {
                avg_cost,
                stop,
                quantity,
            });
    }
    by_ccy
        .into_iter()
        .map(|(currency, positions)| {
            (
                currency,
                steadyinvest_core::risk::capital_at_risk(&positions),
                steadyinvest_core::risk::total_invested(&positions),
            )
        })
        .collect()
}

/// Per currency, the count and total invested value of active holdings with **no trailing stop**
/// (issue #61) — the un-protected exposure. A stop that fails to parse counts as absent (same
/// treatment as [`car_buckets_by_currency`], where an unparseable stop contributes no protection).
/// Sorted by currency; only currencies with an un-stopped holding appear.
pub(crate) fn unstopped_exposure_by_currency(
    holdings: &[HoldingItem],
    reference_currency: &str,
) -> Vec<(String, usize, Decimal)> {
    use std::collections::BTreeMap;
    let mut by_ccy: BTreeMap<String, Vec<steadyinvest_core::risk::PositionRisk>> = BTreeMap::new();
    for h in holdings {
        // A holding with a PARSEABLE trailing stop is protected — skip it. Absent or unparseable → un-protected.
        let has_stop = h
            .trailing_stop_level
            .as_deref()
            .and_then(|s| Decimal::from_str_exact(s).ok())
            .is_some();
        if has_stop {
            continue;
        }
        let Ok(avg_cost) = Decimal::from_str_exact(&h.purchase_price) else {
            continue;
        };
        let Ok(quantity) = Decimal::from_str_exact(&h.quantity) else {
            continue;
        };
        by_ccy
            .entry(effective_currency(h, reference_currency))
            .or_default()
            .push(steadyinvest_core::risk::PositionRisk {
                avg_cost,
                stop: None,
                quantity,
            });
    }
    by_ccy
        .into_iter()
        .map(|(currency, positions)| {
            (
                currency,
                positions.len(),
                steadyinvest_core::risk::total_invested(&positions),
            )
        })
        .collect()
}

/// A holding's **effective currency** (Story 6.2, FR38): its own `currency` when set, else the
/// caller's reference currency — the ONE read-boundary coalescing rule for a pre-6.2 legacy row
/// (the v6 `ADD COLUMN` left it NULL; persistence never rewrites it). Every consumer — the
/// per-currency capital-at-risk buckets, the sell-transaction stamp, the register row labels —
/// goes through this helper so the rule cannot drift.
pub(crate) fn effective_currency(holding: &HoldingItem, reference_currency: &str) -> String {
    holding
        .currency
        .clone()
        .unwrap_or_else(|| reference_currency.to_string())
}

/// The largest magnitude a holding's quantity or price may carry (issue #60): a trillion. Orders of
/// magnitude beyond any real personal portfolio, but small enough that `quantity × price` — and the
/// sum of those across positions — stays comfortably inside `Decimal`'s ~7.9e28 range, so the
/// capital-at-risk overlay never has to saturate an absurd persisted value into a misleading total
/// (its arithmetic is defensively saturating; this write-side bound keeps that path unreachable).
fn max_holding_magnitude() -> Decimal {
    Decimal::from(1_000_000_000_000_i64) // 1e12
}

/// Validate a holding's quantity and purchase price (Story 4.3, FR36 + NFR-C1). Both must parse as
/// **exact** decimals (`Decimal::from_str_exact` — errors instead of silently rounding); quantity
/// must be strictly positive, price non-negative, and both within [`max_holding_magnitude`] (issue
/// #60). On success returns their **canonical** decimal spellings to store as TEXT; a non-number is
/// the neutral [`MSG_HOLDING_INVALID_NUMBER`], an out-of-range magnitude
/// [`MSG_HOLDING_AMOUNT_OUT_OF_RANGE`].
fn validate_holding_amounts(
    quantity: &str,
    purchase_price: &str,
) -> Result<(String, String), String> {
    let qty = Decimal::from_str_exact(quantity.trim())
        .ok()
        .filter(|q| q.is_sign_positive() && !q.is_zero())
        .ok_or(MSG_HOLDING_INVALID_NUMBER.to_string())?;
    let price = Decimal::from_str_exact(purchase_price.trim())
        .ok()
        .filter(|p| !p.is_sign_negative())
        .ok_or(MSG_HOLDING_INVALID_NUMBER.to_string())?;
    let max = max_holding_magnitude();
    if qty > max || price > max {
        return Err(MSG_HOLDING_AMOUNT_OUT_OF_RANGE.to_string());
    }
    Ok((qty.to_string(), price.to_string()))
}
