//! §5 Five-year potential (spec §5): present yield, average annual EPS/dividend over the
//! horizon, average yield, projected appreciation and the projected total annualised return.
//!
//! Gate (recorded interpretation — no §9 row covers it): `current_price` absent or ≤ 0 ⇒
//! every §5 output is unknown (each formula divides by, or is relative to, the current
//! price). The annualised-return fractional `powd` runs only behind the §9 positive-base
//! guard (`current > 0` from the gate, `forecast_high > 0` checked here).

use super::growth::{latest_usable, project};
use super::types::{JudgmentInputs, ReturnOutputs, RiskRewardOutputs, ValuationOutputs};
use crate::method::FORECAST_HORIZON_YEARS;
use crate::normalize::CanonicalFinancials;
use rust_decimal::{Decimal, MathematicalOps};

/// Mean of the projected yearly EPS over the horizon, from the latest usable EPS and the
/// judged EPS growth (exact integer powers; recorded interpretation: spec §5 allows the
/// middle-year value as an alternative — the mean is the deterministic pick).
fn avg_annual_eps(financials: &CanonicalFinancials, judgment: &JudgmentInputs) -> Option<Decimal> {
    let base = latest_usable(financials)?.eps?;
    let growth_pct = judgment.projected_eps_growth_pct?;
    let mut sum = Decimal::ZERO;
    for year in 1..=FORECAST_HORIZON_YEARS {
        sum = sum.checked_add(project(base, growth_pct, year)?)?;
    }
    sum.checked_div(Decimal::from(FORECAST_HORIZON_YEARS))
}

/// Annualised appreciation percent: `((forecast_high / current)^(1/horizon) − 1) × 100`.
/// §9 guard BEFORE the fractional `powd`: both prices must be strictly positive.
fn annualized_appreciation_pct(current: Decimal, forecast_high: Decimal) -> Option<Decimal> {
    if current <= Decimal::ZERO || forecast_high <= Decimal::ZERO {
        return None;
    }
    let ratio = forecast_high.checked_div(current)?;
    let exponent = Decimal::ONE.checked_div(Decimal::from(FORECAST_HORIZON_YEARS))?;
    let factor = ratio.checked_powd(exponent)?;
    (factor - Decimal::ONE).checked_mul(Decimal::ONE_HUNDRED)
}

pub(super) fn compute(
    financials: &CanonicalFinancials,
    valuation: &ValuationOutputs,
    risk_reward: &RiskRewardOutputs,
    judgment: &JudgmentInputs,
) -> ReturnOutputs {
    let unknown = ReturnOutputs {
        present_yield_pct: None,
        avg_annual_eps: None,
        avg_annual_dividend: None,
        avg_yield_pct: None,
        projected_appreciation_pct: None,
        projected_annualized_appreciation_pct: None,
        projected_total_annualized_return_pct: None,
    };
    let Some(current) = judgment.current_price else {
        return unknown;
    };
    if current <= Decimal::ZERO {
        return unknown;
    }

    let over_current_pct = |value: Decimal| -> Option<Decimal> {
        value
            .checked_div(current)?
            .checked_mul(Decimal::ONE_HUNDRED)
    };

    let present_yield_pct = judgment
        .present_full_year_dividend
        .and_then(over_current_pct);
    let avg_annual_eps = avg_annual_eps(financials, judgment);
    let avg_annual_dividend = avg_annual_eps
        .zip(valuation.avg_payout_pct)
        .and_then(|(eps, payout)| eps.checked_mul(payout)?.checked_div(Decimal::ONE_HUNDRED));
    let avg_yield_pct = avg_annual_dividend.and_then(over_current_pct);
    let projected_appreciation_pct = risk_reward
        .forecast_high
        .and_then(|high| over_current_pct(high - current));
    let projected_annualized_appreciation_pct = risk_reward
        .forecast_high
        .and_then(|high| annualized_appreciation_pct(current, high));
    // Both percent terms before summing (spec §5: annualised appreciation PLUS average yield,
    // gross dividend on the study side).
    let projected_total_annualized_return_pct = projected_annualized_appreciation_pct
        .zip(avg_yield_pct)
        .and_then(|(appreciation, yield_pct)| appreciation.checked_add(yield_pct));

    ReturnOutputs {
        present_yield_pct,
        avg_annual_eps,
        avg_annual_dividend,
        avg_yield_pct,
        projected_appreciation_pct,
        projected_annualized_appreciation_pct,
        projected_total_annualized_return_pct,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(mantissa: i64, scale: u32) -> Decimal {
        Decimal::new(mantissa, scale)
    }

    #[test]
    fn annualized_appreciation_guards_non_positive_bases_before_powd() {
        assert_eq!(annualized_appreciation_pct(Decimal::ZERO, d(45, 0)), None);
        assert_eq!(annualized_appreciation_pct(d(-1, 0), d(45, 0)), None);
        assert_eq!(annualized_appreciation_pct(d(18, 0), Decimal::ZERO), None);
        assert_eq!(annualized_appreciation_pct(d(18, 0), d(-45, 0)), None);
    }

    #[test]
    fn annualized_appreciation_is_exact_on_exact_roots() {
        // 18 → 44.78976 = 18 × 1.2^5, so the annualised rate is exactly 20%.
        let pct = annualized_appreciation_pct(d(18, 0), d(4478976, 5)).expect("positive bases");
        let diff = (pct - d(20, 0)).abs();
        assert!(
            diff < d(1, 20),
            "18→44.78976 over 5y must annualise to 20% within 1e-20, got {pct}"
        );
    }
}
