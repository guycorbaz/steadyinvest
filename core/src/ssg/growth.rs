//! §1 Growth (spec §1): historical CAGRs by the endpoints method, recent quarterly % change,
//! and the estimated high/low EPS for the forecast horizon.
//!
//! The CAGR/projection helpers are public on purpose: the UI's numeric judgment-line entry
//! reuses them ("the judgment value can also be set numerically — same `core` function").

use super::types::{GrowthOutputs, JudgmentInputs, QuarterlyObservations};
use crate::method::FORECAST_HORIZON_YEARS;
use crate::normalize::{CanonicalFinancials, CanonicalYear, YearUsability};
use rust_decimal::{Decimal, MathematicalOps};

/// Endpoints compound annual growth rate, in percent.
///
/// `n = span_years` is the calendar-year span between the endpoint years (gaps compound
/// across). Spec §9 guard runs BEFORE any `powd`: `start > 0 && end > 0` (zero is neither
/// sign — ruled out explicitly) and `n ≥ 1`; otherwise `None` (unknown), never 0 — Spike C:
/// `checked_powd` does NOT protect against a negative base (it silently returns a
/// plausible-but-wrong real), the guard does.
pub fn endpoints_cagr_pct(start: Decimal, end: Decimal, span_years: u32) -> Option<Decimal> {
    if span_years == 0 || start <= Decimal::ZERO || end <= Decimal::ZERO {
        return None;
    }
    let ratio = end.checked_div(start)?;
    let exponent = Decimal::ONE.checked_div(Decimal::from(span_years))?;
    let factor = ratio.checked_powd(exponent)?;
    (factor - Decimal::ONE).checked_mul(Decimal::ONE_HUNDRED)
}

/// Exact integer-power growth projection: `base × (1 + growth_pct/100)^years`.
///
/// Integer `powd` is exact (Spike C); `checked_powd` still guards overflow on extreme inputs.
/// A growth below −100%/yr makes the factor negative — degraded to `None` (the same
/// Spike-C sign trap applies; a projection through an annihilated base is not a number).
pub fn project(base: Decimal, growth_pct: Decimal, years: u32) -> Option<Decimal> {
    let rate = growth_pct.checked_div(Decimal::ONE_HUNDRED)?;
    let factor = Decimal::ONE + rate;
    if factor < Decimal::ZERO {
        return None;
    }
    let grown = factor.checked_powd(Decimal::from(years))?;
    base.checked_mul(grown)
}

/// Recent quarterly % change: `(latest − year_ago) / year_ago × 100`.
/// Year-ago absent or ≤ 0 ⇒ `None` (recorded interpretation: a non-positive base has no
/// meaningful percent change).
fn quarterly_change_pct(latest: Option<Decimal>, year_ago: Option<Decimal>) -> Option<Decimal> {
    let latest = latest?;
    let year_ago = year_ago?;
    if year_ago <= Decimal::ZERO {
        return None;
    }
    (latest - year_ago)
        .checked_div(year_ago)?
        .checked_mul(Decimal::ONE_HUNDRED)
}

/// First and last usable years (spec §4 usability), if any.
fn usable_endpoints(financials: &CanonicalFinancials) -> Option<(&CanonicalYear, &CanonicalYear)> {
    let mut usable = financials
        .years
        .iter()
        .filter(|y| matches!(y.usability, YearUsability::Usable));
    let first = usable.next()?;
    let last = usable.next_back().unwrap_or(first);
    Some((first, last))
}

/// The most recent usable year, if any (projection base, §2 trend "recent" side).
pub(super) fn latest_usable(financials: &CanonicalFinancials) -> Option<&CanonicalYear> {
    financials
        .years
        .iter()
        .rev()
        .find(|y| matches!(y.usability, YearUsability::Usable))
}

/// Endpoints CAGR of one canonical field over the usable years.
fn series_cagr_pct(
    financials: &CanonicalFinancials,
    field: impl Fn(&CanonicalYear) -> Option<Decimal>,
) -> Option<Decimal> {
    let (first, last) = usable_endpoints(financials)?;
    let span = u32::try_from(last.year - first.year).ok()?;
    endpoints_cagr_pct(field(first)?, field(last)?, span)
}

pub(super) fn compute(
    financials: &CanonicalFinancials,
    judgment: &JudgmentInputs,
    observations: &QuarterlyObservations,
) -> GrowthOutputs {
    // Direct judgment wins; else derive from the latest usable EPS and the judged EPS growth
    // (same direct-wins pattern as 1.7's PTP gross-up). Low EPS is direct-only in v1.
    let derived_high_eps = || {
        let base = latest_usable(financials)?.eps?;
        let growth = judgment.projected_eps_growth_pct?;
        project(base, growth, FORECAST_HORIZON_YEARS)
    };
    GrowthOutputs {
        sales_cagr_pct: series_cagr_pct(financials, |y| y.sales),
        eps_cagr_pct: series_cagr_pct(financials, |y| y.eps),
        quarterly_sales_change_pct: quarterly_change_pct(
            observations.latest_quarter_sales,
            observations.year_ago_quarter_sales,
        ),
        quarterly_eps_change_pct: quarterly_change_pct(
            observations.latest_quarter_eps,
            observations.year_ago_quarter_eps,
        ),
        estimated_high_eps: judgment.estimated_high_eps.or_else(derived_high_eps),
        estimated_low_eps: judgment.estimated_low_eps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(mantissa: i64, scale: u32) -> Decimal {
        Decimal::new(mantissa, scale)
    }

    #[test]
    fn endpoints_cagr_is_exact_on_exact_roots() {
        // 1000 → 1464.1 over 4 years: ratio 1.4641 = 1.1^4, so the true CAGR is exactly 10%.
        // The fractional powd carries ≤ ~1e-27 relative error (Spike C) — assert within 1e-20.
        let cagr = endpoints_cagr_pct(d(1000, 0), d(14641, 1), 4).expect("valid CAGR inputs");
        let diff = (cagr - d(10, 0)).abs();
        assert!(
            diff < d(1, 20),
            "CAGR of 1000→1464.1 over 4y must be 10% within 1e-20, got {cagr}"
        );
    }

    #[test]
    fn endpoints_cagr_degenerate_bases_are_unknown_never_zero() {
        // Spec §9: n = 0, start ≤ 0, end ≤ 0, sign-crossing ⇒ unknown.
        assert_eq!(endpoints_cagr_pct(d(1, 0), d(2, 0), 0), None, "n = 0");
        assert_eq!(
            endpoints_cagr_pct(Decimal::ZERO, d(2, 0), 3),
            None,
            "start = 0"
        );
        assert_eq!(endpoints_cagr_pct(d(-1, 0), d(2, 0), 3), None, "start < 0");
        assert_eq!(
            endpoints_cagr_pct(d(1, 0), Decimal::ZERO, 3),
            None,
            "end = 0"
        );
        assert_eq!(endpoints_cagr_pct(d(1, 0), d(-2, 0), 3), None, "end < 0");
        assert_eq!(
            endpoints_cagr_pct(d(-1, 0), d(-2, 0), 3),
            None,
            "both negative"
        );
    }

    #[test]
    fn project_integer_powers_are_exact() {
        // 1.4641 × 1.1^5 = 2.357947691 exactly (integer powd is exact, Spike C).
        assert_eq!(
            project(d(14641, 4), d(10, 0), 5),
            Some(d(2357947691, 9)),
            "integer-power projection must be exact"
        );
        // Zero years: factor^0 = 1.
        assert_eq!(project(d(3, 0), d(10, 0), 0), Some(d(3, 0)));
        // Growth of exactly −100%/yr annihilates the base; below it is unknown, never a
        // sign-flipping number.
        assert_eq!(project(d(3, 0), d(-100, 0), 5), Some(Decimal::ZERO));
        assert_eq!(project(d(3, 0), d(-150, 0), 5), None);
    }

    #[test]
    fn quarterly_change_needs_a_positive_year_ago_base() {
        assert_eq!(
            quarterly_change_pct(Some(d(5, 0)), Some(d(4, 0))),
            Some(d(25, 0))
        );
        assert_eq!(
            quarterly_change_pct(Some(d(5, 0)), Some(Decimal::ZERO)),
            None
        );
        assert_eq!(quarterly_change_pct(Some(d(5, 0)), Some(d(-4, 0))), None);
        assert_eq!(quarterly_change_pct(Some(d(5, 0)), None), None);
        assert_eq!(quarterly_change_pct(None, Some(d(4, 0))), None);
    }
}
