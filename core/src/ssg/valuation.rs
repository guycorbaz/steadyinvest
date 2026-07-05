//! §3 Price–earnings history (spec §3): per-year high/low P/E, % payout, % high yield over
//! the last 5 usable years; their averages; current P/E from TTM EPS; relative value.
//!
//! §9 exclusions: per-year `eps ≤ 0` ⇒ that year's P/Es and payout `unknown`
//! (`negative_or_zero_denominator`), excluded from the averages; `low_price ≤ 0` ⇒ that
//! year's high yield `unknown`, excluded from its average; `TTM EPS ≤ 0` ⇒ current P/E and
//! relative value `unknown` (and `relative_value_high` is never raised on unknown).

use super::types::{
    CalcFinding, JudgmentInputs, QuarterlyObservations, ValuationOutputs, YearValuation,
};
use super::{last_usable_window, mean};
use crate::method::{pe_axis_max, pe_axis_min};
use crate::normalize::{CanonicalFinancials, PlausibilityKey};
use rust_decimal::Decimal;

/// `out_of_bounds_ratio` when a computed P/E leaves `[pe_axis_min(), pe_axis_max()]`
/// (the chart axis bound, spec §3) — the value is still reported.
fn check_pe_bound(
    value: Option<Decimal>,
    context: &'static str,
    year: Option<i32>,
    findings: &mut Vec<CalcFinding>,
) {
    if let Some(pe) = value
        && (pe < pe_axis_min() || pe > pe_axis_max())
    {
        findings.push(CalcFinding {
            key: PlausibilityKey::OutOfBoundsRatio,
            year,
            context,
        });
    }
}

pub(super) fn compute(
    financials: &CanonicalFinancials,
    judgment: &JudgmentInputs,
    observations: &QuarterlyObservations,
    findings: &mut Vec<CalcFinding>,
) -> ValuationOutputs {
    let window = last_usable_window(financials);

    let per_year: Vec<YearValuation> = window
        .iter()
        .map(|y| {
            // Load-bearing fields are present in usable years; the guards below handle the
            // non-positive cases (§9).
            let eps = y.eps.filter(|e| {
                let ok = *e > Decimal::ZERO;
                if !ok {
                    findings.push(CalcFinding {
                        key: PlausibilityKey::NegativeOrZeroDenominator,
                        year: Some(y.year),
                        context: "eps",
                    });
                }
                ok
            });
            let high_pe = eps.and_then(|e| y.high_price?.checked_div(e));
            check_pe_bound(high_pe, "high_pe", Some(y.year), findings);
            let low_pe = eps.and_then(|e| y.low_price?.checked_div(e));
            check_pe_bound(low_pe, "low_pe", Some(y.year), findings);
            let payout_pct = eps.and_then(|e| {
                y.dividend_per_share?
                    .checked_div(e)?
                    .checked_mul(Decimal::ONE_HUNDRED)
            });
            let low_price = y.low_price.filter(|p| {
                let ok = *p > Decimal::ZERO;
                if !ok {
                    findings.push(CalcFinding {
                        key: PlausibilityKey::NegativeOrZeroDenominator,
                        year: Some(y.year),
                        context: "low_price",
                    });
                }
                ok
            });
            let high_yield_pct = low_price.and_then(|p| {
                y.dividend_per_share?
                    .checked_div(p)?
                    .checked_mul(Decimal::ONE_HUNDRED)
            });
            YearValuation {
                year: y.year,
                high_pe,
                low_pe,
                payout_pct,
                high_yield_pct,
            }
        })
        .collect();

    let collect = |pick: fn(&YearValuation) -> Option<Decimal>| -> Vec<Decimal> {
        per_year.iter().filter_map(pick).collect()
    };
    let avg_high_pe = mean(&collect(|v| v.high_pe));
    let avg_low_pe = mean(&collect(|v| v.low_pe));
    let avg_pe = avg_high_pe
        .zip(avg_low_pe)
        .and_then(|(h, l)| (h + l).checked_div(Decimal::TWO));
    let avg_payout_pct = mean(&collect(|v| v.payout_pct));
    let avg_high_yield_pct = mean(&collect(|v| v.high_yield_pct));
    // Spec §3 "average low price" carries no exclusion rule: mean of the window's low prices.
    let avg_low_price = mean(
        &window
            .iter()
            .filter_map(|y| y.low_price)
            .collect::<Vec<_>>(),
    );

    // Current P/E = current_price / Σ(last 4 quarterly EPS); TTM ≤ 0 ⇒ unknown (spec §9).
    // The TTM denominator finding mirrors the per-year eps rule (recorded interpretation).
    // Checked fold: an overflowing sum yields `unknown`, never a panic (house `mean` pattern).
    let ttm_eps = observations.ttm_quarterly_eps.and_then(|q| {
        q.iter()
            .try_fold(Decimal::ZERO, |acc, e| acc.checked_add(*e))
    });
    let positive_ttm = ttm_eps.filter(|t| {
        let ok = *t > Decimal::ZERO;
        if !ok {
            findings.push(CalcFinding {
                key: PlausibilityKey::NegativeOrZeroDenominator,
                year: None,
                context: "ttm_eps",
            });
        }
        ok
    });
    let current_pe = positive_ttm.and_then(|t| judgment.current_price?.checked_div(t));
    check_pe_bound(current_pe, "current_pe", None, findings);
    let relative_value_pct =
        current_pe.and_then(|pe| pe.checked_div(avg_pe?)?.checked_mul(Decimal::ONE_HUNDRED));

    ValuationOutputs {
        per_year,
        avg_high_pe,
        avg_low_pe,
        avg_pe,
        avg_payout_pct,
        avg_high_yield_pct,
        avg_low_price,
        ttm_eps,
        current_pe,
        relative_value_pct,
    }
}
