//! §4 Risk & reward (spec §4): forecast high/low, the Buy/Neutral/Sell thirds, the
//! present-price zone and the upside/downside ratio.
//!
//! §9 rules: option (d) is not selectable when the average high yield is ≤ 0 or unknown;
//! `forecast_high ≤ forecast_low` is a degenerate range ⇒ zones and U/D unknown, never
//! inverted bands; U/D with a non-positive denominator is **undefined** (a typed state,
//! never a number); a selected forecast low strictly above the current price raises
//! `low_price_above_current`.

use super::types::{
    CalcFinding, ForecastLowOption, GrowthOutputs, JudgmentInputs, RiskRewardOutputs,
    UpsideDownside, ValuationOutputs, Zone, ZoneBounds,
};
use crate::normalize::PlausibilityKey;
use rust_decimal::Decimal;

/// The §4 forecast low per the user-selected option. Each option degrades to `None` when its
/// input is unknown; option (d) additionally requires a **positive** average high yield
/// (spec §9 — division by a non-positive yield is not selectable).
fn forecast_low(
    judgment: &JudgmentInputs,
    growth: &GrowthOutputs,
    valuation: &ValuationOutputs,
) -> Option<Decimal> {
    match judgment.forecast_low_option {
        ForecastLowOption::AvgLowPeTimesEps => judgment
            .judged_avg_low_pe?
            .checked_mul(growth.estimated_low_eps?),
        ForecastLowOption::AvgLowPriceLast5y => valuation.avg_low_price,
        ForecastLowOption::RecentSevereLow => judgment.recent_severe_low,
        ForecastLowOption::DividendSupported => {
            let yield_pct = valuation.avg_high_yield_pct?;
            if yield_pct <= Decimal::ZERO {
                return None;
            }
            let yield_fraction = yield_pct.checked_div(Decimal::ONE_HUNDRED)?;
            judgment
                .present_full_year_dividend?
                .checked_div(yield_fraction)
        }
    }
}

/// Zone bounds as exact thirds of a positive range. `forecast_high ≤ forecast_low` ⇒ `None`
/// (degenerate — never inverted bands). The `range/3` quotient is genuinely non-terminating
/// for most ranges and truncates at 28 significant digits — deterministic, display rounding
/// is not this module's business (spec §8).
fn zone_bounds(low: Decimal, high: Decimal) -> Option<ZoneBounds> {
    if high <= low {
        return None;
    }
    let third = (high - low).checked_div(Decimal::from(3u32))?;
    let bounds = ZoneBounds {
        forecast_low: low,
        buy_top: low + third,
        neutral_top: low + third + third,
        forecast_high: high,
    };
    // NFR-C3 by construction: a range within ~1e-28 of zero can round to zero-width thirds at
    // `Decimal`'s precision limit — that is degenerate too, never an unordered band set.
    let ordered = bounds.forecast_low < bounds.buy_top
        && bounds.buy_top < bounds.neutral_top
        && bounds.neutral_top < bounds.forecast_high;
    ordered.then_some(bounds)
}

/// The zone the current price falls in, with the spec's normative interval comparators:
/// Buy `[low, buy_top]`, Neutral `(buy_top, neutral_top]`, Sell `(neutral_top, high]`.
/// Outside `[low, high]` ⇒ `None` (recorded interpretation — the spec defines zones only
/// over the range).
fn present_zone(bounds: &ZoneBounds, current: Decimal) -> Option<Zone> {
    if current < bounds.forecast_low || current > bounds.forecast_high {
        return None;
    }
    Some(if current <= bounds.buy_top {
        Zone::Buy
    } else if current <= bounds.neutral_top {
        Zone::Neutral
    } else {
        Zone::Sell
    })
}

pub(super) fn compute(
    growth: &GrowthOutputs,
    valuation: &ValuationOutputs,
    judgment: &JudgmentInputs,
    findings: &mut Vec<CalcFinding>,
) -> RiskRewardOutputs {
    let forecast_high = judgment
        .judged_avg_high_pe
        .zip(growth.estimated_high_eps)
        .and_then(|(pe, eps)| pe.checked_mul(eps));
    let forecast_low = forecast_low(judgment, growth, valuation);
    let current = judgment.current_price;

    // §4 constraint check: a selected forecast low strictly above the current price violates
    // "forecast low ≤ current price" (equality allowed) — study-level finding, value reported.
    if let (Some(low), Some(price)) = (forecast_low, current)
        && low > price
    {
        findings.push(CalcFinding {
            key: PlausibilityKey::LowPriceAboveCurrent,
            year: None,
            context: "forecast_low",
        });
    }

    let zones = forecast_low
        .zip(forecast_high)
        .and_then(|(low, high)| zone_bounds(low, high));
    let present_price_zone = zones
        .as_ref()
        .zip(current)
        .and_then(|(bounds, price)| present_zone(bounds, price));

    // U/D: degenerate/missing inputs ⇒ Unknown; denominator ≤ 0 ⇒ Undefined (spec §9);
    // else the ratio.
    let upside_downside = match (zones.as_ref(), current) {
        (Some(bounds), Some(price)) => {
            let denominator = price - bounds.forecast_low;
            if denominator <= Decimal::ZERO {
                UpsideDownside::Undefined
            } else {
                match (bounds.forecast_high - price).checked_div(denominator) {
                    Some(ratio) => UpsideDownside::Ratio(ratio),
                    None => UpsideDownside::Unknown,
                }
            }
        }
        _ => UpsideDownside::Unknown,
    };

    RiskRewardOutputs {
        forecast_high,
        forecast_low,
        zones,
        present_price_zone,
        upside_downside,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(mantissa: i64, scale: u32) -> Decimal {
        Decimal::new(mantissa, scale)
    }

    #[test]
    fn zone_intervals_use_the_normative_comparators() {
        // Range 12.6 → 45 with estimated values chosen so the thirds terminate:
        // range 32.4, third 10.8 ⇒ Buy [12.6, 23.4], Neutral (23.4, 34.2], Sell (34.2, 45].
        let bounds = zone_bounds(d(126, 1), d(45, 0)).expect("positive range");
        assert_eq!(bounds.buy_top, d(234, 1));
        assert_eq!(bounds.neutral_top, d(342, 1));
        assert_eq!(
            present_zone(&bounds, d(126, 1)),
            Some(Zone::Buy),
            "low edge is Buy"
        );
        assert_eq!(
            present_zone(&bounds, d(234, 1)),
            Some(Zone::Buy),
            "buy_top is Buy (closed)"
        );
        assert_eq!(
            present_zone(&bounds, d(23401, 3)),
            Some(Zone::Neutral),
            "just above buy_top is Neutral (open lower bound)"
        );
        assert_eq!(
            present_zone(&bounds, d(342, 1)),
            Some(Zone::Neutral),
            "neutral_top is Neutral"
        );
        assert_eq!(
            present_zone(&bounds, d(34201, 3)),
            Some(Zone::Sell),
            "just above neutral_top is Sell"
        );
        assert_eq!(
            present_zone(&bounds, d(45, 0)),
            Some(Zone::Sell),
            "high edge is Sell"
        );
        assert_eq!(
            present_zone(&bounds, d(125, 1)),
            None,
            "below the range is unknown"
        );
        assert_eq!(
            present_zone(&bounds, d(451, 1)),
            None,
            "above the range is unknown"
        );
    }

    #[test]
    fn degenerate_range_yields_no_zones() {
        assert_eq!(zone_bounds(d(45, 0), d(45, 0)), None, "high == low");
        assert_eq!(
            zone_bounds(d(45, 0), d(12, 0)),
            None,
            "high < low — never inverted bands"
        );
    }
}
