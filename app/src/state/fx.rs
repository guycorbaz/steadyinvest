//! FX-rate rails (Story 6.5, FR28) — dated, source-aware rates; acquisition + storage ONLY.
//!
//! FR28's structural rule: every figure stays in its native currency; conversion happens only at
//! the consolidation views (Story 6.6, via the pure `core::risk::fx::convert` — which nothing in
//! this module calls). Here the app orchestrates the two acquisition paths:
//!
//! - **provider refresh** (user-initiated, FR65): one pair per FOREIGN currency actually present
//!   among the active journal's holdings, quoted against the reference currency; the fetched rate
//!   is stamped with the fetch **day** (the provider's true session date is #72's known
//!   limitation, shared with the price cache) and the provider wire name as `source`;
//! - **manual entry** (`source = "manuel"`): the honest offline/free path.
//!
//! Rates are stored as `BASE → reference` only — a missing pair is refused honestly downstream,
//! never derived by silent `1/rate` inversion.

use rust_decimal::Decimal;
use steadyinvest_persistence::FxRateItem;

use super::ledger::normalize_event_date;
use super::{
    JournalState, MSG_FX_FUTURE_DATE, MSG_FX_INVALID_CURRENCY, MSG_FX_INVALID_RATE,
    MSG_FX_SAME_CURRENCY, MSG_NO_JOURNAL, MSG_READ_ONLY_WRITE, watch_error,
};

impl JournalState {
    /// The stored rates, deterministic order (pair, most-recent date first) — the Réglages panel
    /// read. `[]` without a journal or on a read failure (a display surface).
    pub fn list_fx_rates(&self) -> Vec<FxRateItem> {
        self.journal
            .as_ref()
            .and_then(|j| j.list_fx_rates().ok())
            .unwrap_or_default()
    }

    /// The FOREIGN currencies in use (Story 6.5, AC3): the effective currencies of the ACTIVE
    /// journal's holdings (all portfolios — consolidation is journal-wide, and sold holdings'
    /// history counts too since 6.6 reads it), minus the reference currency. Deterministic order.
    /// This is the provider-refresh pair set — no speculative pairs.
    pub fn foreign_currencies_in_use(&self, reference_currency: &str) -> Vec<String> {
        use std::collections::BTreeSet;
        let Some(journal) = self.journal.as_ref() else {
            return Vec::new();
        };
        let holdings = journal.list_all_holdings().unwrap_or_default();
        // Uppercase + allow-list filter (2026-07-02 review): an imported off-list or
        // case-mismatched holding currency must not poison the fetch set with pairs the store
        // would refuse anyway ("n-1/n actualisés" forever with no cause).
        let set: BTreeSet<String> = holdings
            .iter()
            .map(|h| super::effective_currency(h, reference_currency).to_uppercase())
            .filter(|c| c != reference_currency && crate::config::is_supported_currency(c))
            .collect();
        set.into_iter().collect()
    }

    /// Record a MANUAL rate (Story 6.5, AC4): `base → reference_currency` at `rate_input` on
    /// `date_input` (`""` = today; real-calendar validation), `source = "manuel"`. Validates:
    /// supported base ≠ reference; rate a strictly positive exact decimal. A same
    /// `(pair, date, source)` re-entry updates in place (the persistence natural-key upsert);
    /// identical values are a true no-op. Guarded.
    pub fn upsert_manual_fx_rate(
        &mut self,
        base: &str,
        rate_input: &str,
        date_input: &str,
        reference_currency: &str,
    ) -> Result<(), String> {
        self.upsert_fx_rate_from(base, rate_input, date_input, reference_currency, "manuel")
    }

    /// Apply one PROVIDER-fetched rate (Story 6.5, AC3): `base → quote` at the fetched value,
    /// dated the fetch **day** (#72 interim rule), `source` = the provider wire name. Guarded.
    pub fn apply_fx_fetch(
        &mut self,
        base: &str,
        quote: &str,
        rate: Decimal,
        source: &str,
    ) -> Result<(), String> {
        self.upsert_fx_rate_from(base, &rate.normalize().to_string(), "", quote, source)
    }

    /// The shared validated upsert: normalize the inputs, mint id/stamp (ADD15), one persistence
    /// call (one tx, ≤1 bump).
    fn upsert_fx_rate_from(
        &mut self,
        base: &str,
        rate_input: &str,
        date_input: &str,
        quote: &str,
        source: &str,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let base = base.trim().to_uppercase();
        let quote = quote.trim().to_uppercase();
        if base == quote {
            return Err(MSG_FX_SAME_CURRENCY.to_string());
        }
        if !crate::config::is_supported_currency(&base) {
            return Err(MSG_FX_INVALID_CURRENCY.to_string());
        }
        let rate = Decimal::from_str_exact(rate_input.trim())
            .ok()
            .filter(|r| r.is_sign_positive() && !r.is_zero())
            .ok_or(MSG_FX_INVALID_RATE.to_string())?;
        let now = self.clock.now();
        // The DAY only (rates are daily facts): reuse the 6.3 real-calendar validation, then keep
        // the date part — `fx_rates.rate_date` stores `AAAA-MM-JJ`.
        let stamped = normalize_event_date(date_input, &now.0)?;
        let rate_date = stamped.get(..10).unwrap_or(&stamped).to_string();
        // A future date would win the "latest" arbitration until it arrives (review) — refuse.
        if rate_date.as_str() > now.0.get(..10).unwrap_or_default() {
            return Err(MSG_FX_FUTURE_DATE.to_string());
        }
        let item = FxRateItem {
            id: self.idgen.new_id(),
            base_currency: base,
            quote_currency: quote,
            rate: rate.normalize().to_string(),
            rate_date,
            source: source.to_string(),
            created_at: now,
        };
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .upsert_fx_rate(&item)
            .map(|_| ())
            .map_err(watch_error)
    }
}

/// One bank's (portfolio's) consolidation slice (Story 6.6, FR44): the FX-free native buckets, the
/// converted subtotal in the reference currency (`None` when a pair is missing or a checked sum
/// overflowed — absent, never wrong), and the missing pairs by name.
pub struct BankConsolidation {
    pub name: String,
    /// `(currency, capital_at_risk, total_invested)` — native, FX-free (FR28).
    pub buckets: Vec<(String, Decimal, Decimal)>,
    /// `(capital_at_risk, total_invested)` in the REFERENCE currency.
    pub converted: Option<(Decimal, Decimal)>,
    /// The `BASE → reference` pairs this bank needed but the store lacks (e.g. `"EUR → CHF"`).
    pub missing_pairs: Vec<String>,
    /// `true` when the bank could not consolidate for a reason that is NOT a nameable missing
    /// pair (2026-07-02 review): the holdings read failed, or a checked conversion/sum
    /// overflowed. The UI renders a plain « indisponible » — an absence, never a zero.
    pub unavailable: bool,
}

/// The journal-wide FR44 hierarchy: per-currency → per-bank → global, conversion ONLY here (the
/// consolidation point — FR28), every rate used carried for display (date + source inspectable).
pub struct JournalConsolidation {
    pub banks: Vec<BankConsolidation>,
    /// The global `(capital_at_risk, total_invested)` in the reference currency — `None` when any
    /// bank could not consolidate (missing pair / overflow): never a partial sum passed off as
    /// the total.
    pub global: Option<(Decimal, Decimal)>,
    /// Union of the banks' missing pairs, deduplicated, deterministic.
    pub missing_pairs: Vec<String>,
    /// The rates actually used (deduplicated per pair) — the UI footnote (FR28 inspectability).
    pub rates_used: Vec<FxRateItem>,
}

impl JournalState {
    /// The FR44 consolidation (Story 6.6): for EVERY portfolio, the ACTIVE holdings' per-currency
    /// buckets (sold holdings carry no position risk — the unchanged 4.6 semantics), each foreign
    /// bucket converted at the LATEST stored rate ([`Journal::latest_fx_rate`] — 6.5's
    /// arbitration; the reference currency converts at identity, no self-rate looked up), summed
    /// checked per bank and globally. A missing pair (or a corrupt stored rate, or an overflow)
    /// makes the affected subtotal and the global ABSENT with the pair named — honesty over
    /// completeness. A pure read; deterministic throughout.
    pub fn journal_capital_at_risk_consolidation(
        &self,
        reference_currency: &str,
    ) -> JournalConsolidation {
        use std::collections::BTreeSet;
        let mut banks = Vec::new();
        let mut rates_used: Vec<FxRateItem> = Vec::new();
        let mut all_missing: BTreeSet<String> = BTreeSet::new();
        let Some(journal) = self.journal.as_ref() else {
            return JournalConsolidation {
                banks,
                global: None,
                missing_pairs: Vec::new(),
                rates_used,
            };
        };
        // Bank order = the portfolio list order (deterministic — the 6.1 selector order), so the
        // UI lines and the tests share one contract.
        for portfolio in self.list_portfolios() {
            // A failed holdings read is an ABSENT bank, never a confident zero (2026-07-02
            // review, HIGH — the "absent, never wrong" rule applies to IO too).
            let holdings = match journal.list_holdings(portfolio.id) {
                Ok(holdings) => holdings,
                Err(_) => {
                    banks.push(BankConsolidation {
                        name: portfolio.name.clone(),
                        buckets: Vec::new(),
                        converted: None,
                        missing_pairs: Vec::new(),
                        unavailable: true,
                    });
                    continue;
                }
            };
            let buckets = super::holdings::car_buckets_by_currency(&holdings, reference_currency);
            let mut converted: Option<(Decimal, Decimal)> = Some((Decimal::ZERO, Decimal::ZERO));
            let mut missing: Vec<String> = Vec::new();
            for (currency, car, invested) in &buckets {
                let (car_ref, invested_ref) = if currency == reference_currency {
                    // Identity — the reference bucket needs (and must never require) a self-rate.
                    (Some(*car), Some(*invested))
                } else {
                    let rate_row = journal
                        .latest_fx_rate(currency, reference_currency, None)
                        .ok()
                        .flatten();
                    match rate_row
                        .as_ref()
                        .and_then(|r| Decimal::from_str_exact(&r.rate).ok())
                        // A nonpositive stored rate (an edited/foreign import) must not convert
                        // to a confident zero — folded into the named refusal (review).
                        .filter(|r| r.is_sign_positive() && !r.is_zero())
                    {
                        Some(rate) => {
                            if let Some(row) = rate_row
                                && !rates_used.iter().any(|u| {
                                    u.base_currency == row.base_currency
                                        && u.quote_currency == row.quote_currency
                                })
                            {
                                rates_used.push(row);
                            }
                            (
                                steadyinvest_core::risk::convert(*car, rate),
                                steadyinvest_core::risk::convert(*invested, rate),
                            )
                        }
                        None => {
                            missing.push(format!("{currency} → {reference_currency}"));
                            (None, None)
                        }
                    }
                };
                converted = match (converted, car_ref, invested_ref) {
                    (Some((acc_car, acc_inv)), Some(c), Some(i)) => {
                        acc_car.checked_add(c).zip(acc_inv.checked_add(i))
                    }
                    _ => None,
                };
            }
            all_missing.extend(missing.iter().cloned());
            // `converted` is None either because a pair is missing (named) or because a checked
            // conversion/sum overflowed — the latter is the plain-« indisponible » case (review:
            // the absence must never render as a dangling empty amount).
            let unavailable = converted.is_none() && missing.is_empty();
            banks.push(BankConsolidation {
                name: portfolio.name.clone(),
                buckets,
                converted,
                missing_pairs: missing,
                unavailable,
            });
        }
        let global = banks.iter().try_fold(
            (Decimal::ZERO, Decimal::ZERO),
            |(acc_car, acc_inv), bank| {
                let (c, i) = bank.converted?;
                acc_car.checked_add(c).zip(acc_inv.checked_add(i))
            },
        );
        JournalConsolidation {
            banks,
            global,
            missing_pairs: all_missing.into_iter().collect(),
            rates_used,
        }
    }
}
