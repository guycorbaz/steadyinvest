//! The deterministic five-section SSG engine (Story 1.8, FR4): [`compute`] consumes Story
//! 1.7's [`CanonicalFinancials`] plus the user's [`JudgmentInputs`] and the market's
//! [`QuarterlyObservations`], and produces the full [`SsgOutputs`] set — §1 growth,
//! §2 management, §3 P/E history, §4 risk & reward, §5 five-year potential — plus quality
//! flags, calc-time plausibility findings, the low-confidence state and neutral verdict facts.
//!
//! Pure calculation (Cardinal Rule): no I/O / UI / SQL / network, all math in exact
//! [`rust_decimal::Decimal`] (never floats), **no rounding anywhere** (display-only,
//! [`crate::rounding`] — this module calls none of it), and **no panic on any input**: every
//! division is `checked_div`, every exponentiation `checked_powd`, and the spec-§9
//! degenerate-base guard runs BEFORE any `powd` (Spike C: `checked_powd(-2, 0.5)` silently
//! returns a wrong real — the guard, not `checked_*`, is the protection). Outputs are in the
//! caller's native currency (FR5); `core` does no FX.
//!
//! The engine consumes the canonical layer as-is (splits rebased, PTP grossed up,
//! `usable_years` counted) and re-runs none of its checks; the caller resolves
//! `NormalizeError` before computing. Thresholds come exclusively from [`crate::method`] —
//! this story adds no constant, so the method fingerprint is unchanged.

mod growth;
mod management;
mod return_proj;
mod risk_reward;
mod types;
mod valuation;

pub use growth::{EpsSeedBand, endpoints_cagr_pct, least_squares_log_eps_band, project};
pub use types::{
    CalcFinding, CriterionFact, ForecastLowOption, GrowthOutputs, JudgmentInputs,
    ManagementOutputs, QualityFlagKey, QuarterlyObservations, ReturnOutputs, RiskRewardOutputs,
    SsgOutputs, Trend, UpsideDownside, ValuationOutputs, VerdictFacts, YearRatios, YearValuation,
    Zone, ZoneBounds,
};

use crate::method::{
    USABLE_YEARS_FLOOR, high_pe_aggressive, high_pe_implausible, relative_value_ceiling_pct,
    roe_low_pct, ud_extreme, ud_target, verdict_double_appreciation_pct,
};
use crate::normalize::{CanonicalFinancials, CanonicalYear, YearUsability};
use rust_decimal::Decimal;

/// Compute the full SSG output set from canonical financials, judgment inputs and quarterly
/// observations (a distinct third input: market facts, not judgments).
///
/// Deterministic — identical inputs ⇒ identical outputs (FR4/FR29). Every degenerate input
/// resolves to a typed `unknown` per spec §9: never a panic, never a silent 0, never a
/// plausible-but-wrong number.
pub fn compute(
    financials: &CanonicalFinancials,
    judgment: &JudgmentInputs,
    observations: &QuarterlyObservations,
) -> SsgOutputs {
    let mut findings = Vec::new();
    let growth = growth::compute(financials, judgment, observations);
    let management = management::compute(financials, &mut findings);
    let valuation = valuation::compute(financials, judgment, observations, &mut findings);
    let risk_reward = risk_reward::compute(&growth, &valuation, judgment, &mut findings);
    let returns = return_proj::compute(financials, &valuation, &risk_reward, judgment);
    let quality_flags = quality_flags(&growth, &management, &valuation, &risk_reward, judgment);
    let verdict_facts = verdict_facts(&valuation, &risk_reward, &returns);
    SsgOutputs {
        growth,
        management,
        valuation,
        risk_reward,
        returns,
        quality_flags,
        findings,
        low_confidence: financials.usable_years < USABLE_YEARS_FLOOR,
        verdict_facts,
    }
}

/// Arithmetic mean; empty ⇒ `None` (an average of nothing is unknown, never 0).
fn mean(values: &[Decimal]) -> Option<Decimal> {
    if values.is_empty() {
        return None;
    }
    let mut sum = Decimal::ZERO;
    for v in values {
        sum = sum.checked_add(*v)?;
    }
    sum.checked_div(Decimal::from(values.len() as u64))
}

/// The last `USABLE_YEARS_FLOOR` usable years, ascending (fewer when fewer are usable) —
/// the §2/§3 averaging window. Computation still runs on the smaller window (FR8); the
/// `low_confidence` state is carried separately.
fn last_usable_window(financials: &CanonicalFinancials) -> Vec<&CanonicalYear> {
    let usable: Vec<&CanonicalYear> = financials
        .years
        .iter()
        .filter(|y| matches!(y.usability, YearUsability::Usable))
        .collect();
    let start = usable.len().saturating_sub(USABLE_YEARS_FLOOR as usize);
    usable[start..].to_vec()
}

/// Raise the computable quality flags (FR7) with the spec's normative comparators (§2/§7),
/// in pinned-catalog order. **A flag is never raised on an `unknown` metric** — unknown is
/// not a comparison result (spec §9). `high_debt` is not raisable in v1 (no debt input —
/// see [`QualityFlagKey`]).
fn quality_flags(
    growth: &GrowthOutputs,
    management: &ManagementOutputs,
    valuation: &ValuationOutputs,
    risk_reward: &RiskRewardOutputs,
    judgment: &JudgmentInputs,
) -> Vec<QualityFlagKey> {
    let mut flags = Vec::new();
    if management.ptp_trend == Some(Trend::Down) {
        flags.push(QualityFlagKey::PtpTrendDeclining);
    }
    if management.roe_trend == Some(Trend::Down) {
        flags.push(QualityFlagKey::RoeTrendDeclining);
    }
    // Latest ROE < 10 (strict, spec §2) — latest unknown ⇒ not raised.
    if management
        .latest_roe_pct
        .is_some_and(|roe| roe < roe_low_pct())
    {
        flags.push(QualityFlagKey::RoeLow);
    }
    // EPS CAGR < sales CAGR (strict — equal CAGRs do NOT lag); either unknown ⇒ not raised.
    if let (Some(eps), Some(sales)) = (growth.eps_cagr_pct, growth.sales_cagr_pct)
        && eps < sales
    {
        flags.push(QualityFlagKey::EpsLagsSales);
    }
    // Judged future high P/E > 20 / > 25 (strict, spec §2) — independent rows, both can fire.
    if let Some(pe) = judgment.judged_avg_high_pe {
        if pe > high_pe_aggressive() {
            flags.push(QualityFlagKey::ProjectedHighPeAggressive);
        }
        if pe > high_pe_implausible() {
            flags.push(QualityFlagKey::ProjectedHighPeImplausible);
        }
    }
    // U/D flags only on a measured ratio — Undefined/Unknown are not comparison results.
    if let UpsideDownside::Ratio(ud) = risk_reward.upside_downside {
        if ud < ud_target() {
            flags.push(QualityFlagKey::UdBelowTarget);
        }
        if ud > ud_extreme() {
            flags.push(QualityFlagKey::UdExtreme);
        }
    }
    // Relative value ≥ 100 (spec §7: ≥ for the flag, < for the verdict criterion).
    if valuation
        .relative_value_pct
        .is_some_and(|rv| rv >= relative_value_ceiling_pct())
    {
        flags.push(QualityFlagKey::RelativeValueHigh);
    }
    flags
}

/// A tri-valued criterion fact: a measured value compares against its threshold; an unknown
/// input is unmet-by-insufficiency (spec §9) — queryably distinct from a measured miss.
fn criterion(value: Option<Decimal>, met: impl Fn(Decimal) -> bool) -> CriterionFact {
    match value {
        Some(v) if met(v) => CriterionFact::Met,
        Some(_) => CriterionFact::Unmet,
        None => CriterionFact::UnmetByInsufficiency,
    }
}

/// The neutral verdict facts (spec §1 verdict, FR13 posture): the four quality-candidate
/// criteria with their normative comparators, plus the combined candidate fact. No
/// recommendation — `FullVerdict` (integrity-gated) is Story 1.11.
fn verdict_facts(
    valuation: &ValuationOutputs,
    risk_reward: &RiskRewardOutputs,
    returns: &ReturnOutputs,
) -> VerdictFacts {
    let ud_at_or_above_target = match risk_reward.upside_downside {
        UpsideDownside::Ratio(ud) if ud >= ud_target() => CriterionFact::Met,
        UpsideDownside::Ratio(_) => CriterionFact::Unmet,
        // Undefined and Unknown are both insufficiency: §9 withholds the U/D criterion.
        UpsideDownside::Undefined | UpsideDownside::Unknown => CriterionFact::UnmetByInsufficiency,
    };
    let relative_value_below_ceiling = criterion(valuation.relative_value_pct, |rv| {
        rv < relative_value_ceiling_pct()
    });
    let present_price_in_buy_zone = match risk_reward.present_price_zone {
        Some(Zone::Buy) => CriterionFact::Met,
        Some(_) => CriterionFact::Unmet,
        None => CriterionFact::UnmetByInsufficiency,
    };
    // ≥ comparator (recorded interpretation — "roughly doubling" has no §7 table row).
    let appreciation_at_or_above_double = criterion(returns.projected_appreciation_pct, |a| {
        a >= verdict_double_appreciation_pct()
    });
    let all = [
        ud_at_or_above_target,
        relative_value_below_ceiling,
        present_price_in_buy_zone,
        appreciation_at_or_above_double,
    ];
    VerdictFacts {
        present_price_zone: risk_reward.present_price_zone,
        ud_at_or_above_target,
        relative_value_below_ceiling,
        present_price_in_buy_zone,
        appreciation_at_or_above_double,
        quality_value_candidate: all.iter().all(|c| *c == CriterionFact::Met),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_of_nothing_is_unknown_never_zero() {
        assert_eq!(mean(&[]), None);
        assert_eq!(
            mean(&[Decimal::TWO, Decimal::from(4u32)]),
            Some(Decimal::from(3u32))
        );
    }

    /// FR13 posture self-check: no string the engine emits contains a banned imperative verb
    /// (case-insensitive, whole-word), EXCLUDING the zone-label nouns Buy/Neutral/Sell —
    /// spec §6 exempts them (they name the user-defined price bands). The full enforcement
    /// framework is UI-side (Story 2.14); this is the engine's cheap gate.
    #[test]
    fn engine_emitted_strings_contain_no_banned_verbs() {
        use crate::method::{BANNED_VERBS_EN, BANNED_VERBS_FR, contains_word};
        use crate::normalize::PlausibilityKey;

        let zone_labels: Vec<&str> = [Zone::Buy, Zone::Neutral, Zone::Sell]
            .iter()
            .map(|z| z.label())
            .collect();

        let mut emitted: Vec<&str> = Vec::new();
        emitted.extend(QualityFlagKey::ALL.iter().map(|k| k.as_str()));
        emitted.extend(
            [
                PlausibilityKey::OutOfBoundsRatio,
                PlausibilityKey::NegativeOrZeroDenominator,
                PlausibilityKey::LowPriceAboveCurrent,
            ]
            .iter()
            .map(|k| k.as_str()),
        );
        // Finding contexts the engine attaches (see the modules raising them).
        emitted.extend([
            "sales",
            "book_value_per_share",
            "ptp_pct",
            "roe_pct",
            "eps",
            "low_price",
            "ttm_eps",
            "high_pe",
            "low_pe",
            "current_pe",
            "forecast_low",
        ]);

        for s in emitted {
            assert!(
                !zone_labels.contains(&s),
                "zone labels are exempt and must not be in the checked inventory: {s}"
            );
            for banned in BANNED_VERBS_EN.iter().chain(BANNED_VERBS_FR.iter()) {
                assert!(
                    !contains_word(s, banned),
                    "engine-emitted string {s:?} contains banned verb {banned:?} (FR13)"
                );
            }
        }
    }
}
