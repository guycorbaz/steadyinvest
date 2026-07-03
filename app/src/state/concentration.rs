//! Concentration & diversify-by-size read (Story 6.7, FR45) — journal-wide, a pure read.
//!
//! FR45's point (PRD Journey 3): concentration is checked against the **total** invested capital,
//! "regardless of which bank or currency holds it" — so every ACTIVE holding across ALL
//! portfolios aggregates by security (uppercased ticker; the allow-list uppercases every write
//! site) and converts to the reference currency at the latest stored rate. Conversion happens
//! ONLY here (a consolidation-point read, the FR28 structural rule shared with Story 6.6); a
//! missing pair absents the affected figure with the pair named — never a partial sum passed off
//! as a share, never a silent inversion. The share division and the size classification are pure
//! `core::risk` arithmetic ([`share_pct`], [`size_class`]).
//!
//! The denominator is the checked sum of the per-ticker converted figures — self-consistent with
//! the numerators (the shares sum to 100 when everything converts) and identical to the 6.6
//! global invested by construction (same holdings, same rates, same identity rule).

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use steadyinvest_core::risk::{share_pct, size_class, SizeClass};
use steadyinvest_persistence::FxRateItem;

use super::JournalState;

/// One security's slice of the total invested capital (FR45). `invested`/`share_pct` are absent
/// (with the pairs named) when a needed rate is missing or a checked operation overflowed.
pub struct ConcentrationRow {
    /// The UPPERCASED ticker (the journal-wide aggregation key).
    pub ticker: String,
    /// The security's invested capital in the REFERENCE currency, summed across all banks.
    pub invested: Option<Decimal>,
    /// `invested / global × 100` — absent when either side is.
    pub share_pct: Option<Decimal>,
    /// The `BASE → reference` pairs this security needed but the store lacks.
    pub missing_pairs: Vec<String>,
}

/// Why a held security could not be classified by size (Story 6.7, AC3) — an honest bucket,
/// never a default class.
pub enum UnclassifiedReason {
    /// No saved study matches the ticker (case-insensitively).
    NoStudy,
    /// The matched study has no year with a sales value.
    NoSales,
    /// The sales figure could not convert — the named `BASE → reference` pair is missing.
    MissingRate(String),
    /// A rate exists but the checked conversion could not state the figure (overflow) — the
    /// sales are present, so neither `NoSales` nor `MissingRate` would be true.
    Unconvertible,
}

/// One unclassifiable security, with its named reason.
pub struct UnclassifiedRow {
    pub ticker: String,
    pub reason: UnclassifiedReason,
}

/// One size class's slice of the mix: its share of the global invested capital. Absent when any
/// member security's converted figure is (never a partial class sum) or the denominator is.
pub struct SizeMixSlot {
    pub share_pct: Option<Decimal>,
}

/// The journal-wide FR45 view: per-security concentration rows (largest share first), the size
/// mix against the configured table, the honest leftovers, and every rate used (FR28 footnote).
pub struct JournalDiversification {
    /// Per-security rows, sorted by descending invested (absent last), then ticker.
    pub rows: Vec<ConcentrationRow>,
    /// The total invested capital in the reference currency — the shares' denominator. `None`
    /// when any security could not convert (named below) or a checked sum overflowed.
    pub global_invested: Option<Decimal>,
    pub small: SizeMixSlot,
    pub medium: SizeMixSlot,
    pub large: SizeMixSlot,
    /// Securities outside the three classes, each with its named reason.
    pub unclassified: Vec<UnclassifiedRow>,
    /// Union of the missing pairs that absent the DENOMINATOR (holding conversions only —
    /// classification-only pairs are named on their « non classé » rows, never here).
    pub missing_pairs: Vec<String>,
    /// The rates actually used (deduplicated per pair) — the UI footnote (FR28 inspectability).
    pub rates_used: Vec<FxRateItem>,
    /// `true` when the view could not be built at all (no journal / a failed holdings read) —
    /// an absence, never an empty-looking zero state (the 6.6 review rule applied to IO).
    pub unavailable: bool,
}

impl JournalDiversification {
    fn unavailable() -> Self {
        JournalDiversification {
            rows: Vec::new(),
            global_invested: None,
            small: SizeMixSlot { share_pct: None },
            medium: SizeMixSlot { share_pct: None },
            large: SizeMixSlot { share_pct: None },
            unclassified: Vec::new(),
            missing_pairs: Vec::new(),
            rates_used: Vec::new(),
            unavailable: true,
        }
    }
}

/// A per-ticker accumulator while summing holdings.
#[derive(Default)]
struct TickerAcc {
    /// `Some(sum)` while every converted figure landed; `None` after a missing pair/overflow.
    invested: Option<Decimal>,
    missing: Vec<String>,
    started: bool,
}

impl JournalState {
    /// The FR45 diversification view (Story 6.7): every ACTIVE holding of EVERY portfolio,
    /// aggregated per uppercased ticker, converted at the latest stored rate (identity for the
    /// reference currency — no self-rate looked up), divided by the checked global total; each
    /// security classified by its study's latest entered sales against the configured boundaries.
    /// Sold holdings are EXCLUDED (position facts — the unchanged 4.6/6.2 semantics). A pure
    /// read; deterministic throughout.
    pub fn journal_diversification(
        &self,
        reference_currency: &str,
        small_max: Decimal,
        medium_max: Decimal,
    ) -> JournalDiversification {
        let Some(journal) = self.journal.as_ref() else {
            return JournalDiversification::unavailable();
        };
        // 1) Collect every ACTIVE holding journal-wide. A failed read — portfolios OR any
        // bank's holdings — makes the WHOLE view unavailable (2026-07-03 review: the swallowing
        // `self.list_portfolios()` would render a vanished block, an empty-looking zero state):
        // unlike the 6.6 per-bank lines, concentration has no honest partial — a share against
        // an incomplete total is exactly the "partial passed off as total" FR45 forbids.
        let Ok(portfolios) = journal.list_portfolios() else {
            return JournalDiversification::unavailable();
        };
        let mut all_holdings = Vec::new();
        for portfolio in portfolios {
            match journal.list_holdings(portfolio.id) {
                Ok(mut holdings) => all_holdings.append(&mut holdings),
                Err(_) => return JournalDiversification::unavailable(),
            }
        }

        let mut rates_used: Vec<FxRateItem> = Vec::new();
        let mut all_missing: std::collections::BTreeSet<String> = Default::default();

        // The shared rate lookup, MEMOIZED per base currency (2026-07-03 review: N same-currency
        // holdings must not issue N identical DB reads per render): `Some(rate)` (recorded once
        // for the footnote) or `None` with the pair named into the caller's `missing`. Identity
        // is handled by the callers (never a self-rate row).
        let mut rate_cache: BTreeMap<String, Option<Decimal>> = BTreeMap::new();
        let mut rate_for = |base: &str, missing: &mut Vec<String>| -> Option<Decimal> {
            let cached = match rate_cache.get(base) {
                Some(cached) => *cached,
                None => {
                    let row = journal
                        .latest_fx_rate(base, reference_currency, None)
                        .ok()
                        .flatten();
                    let rate = row
                        .as_ref()
                        .and_then(|r| Decimal::from_str_exact(&r.rate).ok())
                        // A nonpositive stored rate must not convert to a confident zero (6.6
                        // review) — folded into the named refusal.
                        .filter(|r| r.is_sign_positive() && !r.is_zero());
                    if rate.is_some() {
                        if let Some(row) = row {
                            if !rates_used.iter().any(|u| {
                                u.base_currency == row.base_currency
                                    && u.quote_currency == row.quote_currency
                            }) {
                                rates_used.push(row);
                            }
                        }
                    }
                    rate_cache.insert(base.to_string(), rate);
                    rate
                }
            };
            if cached.is_none() {
                let pair = format!("{base} → {reference_currency}");
                if !missing.contains(&pair) {
                    missing.push(pair);
                }
            }
            cached
        };

        // 2) Group per uppercased ticker; convert + checked-sum each holding's invested.
        let mut by_ticker: BTreeMap<String, TickerAcc> = BTreeMap::new();
        for h in &all_holdings {
            // Unparseable stored decimals are skipped defensively (unreachable through the
            // validated write path) — the `car_buckets_by_currency` posture.
            let (Ok(avg_cost), Ok(quantity)) = (
                Decimal::from_str_exact(&h.purchase_price),
                Decimal::from_str_exact(&h.quantity),
            ) else {
                continue;
            };
            let acc = by_ticker
                .entry(h.security_ticker.to_uppercase())
                .or_default();
            if !acc.started {
                acc.started = true;
                acc.invested = Some(Decimal::ZERO);
            }
            let currency = super::effective_currency(h, reference_currency);
            let native = avg_cost.checked_mul(quantity);
            let converted = if currency == reference_currency {
                native
            } else {
                match rate_for(&currency, &mut acc.missing) {
                    Some(rate) => native.and_then(|n| steadyinvest_core::risk::convert(n, rate)),
                    None => None,
                }
            };
            acc.invested = match (acc.invested, converted) {
                (Some(sum), Some(v)) => sum.checked_add(v),
                _ => None,
            };
        }

        // 3) The global denominator: the checked sum over tickers — None as soon as one security
        // could not convert (its pairs are named), never a partial total.
        let global_invested = by_ticker
            .values()
            .try_fold(Decimal::ZERO, |acc, t| acc.checked_add(t.invested?));

        // 4) Size classification per ticker (study join → latest entered sales → convert).
        let mut slot_small: Option<Decimal> = Some(Decimal::ZERO);
        let mut slot_medium: Option<Decimal> = Some(Decimal::ZERO);
        let mut slot_large: Option<Decimal> = Some(Decimal::ZERO);
        let mut unclassified: Vec<UnclassifiedRow> = Vec::new();
        for (ticker, acc) in &by_ticker {
            let study = self
                .study_id_for_ticker(ticker)
                .and_then(|sid| self.get_study(sid));
            let Some(study) = study else {
                unclassified.push(UnclassifiedRow {
                    ticker: ticker.clone(),
                    reason: UnclassifiedReason::NoStudy,
                });
                continue;
            };
            // The latest year with an entered sales value — the RAW study figure (no
            // `core::normalize` in a portfolio read; the engine path stays untouched). Only the
            // live `value` is read, never a pending provider divergence.
            let Some(sales) = study
                .years
                .iter()
                .rev()
                .find_map(|y| y.sales.value.map(|m| m.as_decimal()))
                // A nonpositive latest sales figure is not a usable classification input
                // (2026-07-03 review: it must not classify confidently as Small — the same
                // rule the boundaries and rates already follow).
                .filter(|s| *s > Decimal::ZERO)
            else {
                unclassified.push(UnclassifiedRow {
                    ticker: ticker.clone(),
                    reason: UnclassifiedReason::NoSales,
                });
                continue;
            };
            let native = study.native_currency.to_uppercase();
            let sales_ref = if native == reference_currency {
                Some(sales)
            } else {
                let mut missing = Vec::new();
                match rate_for(&native, &mut missing) {
                    Some(rate) => steadyinvest_core::risk::convert(sales, rate),
                    None => {
                        // NOT unioned into the denominator's missing set (2026-07-03 review):
                        // this pair blocks only the CLASSIFICATION — it is named on its own
                        // « non classé » row, never blamed for the absent shares.
                        unclassified.push(UnclassifiedRow {
                            ticker: ticker.clone(),
                            reason: UnclassifiedReason::MissingRate(
                                missing.into_iter().next().unwrap_or_default(),
                            ),
                        });
                        continue;
                    }
                }
            };
            let Some(sales_ref) = sales_ref else {
                // A checked conversion overflow — the study exists, sales exist, the converted
                // figure cannot be stated. Named as its OWN reason (2026-07-03 review: « chiffre
                // d'affaires indisponible » would be factually wrong here).
                unclassified.push(UnclassifiedRow {
                    ticker: ticker.clone(),
                    reason: UnclassifiedReason::Unconvertible,
                });
                continue;
            };
            let slot = match size_class(sales_ref, small_max, medium_max) {
                SizeClass::Small => &mut slot_small,
                SizeClass::Medium => &mut slot_medium,
                SizeClass::Large => &mut slot_large,
            };
            // The class total absents when a member's converted invested is absent — a class sum
            // missing one member is a partial passed off as the class.
            *slot = match (slot.take(), acc.invested) {
                (Some(sum), Some(v)) => sum.checked_add(v),
                _ => None,
            };
        }

        // 5) Assemble rows, largest invested first (absent last), ticker tiebreak — deterministic.
        for acc in by_ticker.values() {
            all_missing.extend(acc.missing.iter().cloned());
        }
        let mut rows: Vec<ConcentrationRow> = by_ticker
            .into_iter()
            .map(|(ticker, acc)| ConcentrationRow {
                share_pct: acc
                    .invested
                    .zip(global_invested)
                    .and_then(|(v, g)| share_pct(v, g)),
                ticker,
                invested: acc.invested,
                missing_pairs: acc.missing,
            })
            .collect();
        rows.sort_by(|a, b| match (a.invested, b.invested) {
            // Descending invested — the largest share reads first; absent rows sink to the end.
            (Some(x), Some(y)) => y.cmp(&x).then_with(|| a.ticker.cmp(&b.ticker)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.ticker.cmp(&b.ticker),
        });

        let slot = |invested: Option<Decimal>| SizeMixSlot {
            share_pct: invested
                .zip(global_invested)
                .and_then(|(v, g)| share_pct(v, g)),
        };
        JournalDiversification {
            rows,
            global_invested,
            small: slot(slot_small),
            medium: slot(slot_medium),
            large: slot(slot_large),
            unclassified,
            missing_pairs: all_missing.into_iter().collect(),
            rates_used,
            unavailable: false,
        }
    }
}
