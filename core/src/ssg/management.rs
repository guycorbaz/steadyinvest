//! §2 Management (spec §2): per-year % pre-tax profit on sales and % earned on equity,
//! their 5-year averages and trends.
//!
//! PTP arrives already canonicalized from Story 1.7 (§2 gross-up done upstream) — this module
//! only forms the ratios. §9 exclusions: a non-positive denominator makes the year's ratio
//! `unknown` + `negative_or_zero_denominator`, excluded from the average/trend (the `sales ≤ 0`
//! extension is a recorded interpretation — the spec defines the key for EPS/book-value
//! denominators only).

use super::types::{CalcFinding, ManagementOutputs, Trend, YearRatios};
use super::{last_usable_window, mean};
use crate::method::{ptp_roe_bound_pct, trend_even_band_pp};
use crate::normalize::{CanonicalFinancials, CanonicalYear, PlausibilityKey};
use rust_decimal::Decimal;

/// `numerator / denominator × 100` with the §9 denominator rule: denominator present and ≤ 0
/// ⇒ `None` + `negative_or_zero_denominator` on `denominator_name`; either input absent ⇒
/// `None` without a finding (missing data is insufficiency, not implausibility).
fn ratio_pct(
    numerator: Option<Decimal>,
    denominator: Option<Decimal>,
    denominator_name: &'static str,
    year: i32,
    findings: &mut Vec<CalcFinding>,
) -> Option<Decimal> {
    let denominator = denominator?;
    if denominator <= Decimal::ZERO {
        findings.push(CalcFinding {
            key: PlausibilityKey::NegativeOrZeroDenominator,
            year: Some(year),
            context: denominator_name,
        });
        return None;
    }
    numerator?
        .checked_div(denominator)?
        .checked_mul(Decimal::ONE_HUNDRED)
}

/// `out_of_bounds_ratio` when a computed PTP/ROE leaves `[−ptp_roe_bound_pct(), +bound]`
/// (spec §3) — the value is still reported; plausibility never blocks.
fn check_bound(
    value: Option<Decimal>,
    context: &'static str,
    year: i32,
    findings: &mut Vec<CalcFinding>,
) {
    if let Some(v) = value {
        if v.abs() > ptp_roe_bound_pct() {
            findings.push(CalcFinding {
                key: PlausibilityKey::OutOfBoundsRatio,
                year: Some(year),
                context,
            });
        }
    }
}

/// Trend of the latest usable year's ratio vs the 5-yr average (recorded interpretation of
/// spec §2 "comparison of recent years to the 5-year average"): within
/// ±`trend_even_band_pp()` ⇒ even; beyond ⇒ up/down. Either side unknown ⇒ `None` — the
/// trend flags are never raised on an unknown metric.
fn trend(latest: Option<Decimal>, average: Option<Decimal>) -> Option<Trend> {
    let diff = latest? - average?;
    let band = trend_even_band_pp();
    Some(if diff.abs() <= band {
        Trend::Even
    } else if diff > band {
        Trend::Up
    } else {
        Trend::Down
    })
}

pub(super) fn compute(
    financials: &CanonicalFinancials,
    findings: &mut Vec<CalcFinding>,
) -> ManagementOutputs {
    let per_year: Vec<YearRatios> = financials
        .years
        .iter()
        .map(|y| {
            let ptp_pct = ratio_pct(y.pre_tax_profit, y.sales, "sales", y.year, findings);
            check_bound(ptp_pct, "ptp_pct", y.year, findings);
            let roe_pct = ratio_pct(
                y.eps,
                y.book_value_per_share,
                "book_value_per_share",
                y.year,
                findings,
            );
            check_bound(roe_pct, "roe_pct", y.year, findings);
            YearRatios {
                year: y.year,
                ptp_pct,
                roe_pct,
            }
        })
        .collect();

    let window = last_usable_window(financials);
    let ratio_of = |year: &CanonicalYear, pick: fn(&YearRatios) -> Option<Decimal>| {
        per_year.iter().find(|r| r.year == year.year).and_then(pick)
    };
    let window_values = |pick: fn(&YearRatios) -> Option<Decimal>| -> Vec<Decimal> {
        window.iter().filter_map(|y| ratio_of(y, pick)).collect()
    };

    let avg_ptp_pct = mean(&window_values(|r| r.ptp_pct));
    let avg_roe_pct = mean(&window_values(|r| r.roe_pct));
    let latest = window.last();
    let latest_ptp_pct = latest.and_then(|y| ratio_of(y, |r| r.ptp_pct));
    let latest_roe_pct = latest.and_then(|y| ratio_of(y, |r| r.roe_pct));

    ManagementOutputs {
        per_year,
        avg_ptp_pct,
        avg_roe_pct,
        latest_ptp_pct,
        latest_roe_pct,
        ptp_trend: trend(latest_ptp_pct, avg_ptp_pct),
        roe_trend: trend(latest_roe_pct, avg_roe_pct),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(mantissa: i64, scale: u32) -> Decimal {
        Decimal::new(mantissa, scale)
    }

    #[test]
    fn trend_band_boundaries_are_inclusive_even() {
        let avg = Some(d(10, 0));
        // Exactly ±0.5 pp is even (≤ comparator, spec §2 "even" band).
        assert_eq!(trend(Some(d(105, 1)), avg), Some(Trend::Even), "+0.5 pp");
        assert_eq!(trend(Some(d(95, 1)), avg), Some(Trend::Even), "-0.5 pp");
        assert_eq!(trend(Some(d(1051, 2)), avg), Some(Trend::Up), "+0.51 pp");
        assert_eq!(trend(Some(d(949, 2)), avg), Some(Trend::Down), "-0.51 pp");
        assert_eq!(trend(None, avg), None, "unknown latest ⇒ no trend");
        assert_eq!(
            trend(Some(d(10, 0)), None),
            None,
            "unknown average ⇒ no trend"
        );
    }

    #[test]
    fn non_positive_denominator_is_unknown_with_finding() {
        let mut findings = Vec::new();
        assert_eq!(
            ratio_pct(
                Some(d(10, 0)),
                Some(Decimal::ZERO),
                "sales",
                2024,
                &mut findings
            ),
            None
        );
        assert_eq!(
            ratio_pct(
                Some(d(10, 0)),
                Some(d(-5, 0)),
                "book_value_per_share",
                2025,
                &mut findings
            ),
            None
        );
        assert_eq!(
            findings,
            vec![
                CalcFinding {
                    key: PlausibilityKey::NegativeOrZeroDenominator,
                    year: Some(2024),
                    context: "sales",
                },
                CalcFinding {
                    key: PlausibilityKey::NegativeOrZeroDenominator,
                    year: Some(2025),
                    context: "book_value_per_share",
                },
            ]
        );
        // Missing inputs are insufficiency, not implausibility: no finding.
        let mut quiet = Vec::new();
        assert_eq!(
            ratio_pct(None, Some(d(5, 0)), "sales", 2024, &mut quiet),
            None
        );
        assert_eq!(
            ratio_pct(Some(d(5, 0)), None, "sales", 2024, &mut quiet),
            None
        );
        assert!(
            quiet.is_empty(),
            "absent inputs must not raise findings: {quiet:?}"
        );
    }

    #[test]
    fn out_of_bounds_ratio_reports_value_and_finding() {
        let mut findings = Vec::new();
        check_bound(Some(d(1001, 1)), "ptp_pct", 2024, &mut findings); // 100.1 > 100
        check_bound(Some(d(-1001, 1)), "roe_pct", 2024, &mut findings); // |−100.1| > 100
        check_bound(Some(d(100, 0)), "ptp_pct", 2024, &mut findings); // exactly 100: in bounds
        assert_eq!(
            findings.len(),
            2,
            "±100 is inside the bound, beyond is not: {findings:?}"
        );
    }
}
