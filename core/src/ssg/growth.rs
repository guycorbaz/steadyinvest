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

/// The least-squares seed band for the estimated future EPS (§1 chart pre-configuration,
/// issue #121). Central fit plus a symmetric log-space residual band (`fit × exp(±σ)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpsSeedBand {
    /// Central least-squares projection at the horizon (`exp(â + b̂·x_h)`).
    pub fit: Decimal,
    /// Upper band: `fit × exp(+σ)`, σ = log-space residual spread.
    pub high: Decimal,
    /// Lower band: `fit × exp(−σ)`.
    pub low: Decimal,
}

/// Ordinary least-squares fit of `ln(EPS)` against the fiscal year over `points`
/// (compound-growth / semi-log — matching the §1 chart's log scale), projected `horizon_years`
/// past the latest supplied year, with a symmetric band at `exp(±σ)` where σ is the population
/// standard deviation of the log-space residuals.
///
/// **Display-only helper (issue #121), deliberately NOT wired into [`compute`]:** it seeds the
/// draggable §1 est-high / est-low handles at a statistical starting point, but an *untouched*
/// seed must never flow into §4/verdict — that stays the user's judgment (the "never guess a
/// number" invariant / FR33). Public for the same reason as [`project`]/[`endpoints_cagr_pct`]:
/// the UI reuses `core` math rather than re-deriving it.
///
/// `None` (unknown — never a guessed number, spec §9 discipline) when fewer than two points have
/// `eps > 0` (ln is undefined at ≤ 0; a line needs two points), the years are all identical
/// (`Σ(x−x̄)² = 0` — no trend), or any checked Decimal op overflows.
pub fn least_squares_log_eps_band(
    points: &[(i32, Decimal)],
    horizon_years: u32,
) -> Option<EpsSeedBand> {
    // Only strictly-positive EPS can be logged — a compound-growth trend is undefined through ≤ 0.
    let mut obs: Vec<(Decimal, Decimal)> = Vec::with_capacity(points.len());
    for (year, eps) in points {
        if *eps > Decimal::ZERO {
            obs.push((Decimal::from(*year), eps.checked_ln()?));
        }
    }
    if obs.len() < 2 {
        return None;
    }
    let n = Decimal::from(obs.len() as u64);

    let mut sum_x = Decimal::ZERO;
    let mut sum_y = Decimal::ZERO;
    for (x, y) in &obs {
        sum_x = sum_x.checked_add(*x)?;
        sum_y = sum_y.checked_add(*y)?;
    }
    let x_bar = sum_x.checked_div(n)?;
    let y_bar = sum_y.checked_div(n)?;

    let mut sxx = Decimal::ZERO;
    let mut sxy = Decimal::ZERO;
    for (x, y) in &obs {
        let dx = x.checked_sub(x_bar)?;
        sxx = sxx.checked_add(dx.checked_mul(dx)?)?;
        sxy = sxy.checked_add(dx.checked_mul(y.checked_sub(y_bar)?)?)?;
    }
    if sxx <= Decimal::ZERO {
        return None; // every year identical — no trend to fit
    }
    let slope = sxy.checked_div(sxx)?;
    let intercept = y_bar.checked_sub(slope.checked_mul(x_bar)?)?;

    // Population σ of the log-space residuals (÷n, not ÷(n−2): a two-point fit then gives σ = 0,
    // i.e. a zero-width band, rather than a division by zero).
    let mut ss_res = Decimal::ZERO;
    for (x, y) in &obs {
        let fitted = intercept.checked_add(slope.checked_mul(*x)?)?;
        let r = y.checked_sub(fitted)?;
        ss_res = ss_res.checked_add(r.checked_mul(r)?)?;
    }
    let sigma = ss_res.checked_div(n)?.sqrt()?;

    // Project to the horizon year past the latest supplied year (the chart's forecast x).
    let last_year = points.iter().map(|(y, _)| *y).max()?;
    let x_h = Decimal::from(last_year).checked_add(Decimal::from(horizon_years))?;
    let ly_h = intercept.checked_add(slope.checked_mul(x_h)?)?;

    Some(EpsSeedBand {
        fit: ly_h.checked_exp()?,
        high: ly_h.checked_add(sigma)?.checked_exp()?,
        low: ly_h.checked_sub(sigma)?.checked_exp()?,
    })
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

    fn approx(a: Decimal, b: Decimal, rel: f64) -> bool {
        use rust_decimal::prelude::ToPrimitive;
        let (a, b) = (a.to_f64().unwrap(), b.to_f64().unwrap());
        (a - b).abs() <= rel * b.abs().max(1e-9)
    }

    #[test]
    fn least_squares_seeds_an_exact_exponential_series() {
        // EPS = 4 · 1.5^k over 2021..=2025: ln(EPS) is exactly linear in the year, so the fit
        // projects (within the ln/exp approximation) to 4 · 1.5^(4 + horizon) and the residual
        // band is ~zero-width (the points sit on the line).
        let points: Vec<(i32, Decimal)> = (0..5)
            .map(|k| (2021 + k, project(d(4, 0), d(50, 0), k as u32).unwrap()))
            .collect();
        let band = least_squares_log_eps_band(&points, 5).expect("valid trend");
        // Central projection lands at 4 · 1.5^(4+5) = 4 · 1.5^9.
        let expected = project(d(4, 0), d(50, 0), 9).unwrap();
        assert!(
            approx(band.fit, expected, 1e-6),
            "fit {} must project the exact trend to {expected}",
            band.fit
        );
        // A trend with no scatter has an essentially zero-width band.
        assert!(
            band.low <= band.fit && band.fit <= band.high,
            "band is ordered"
        );
        assert!(
            approx(band.high, band.low, 1e-6),
            "a scatter-free series has a ~zero-width band, got [{}, {}]",
            band.low,
            band.high
        );
    }

    #[test]
    fn least_squares_band_widens_with_scatter_and_stays_ordered() {
        // A noisy but upward series: the band must straddle the central fit (low < fit < high).
        let points = vec![
            (2021, d(40, 1)),  // 4.0
            (2022, d(70, 1)),  // 7.0
            (2023, d(55, 1)),  // 5.5
            (2024, d(90, 1)),  // 9.0
            (2025, d(110, 1)), // 11.0
        ];
        let band = least_squares_log_eps_band(&points, 5).expect("valid trend");
        assert!(
            band.low < band.fit && band.fit < band.high,
            "a scattered series yields a strictly-widening band, got [{}, {}, {}]",
            band.low,
            band.fit,
            band.high
        );
    }

    #[test]
    fn least_squares_ignores_non_positive_eps_and_needs_two_points() {
        // ln is undefined at ≤ 0 — those years are dropped, not guessed.
        let with_bad = vec![(2021, d(4, 0)), (2022, Decimal::ZERO), (2023, d(9, 0))];
        assert!(
            least_squares_log_eps_band(&with_bad, 5).is_some(),
            "two positive points survive the ≤0 filter"
        );
        // Only one usable point → unknown (a line needs two).
        let one = vec![(2021, d(4, 0)), (2022, d(-1, 0))];
        assert_eq!(
            least_squares_log_eps_band(&one, 5),
            None,
            "one point → None"
        );
        assert_eq!(least_squares_log_eps_band(&[], 5), None, "empty → None");
    }

    #[test]
    fn least_squares_identical_years_have_no_trend() {
        let flat_x = vec![(2021, d(4, 0)), (2021, d(9, 0))];
        assert_eq!(
            least_squares_log_eps_band(&flat_x, 5),
            None,
            "Σ(x−x̄)² = 0 → no trend, never a guessed slope"
        );
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
