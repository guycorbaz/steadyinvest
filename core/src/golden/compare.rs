//! The pure self-check (AC 2): [`check`] replays one [`GoldenStudy`] through the real
//! pipeline `normalize` → `ssg::compute` and compares actual vs expected — exact for
//! everything categorical, within `golden_relative_tolerance()` for derived numerics
//! (relative to the EXPECTED value; `expected == 0` therefore demands exact equality).
//!
//! Deviation wording is neutral (FR13): paths, values and the words "expected"/"actual" —
//! no imperative verbs. The zone-derived field-path nouns (`buy_top`,
//! `present_price_in_buy_zone`) and the zone labels themselves are exempt per spec §6,
//! gated by the module-local posture test below (NOT by the `ssg` inventory, whose
//! `contains_word` matcher treats `_` as a word boundary and would reject `buy_top`).

use super::schema::{
    ExpectedCalcFinding, ExpectedCriterion, ExpectedGrowth, ExpectedManagement,
    ExpectedNormalizeFinding, ExpectedOutputs, ExpectedReturns, ExpectedRiskReward, ExpectedTrend,
    ExpectedUpsideDownside, ExpectedValuation, ExpectedVerdictFacts, ExpectedZone, GoldenStudy,
};
use crate::method::golden_relative_tolerance;
use crate::method_version::METHOD_VERSION;
use crate::normalize::{normalize, Finding, RawFinancials};
use crate::ssg::{
    compute, CalcFinding, CriterionFact, JudgmentInputs, QuarterlyObservations, SsgOutputs, Trend,
    UpsideDownside, Zone,
};
use rust_decimal::Decimal;
use std::fmt;

/// One observed deviation: where, what was expected, what the engine produced, and the
/// relative error when the comparison was numeric.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenDeviation {
    /// Dotted field path into the expected/actual output surface.
    pub path: String,
    pub expected: String,
    pub actual: String,
    /// `|actual − expected| / |expected|` for numeric comparisons with a non-zero expected.
    pub relative_error: Option<Decimal>,
}

impl fmt::Display for GoldenDeviation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: expected {}, actual {}",
            self.path, self.expected, self.actual
        )?;
        if let Some(re) = self.relative_error {
            write!(f, " (relative error {re})")?;
        }
        Ok(())
    }
}

/// The result of replaying one golden study.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenReport {
    /// The golden's `meta.id`.
    pub id: String,
    pub passed: bool,
    pub deviations: Vec<GoldenDeviation>,
}

/// Replay one golden study through `normalize` → `compute` and compare. Pure — no I/O.
pub fn check(study: &GoldenStudy) -> GoldenReport {
    let mut deviations = Vec::new();

    // A stale golden must be re-validated at a method bump, never silently replayed.
    if study.meta.method_version != METHOD_VERSION {
        deviations.push(GoldenDeviation {
            path: "meta.method_version".to_string(),
            expected: study.meta.method_version.clone(),
            actual: METHOD_VERSION.to_string(),
            relative_error: None,
        });
        return GoldenReport {
            id: study.meta.id.clone(),
            passed: false,
            deviations,
        };
    }

    // A golden whose input fails normalization is a failing golden, not a panic.
    let canonical = match normalize(RawFinancials::from(&study.input)) {
        Ok(c) => c,
        Err(e) => {
            deviations.push(GoldenDeviation {
                path: "input".to_string(),
                expected: "structurally valid raw financials".to_string(),
                actual: e.to_string(),
                relative_error: None,
            });
            return GoldenReport {
                id: study.meta.id.clone(),
                passed: false,
                deviations,
            };
        }
    };

    if let Some(expected) = &study.expected.normalize_findings {
        compare_normalize_findings(expected, &canonical.findings, &mut deviations);
    }

    let judgment = JudgmentInputs::from(&study.input.judgment);
    let observations = QuarterlyObservations::from(&study.input.quarterly);
    let actual = compute(&canonical, &judgment, &observations);
    compare_outputs(&study.expected, &actual, &mut deviations);

    GoldenReport {
        id: study.meta.id.clone(),
        passed: deviations.is_empty(),
        deviations,
    }
}

/// [`check`] over a bundle — the exact pair of entry points Story 2.13 consumes.
pub fn check_all(studies: &[GoldenStudy]) -> Vec<GoldenReport> {
    studies.iter().map(check).collect()
}

// ── Rendering (neutral vocabulary — see the posture test) ──

fn opt_decimal(value: Option<Decimal>) -> String {
    match value {
        Some(d) => d.to_string(),
        None => "null".to_string(),
    }
}

fn zone_str(zone: Zone) -> &'static str {
    // Zone labels name the user-defined price bands (spec §6 exemption).
    match zone {
        Zone::Buy => "buy",
        Zone::Neutral => "neutral",
        Zone::Sell => "sell",
    }
}

fn opt_zone(zone: Option<Zone>) -> String {
    match zone {
        Some(z) => zone_str(z).to_string(),
        None => "null".to_string(),
    }
}

fn trend_str(trend: Trend) -> &'static str {
    match trend {
        Trend::Up => "up",
        Trend::Even => "even",
        Trend::Down => "down",
    }
}

fn opt_trend(trend: Option<Trend>) -> String {
    match trend {
        Some(t) => trend_str(t).to_string(),
        None => "null".to_string(),
    }
}

fn criterion_str(fact: CriterionFact) -> &'static str {
    match fact {
        CriterionFact::Met => "met",
        CriterionFact::Unmet => "unmet",
        CriterionFact::UnmetByInsufficiency => "unmet_by_insufficiency",
    }
}

fn ud_str(ud: UpsideDownside) -> String {
    match ud {
        UpsideDownside::Ratio(r) => format!("ratio {r}"),
        UpsideDownside::Undefined => "undefined".to_string(),
        UpsideDownside::Unknown => "unknown".to_string(),
    }
}

fn calc_finding_str(key: &str, year: Option<i32>, context: &str) -> String {
    match year {
        Some(y) => format!("{key} year {y} context {context}"),
        None => format!("{key} study-level context {context}"),
    }
}

// ── Expected → engine-type conversions (compare on engine vocabulary) ──

fn expected_zone(zone: ExpectedZone) -> Zone {
    match zone {
        ExpectedZone::Buy => Zone::Buy,
        ExpectedZone::Neutral => Zone::Neutral,
        ExpectedZone::Sell => Zone::Sell,
    }
}

fn expected_trend(trend: ExpectedTrend) -> Trend {
    match trend {
        ExpectedTrend::Up => Trend::Up,
        ExpectedTrend::Even => Trend::Even,
        ExpectedTrend::Down => Trend::Down,
    }
}

fn expected_criterion(fact: ExpectedCriterion) -> CriterionFact {
    match fact {
        ExpectedCriterion::Met => CriterionFact::Met,
        ExpectedCriterion::Unmet => CriterionFact::Unmet,
        ExpectedCriterion::UnmetByInsufficiency => CriterionFact::UnmetByInsufficiency,
    }
}

// ── Comparison primitives ──

/// Numeric within the method tolerance: `|actual − expected| ≤ tol × |expected|` (spec §7,
/// symmetric, relative to the EXPECTED value — `expected == 0` demands exact equality, no
/// special case needed). Unknown matches only unknown: expected `null` ⇔ actual `None`,
/// both directions.
fn compare_numeric(
    path: &str,
    expected: Option<Decimal>,
    actual: Option<Decimal>,
    deviations: &mut Vec<GoldenDeviation>,
) {
    match (expected, actual) {
        (None, None) => {}
        (Some(e), Some(a)) => {
            let diff = a.checked_sub(e).map(|d| d.abs());
            let tolerance = golden_relative_tolerance().checked_mul(e.abs());
            let within = matches!((diff, tolerance), (Some(d), Some(t)) if d <= t);
            if !within {
                let relative_error = match diff {
                    Some(d) if !e.is_zero() => d.checked_div(e.abs()),
                    _ => None,
                };
                deviations.push(GoldenDeviation {
                    path: path.to_string(),
                    expected: e.to_string(),
                    actual: a.to_string(),
                    relative_error,
                });
            }
        }
        (e, a) => deviations.push(GoldenDeviation {
            path: path.to_string(),
            expected: opt_decimal(e),
            actual: opt_decimal(a),
            relative_error: None,
        }),
    }
}

/// Exact categorical comparison on rendered values.
fn compare_exact(
    path: &str,
    expected: String,
    actual: String,
    deviations: &mut Vec<GoldenDeviation>,
) {
    if expected != actual {
        deviations.push(GoldenDeviation {
            path: path.to_string(),
            expected,
            actual,
            relative_error: None,
        });
    }
}

/// Ordered-list comparison on rendered elements — one deviation carrying both full lists.
fn compare_list(
    path: &str,
    expected: &[String],
    actual: &[String],
    deviations: &mut Vec<GoldenDeviation>,
) {
    if expected != actual {
        deviations.push(GoldenDeviation {
            path: path.to_string(),
            expected: format!("[{}]", expected.join("; ")),
            actual: format!("[{}]", actual.join("; ")),
            relative_error: None,
        });
    }
}

// ── Section comparisons ──

fn compare_outputs(
    expected: &ExpectedOutputs,
    actual: &SsgOutputs,
    deviations: &mut Vec<GoldenDeviation>,
) {
    compare_growth(&expected.growth, actual, deviations);
    compare_management(&expected.management, actual, deviations);
    compare_valuation(&expected.valuation, actual, deviations);
    compare_risk_reward(&expected.risk_reward, actual, deviations);
    compare_returns(&expected.returns, actual, deviations);

    let expected_flags: Vec<String> = expected.quality_flags.clone();
    let actual_flags: Vec<String> = actual
        .quality_flags
        .iter()
        .map(|k| k.as_str().to_string())
        .collect();
    compare_list("quality_flags", &expected_flags, &actual_flags, deviations);

    compare_calc_findings(&expected.findings, &actual.findings, deviations);

    compare_exact(
        "low_confidence",
        expected.low_confidence.to_string(),
        actual.low_confidence.to_string(),
        deviations,
    );

    compare_verdict_facts(&expected.verdict_facts, actual, deviations);
}

fn compare_growth(
    expected: &ExpectedGrowth,
    actual: &SsgOutputs,
    deviations: &mut Vec<GoldenDeviation>,
) {
    let g = &actual.growth;
    compare_numeric(
        "growth.sales_cagr_pct",
        expected.sales_cagr_pct,
        g.sales_cagr_pct,
        deviations,
    );
    compare_numeric(
        "growth.eps_cagr_pct",
        expected.eps_cagr_pct,
        g.eps_cagr_pct,
        deviations,
    );
    compare_numeric(
        "growth.quarterly_sales_change_pct",
        expected.quarterly_sales_change_pct,
        g.quarterly_sales_change_pct,
        deviations,
    );
    compare_numeric(
        "growth.quarterly_eps_change_pct",
        expected.quarterly_eps_change_pct,
        g.quarterly_eps_change_pct,
        deviations,
    );
    compare_numeric(
        "growth.estimated_high_eps",
        expected.estimated_high_eps,
        g.estimated_high_eps,
        deviations,
    );
    compare_numeric(
        "growth.estimated_low_eps",
        expected.estimated_low_eps,
        g.estimated_low_eps,
        deviations,
    );
}

fn compare_management(
    expected: &ExpectedManagement,
    actual: &SsgOutputs,
    deviations: &mut Vec<GoldenDeviation>,
) {
    let m = &actual.management;
    compare_numeric(
        "management.avg_ptp_pct",
        expected.avg_ptp_pct,
        m.avg_ptp_pct,
        deviations,
    );
    compare_numeric(
        "management.avg_roe_pct",
        expected.avg_roe_pct,
        m.avg_roe_pct,
        deviations,
    );
    compare_numeric(
        "management.latest_ptp_pct",
        expected.latest_ptp_pct,
        m.latest_ptp_pct,
        deviations,
    );
    compare_numeric(
        "management.latest_roe_pct",
        expected.latest_roe_pct,
        m.latest_roe_pct,
        deviations,
    );
    compare_exact(
        "management.ptp_trend",
        opt_trend(expected.ptp_trend.map(expected_trend)),
        opt_trend(m.ptp_trend),
        deviations,
    );
    compare_exact(
        "management.roe_trend",
        opt_trend(expected.roe_trend.map(expected_trend)),
        opt_trend(m.roe_trend),
        deviations,
    );
    if let Some(rows) = &expected.per_year {
        if rows.len() != m.per_year.len() {
            deviations.push(GoldenDeviation {
                path: "management.per_year".to_string(),
                expected: format!("{} rows", rows.len()),
                actual: format!("{} rows", m.per_year.len()),
                relative_error: None,
            });
            return;
        }
        for (row, actual_row) in rows.iter().zip(&m.per_year) {
            let prefix = format!("management.per_year[{}]", row.year);
            compare_exact(
                &format!("{prefix}.year"),
                row.year.to_string(),
                actual_row.year.to_string(),
                deviations,
            );
            compare_numeric(
                &format!("{prefix}.ptp_pct"),
                row.ptp_pct,
                actual_row.ptp_pct,
                deviations,
            );
            compare_numeric(
                &format!("{prefix}.roe_pct"),
                row.roe_pct,
                actual_row.roe_pct,
                deviations,
            );
        }
    }
}

fn compare_valuation(
    expected: &ExpectedValuation,
    actual: &SsgOutputs,
    deviations: &mut Vec<GoldenDeviation>,
) {
    let v = &actual.valuation;
    compare_numeric(
        "valuation.avg_high_pe",
        expected.avg_high_pe,
        v.avg_high_pe,
        deviations,
    );
    compare_numeric(
        "valuation.avg_low_pe",
        expected.avg_low_pe,
        v.avg_low_pe,
        deviations,
    );
    compare_numeric("valuation.avg_pe", expected.avg_pe, v.avg_pe, deviations);
    compare_numeric(
        "valuation.avg_payout_pct",
        expected.avg_payout_pct,
        v.avg_payout_pct,
        deviations,
    );
    compare_numeric(
        "valuation.avg_high_yield_pct",
        expected.avg_high_yield_pct,
        v.avg_high_yield_pct,
        deviations,
    );
    compare_numeric(
        "valuation.avg_low_price",
        expected.avg_low_price,
        v.avg_low_price,
        deviations,
    );
    compare_numeric("valuation.ttm_eps", expected.ttm_eps, v.ttm_eps, deviations);
    compare_numeric(
        "valuation.current_pe",
        expected.current_pe,
        v.current_pe,
        deviations,
    );
    compare_numeric(
        "valuation.relative_value_pct",
        expected.relative_value_pct,
        v.relative_value_pct,
        deviations,
    );
    if let Some(rows) = &expected.per_year {
        if rows.len() != v.per_year.len() {
            deviations.push(GoldenDeviation {
                path: "valuation.per_year".to_string(),
                expected: format!("{} rows", rows.len()),
                actual: format!("{} rows", v.per_year.len()),
                relative_error: None,
            });
            return;
        }
        for (row, actual_row) in rows.iter().zip(&v.per_year) {
            let prefix = format!("valuation.per_year[{}]", row.year);
            compare_exact(
                &format!("{prefix}.year"),
                row.year.to_string(),
                actual_row.year.to_string(),
                deviations,
            );
            compare_numeric(
                &format!("{prefix}.high_pe"),
                row.high_pe,
                actual_row.high_pe,
                deviations,
            );
            compare_numeric(
                &format!("{prefix}.low_pe"),
                row.low_pe,
                actual_row.low_pe,
                deviations,
            );
            compare_numeric(
                &format!("{prefix}.payout_pct"),
                row.payout_pct,
                actual_row.payout_pct,
                deviations,
            );
            compare_numeric(
                &format!("{prefix}.high_yield_pct"),
                row.high_yield_pct,
                actual_row.high_yield_pct,
                deviations,
            );
        }
    }
}

fn compare_risk_reward(
    expected: &ExpectedRiskReward,
    actual: &SsgOutputs,
    deviations: &mut Vec<GoldenDeviation>,
) {
    let r = &actual.risk_reward;
    compare_numeric(
        "risk_reward.forecast_high",
        expected.forecast_high,
        r.forecast_high,
        deviations,
    );
    compare_numeric(
        "risk_reward.forecast_low",
        expected.forecast_low,
        r.forecast_low,
        deviations,
    );
    match (&expected.zones, &r.zones) {
        (None, None) => {}
        (Some(e), Some(a)) => {
            compare_numeric(
                "risk_reward.zones.forecast_low",
                Some(e.forecast_low),
                Some(a.forecast_low),
                deviations,
            );
            compare_numeric(
                "risk_reward.zones.buy_top",
                Some(e.buy_top),
                Some(a.buy_top),
                deviations,
            );
            compare_numeric(
                "risk_reward.zones.neutral_top",
                Some(e.neutral_top),
                Some(a.neutral_top),
                deviations,
            );
            compare_numeric(
                "risk_reward.zones.forecast_high",
                Some(e.forecast_high),
                Some(a.forecast_high),
                deviations,
            );
        }
        (e, a) => deviations.push(GoldenDeviation {
            path: "risk_reward.zones".to_string(),
            expected: e.as_ref().map_or("null".to_string(), |z| {
                format!(
                    "[{}, {}, {}, {}]",
                    z.forecast_low, z.buy_top, z.neutral_top, z.forecast_high
                )
            }),
            actual: a.as_ref().map_or("null".to_string(), |z| {
                format!(
                    "[{}, {}, {}, {}]",
                    z.forecast_low, z.buy_top, z.neutral_top, z.forecast_high
                )
            }),
            relative_error: None,
        }),
    }
    compare_exact(
        "risk_reward.present_price_zone",
        opt_zone(expected.present_price_zone.map(expected_zone)),
        opt_zone(r.present_price_zone),
        deviations,
    );
    match (expected.upside_downside, r.upside_downside) {
        (ExpectedUpsideDownside::Ratio(e), UpsideDownside::Ratio(a)) => {
            compare_numeric("risk_reward.upside_downside", Some(e), Some(a), deviations);
        }
        (ExpectedUpsideDownside::Undefined, UpsideDownside::Undefined)
        | (ExpectedUpsideDownside::Unknown, UpsideDownside::Unknown) => {}
        (e, a) => {
            let expected_str = match e {
                ExpectedUpsideDownside::Ratio(v) => format!("ratio {v}"),
                ExpectedUpsideDownside::Undefined => "undefined".to_string(),
                ExpectedUpsideDownside::Unknown => "unknown".to_string(),
            };
            deviations.push(GoldenDeviation {
                path: "risk_reward.upside_downside".to_string(),
                expected: expected_str,
                actual: ud_str(a),
                relative_error: None,
            });
        }
    }
}

fn compare_returns(
    expected: &ExpectedReturns,
    actual: &SsgOutputs,
    deviations: &mut Vec<GoldenDeviation>,
) {
    let r = &actual.returns;
    compare_numeric(
        "returns.present_yield_pct",
        expected.present_yield_pct,
        r.present_yield_pct,
        deviations,
    );
    compare_numeric(
        "returns.avg_annual_eps",
        expected.avg_annual_eps,
        r.avg_annual_eps,
        deviations,
    );
    compare_numeric(
        "returns.avg_annual_dividend",
        expected.avg_annual_dividend,
        r.avg_annual_dividend,
        deviations,
    );
    compare_numeric(
        "returns.avg_yield_pct",
        expected.avg_yield_pct,
        r.avg_yield_pct,
        deviations,
    );
    compare_numeric(
        "returns.projected_appreciation_pct",
        expected.projected_appreciation_pct,
        r.projected_appreciation_pct,
        deviations,
    );
    compare_numeric(
        "returns.projected_total_annualized_return_pct",
        expected.projected_total_annualized_return_pct,
        r.projected_total_annualized_return_pct,
        deviations,
    );
}

fn compare_verdict_facts(
    expected: &ExpectedVerdictFacts,
    actual: &SsgOutputs,
    deviations: &mut Vec<GoldenDeviation>,
) {
    let v = &actual.verdict_facts;
    compare_exact(
        "verdict_facts.present_price_zone",
        opt_zone(expected.present_price_zone.map(expected_zone)),
        opt_zone(v.present_price_zone),
        deviations,
    );
    for (path, e, a) in [
        (
            "verdict_facts.ud_at_or_above_target",
            expected.ud_at_or_above_target,
            v.ud_at_or_above_target,
        ),
        (
            "verdict_facts.relative_value_below_ceiling",
            expected.relative_value_below_ceiling,
            v.relative_value_below_ceiling,
        ),
        (
            "verdict_facts.present_price_in_buy_zone",
            expected.present_price_in_buy_zone,
            v.present_price_in_buy_zone,
        ),
        (
            "verdict_facts.appreciation_at_or_above_double",
            expected.appreciation_at_or_above_double,
            v.appreciation_at_or_above_double,
        ),
    ] {
        compare_exact(
            path,
            criterion_str(expected_criterion(e)).to_string(),
            criterion_str(a).to_string(),
            deviations,
        );
    }
    compare_exact(
        "verdict_facts.quality_value_candidate",
        expected.quality_value_candidate.to_string(),
        v.quality_value_candidate.to_string(),
        deviations,
    );
}

fn compare_calc_findings(
    expected: &[ExpectedCalcFinding],
    actual: &[CalcFinding],
    deviations: &mut Vec<GoldenDeviation>,
) {
    let expected_strs: Vec<String> = expected
        .iter()
        .map(|f| calc_finding_str(&f.key, f.year, &f.context))
        .collect();
    let actual_strs: Vec<String> = actual
        .iter()
        .map(|f| calc_finding_str(f.key.as_str(), f.year, f.context))
        .collect();
    compare_list("findings", &expected_strs, &actual_strs, deviations);
}

fn compare_normalize_findings(
    expected: &[ExpectedNormalizeFinding],
    actual: &[Finding],
    deviations: &mut Vec<GoldenDeviation>,
) {
    let expected_strs: Vec<String> = expected
        .iter()
        .map(|f| calc_finding_str(&f.key, Some(f.year), &f.context))
        .collect();
    let actual_strs: Vec<String> = actual
        .iter()
        .map(|f| calc_finding_str(f.key.as_str(), Some(f.year), f.context))
        .collect();
    compare_list(
        "normalize_findings",
        &expected_strs,
        &actual_strs,
        deviations,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        s.parse()
            .unwrap_or_else(|_| panic!("bad decimal literal {s:?}"))
    }

    fn deviations_of(expected: Option<Decimal>, actual: Option<Decimal>) -> Vec<GoldenDeviation> {
        let mut deviations = Vec::new();
        compare_numeric("probe", expected, actual, &mut deviations);
        deviations
    }

    /// Spec §7 boundary: exactly at ±0.5% of the EXPECTED value passes; just beyond fails.
    #[test]
    fn tolerance_boundary_is_inclusive_at_half_percent() {
        // expected 100 ⇒ tolerance 0.5 — actual 100.5 / 99.5 pass, 100.51 / 99.49 fail.
        assert!(deviations_of(Some(d("100")), Some(d("100.5"))).is_empty());
        assert!(deviations_of(Some(d("100")), Some(d("99.5"))).is_empty());
        let over = deviations_of(Some(d("100")), Some(d("100.51")));
        assert_eq!(over.len(), 1, "just beyond the tolerance must deviate");
        assert_eq!(over[0].relative_error, Some(d("0.0051")));
        assert_eq!(deviations_of(Some(d("100")), Some(d("99.49"))).len(), 1);
        // Tolerance is relative to EXPECTED: expected 10.05 vs actual 10 passes
        // (0.05 ≤ 0.005 × 10.05 = 0.05025); expected 10.06 vs actual 10 fails.
        assert!(deviations_of(Some(d("10.05")), Some(d("10"))).is_empty());
        assert_eq!(deviations_of(Some(d("10.06")), Some(d("10"))).len(), 1);
    }

    /// The §7 formula itself makes `expected == 0` demand exact equality (0.005 × 0 = 0).
    #[test]
    fn expected_zero_demands_exact_equality() {
        assert!(deviations_of(Some(d("0")), Some(d("0"))).is_empty());
        assert!(
            deviations_of(Some(d("0")), Some(d("0.00"))).is_empty(),
            "value equality"
        );
        let off = deviations_of(Some(d("0")), Some(d("0.0001")));
        assert_eq!(off.len(), 1);
        assert_eq!(
            off[0].relative_error, None,
            "no relative error against zero"
        );
    }

    /// Unknown matches only unknown — both directions deviate.
    #[test]
    fn null_and_none_must_agree_in_both_directions() {
        assert!(deviations_of(None, None).is_empty());
        let expected_null = deviations_of(None, Some(d("1")));
        assert_eq!(expected_null.len(), 1);
        assert_eq!(expected_null[0].expected, "null");
        assert_eq!(expected_null[0].actual, "1");
        let actual_none = deviations_of(Some(d("1")), None);
        assert_eq!(actual_none.len(), 1);
        assert_eq!(actual_none[0].expected, "1");
        assert_eq!(actual_none[0].actual, "null");
    }

    /// A no-years minimal study: parses, runs the pipeline, and every unknown matches null.
    const MINIMAL_STUDY: &str = r#"{
        "meta": {
            "id": "minimal",
            "title": "minimal",
            "description": "no years - every output unknown",
            "provenance": "trivial by construction: no usable data exists",
            "method_version": "ssg-1.0.0",
            "fixture_format_version": 1
        },
        "input": {
            "native_currency": "USD",
            "years": [],
            "splits": [],
            "judgment": {
                "estimated_high_eps": null,
                "estimated_low_eps": null,
                "projected_sales_growth_pct": null,
                "projected_eps_growth_pct": null,
                "judged_avg_high_pe": null,
                "judged_avg_low_pe": null,
                "forecast_low_option": "avg_low_pe_times_eps",
                "recent_severe_low": null,
                "current_price": null,
                "present_full_year_dividend": null
            },
            "quarterly": {
                "ttm_quarterly_eps": null,
                "latest_quarter_sales": null,
                "latest_quarter_eps": null,
                "year_ago_quarter_sales": null,
                "year_ago_quarter_eps": null
            }
        },
        "expected": {
            "growth": {
                "sales_cagr_pct": null,
                "eps_cagr_pct": null,
                "quarterly_sales_change_pct": null,
                "quarterly_eps_change_pct": null,
                "estimated_high_eps": null,
                "estimated_low_eps": null
            },
            "management": {
                "avg_ptp_pct": null,
                "avg_roe_pct": null,
                "latest_ptp_pct": null,
                "latest_roe_pct": null,
                "ptp_trend": null,
                "roe_trend": null,
                "per_year": []
            },
            "valuation": {
                "avg_high_pe": null,
                "avg_low_pe": null,
                "avg_pe": null,
                "avg_payout_pct": null,
                "avg_high_yield_pct": null,
                "avg_low_price": null,
                "ttm_eps": null,
                "current_pe": null,
                "relative_value_pct": null,
                "per_year": []
            },
            "risk_reward": {
                "forecast_high": null,
                "forecast_low": null,
                "zones": null,
                "present_price_zone": null,
                "upside_downside": "unknown"
            },
            "returns": {
                "present_yield_pct": null,
                "avg_annual_eps": null,
                "avg_annual_dividend": null,
                "avg_yield_pct": null,
                "projected_appreciation_pct": null,
                "projected_total_annualized_return_pct": null
            },
            "quality_flags": [],
            "findings": [],
            "low_confidence": true,
            "verdict_facts": {
                "present_price_zone": null,
                "ud_at_or_above_target": "unmet_by_insufficiency",
                "relative_value_below_ceiling": "unmet_by_insufficiency",
                "present_price_in_buy_zone": "unmet_by_insufficiency",
                "appreciation_at_or_above_double": "unmet_by_insufficiency",
                "quality_value_candidate": false
            },
            "normalize_findings": []
        }
    }"#;

    #[test]
    fn minimal_all_unknown_study_passes() {
        let study: GoldenStudy = serde_json::from_str(MINIMAL_STUDY).expect("valid fixture");
        let report = check(&study);
        assert!(
            report.passed,
            "deviations: {:?}",
            report
                .deviations
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(report.id, "minimal");
        // check_all is the same comparison over a bundle.
        let reports = check_all(std::slice::from_ref(&study));
        assert_eq!(reports.len(), 1);
        assert!(reports[0].passed);
    }

    /// A fixture pinned to another method version fails its check (never silently replayed).
    #[test]
    fn stale_method_version_fails_the_check() {
        let stale = MINIMAL_STUDY.replace("\"ssg-1.0.0\"", "\"ssg-0.9.0\"");
        assert_ne!(stale, MINIMAL_STUDY, "the replacement must have applied");
        let study: GoldenStudy = serde_json::from_str(&stale).expect("still parses");
        let report = check(&study);
        assert!(!report.passed);
        assert_eq!(report.deviations.len(), 1);
        assert_eq!(report.deviations[0].path, "meta.method_version");
        assert_eq!(report.deviations[0].expected, "ssg-0.9.0");
        assert_eq!(report.deviations[0].actual, METHOD_VERSION);
    }

    /// A golden whose input fails normalization is a failing golden, not a panic.
    #[test]
    fn structurally_invalid_input_is_a_failing_report() {
        let mut study: GoldenStudy = serde_json::from_str(MINIMAL_STUDY).expect("valid fixture");
        // Inject a zero split ratio (structural NormalizeError) without touching the JSON.
        study.input.splits.push(crate::golden::FixtureSplit {
            effective_year: 2024,
            numerator: 0,
            denominator: 1,
        });
        let report = check(&study);
        assert!(!report.passed);
        assert_eq!(report.deviations.len(), 1);
        assert_eq!(report.deviations[0].path, "input");
        assert!(report.deviations[0].actual.contains("2024"));
    }

    /// FR13 posture gate, golden-local (AC 2): no string this module emits contains a banned
    /// imperative verb. The §6 zone-label exemption extends to the zone-derived field-path
    /// nouns `buy_top` / `present_price_in_buy_zone` and to the zone labels themselves —
    /// recorded interpretation (Story 1.9 issue). This inventory is hand-maintained: extend
    /// it whenever compare.rs gains a new emitted string.
    #[test]
    fn golden_emitted_strings_contain_no_banned_verbs() {
        use crate::method::{BANNED_VERBS_EN, BANNED_VERBS_FR};

        // Spec §6 exemptions: zone labels + zone-derived field-path nouns.
        let exempt = [
            "buy",
            "neutral",
            "sell",
            "risk_reward.zones.buy_top",
            "verdict_facts.present_price_in_buy_zone",
        ];

        let emitted = [
            // Report wording.
            "expected",
            "actual",
            "relative error",
            "rows",
            "study-level",
            "year",
            "context",
            "null",
            "true",
            "false",
            "structurally valid raw financials",
            // States.
            "ratio",
            "undefined",
            "unknown",
            "met",
            "unmet",
            "unmet_by_insufficiency",
            "up",
            "even",
            "down",
            // Field paths.
            "meta.method_version",
            "input",
            "growth.sales_cagr_pct",
            "growth.eps_cagr_pct",
            "growth.quarterly_sales_change_pct",
            "growth.quarterly_eps_change_pct",
            "growth.estimated_high_eps",
            "growth.estimated_low_eps",
            "management.avg_ptp_pct",
            "management.avg_roe_pct",
            "management.latest_ptp_pct",
            "management.latest_roe_pct",
            "management.ptp_trend",
            "management.roe_trend",
            "management.per_year",
            // Per-year row paths (representative index — the bracket index is numeric).
            "management.per_year[2021].year",
            "management.per_year[2021].ptp_pct",
            "management.per_year[2021].roe_pct",
            "valuation.per_year[2021].year",
            "valuation.per_year[2021].high_pe",
            "valuation.per_year[2021].low_pe",
            "valuation.per_year[2021].payout_pct",
            "valuation.per_year[2021].high_yield_pct",
            "valuation.avg_high_pe",
            "valuation.avg_low_pe",
            "valuation.avg_pe",
            "valuation.avg_payout_pct",
            "valuation.avg_high_yield_pct",
            "valuation.avg_low_price",
            "valuation.ttm_eps",
            "valuation.current_pe",
            "valuation.relative_value_pct",
            "valuation.per_year",
            "risk_reward.forecast_high",
            "risk_reward.forecast_low",
            "risk_reward.zones",
            "risk_reward.zones.forecast_low",
            "risk_reward.zones.neutral_top",
            "risk_reward.zones.forecast_high",
            "risk_reward.present_price_zone",
            "risk_reward.upside_downside",
            "returns.present_yield_pct",
            "returns.avg_annual_eps",
            "returns.avg_annual_dividend",
            "returns.avg_yield_pct",
            "returns.projected_appreciation_pct",
            "returns.projected_total_annualized_return_pct",
            "quality_flags",
            "findings",
            "normalize_findings",
            "low_confidence",
            "verdict_facts.present_price_zone",
            "verdict_facts.ud_at_or_above_target",
            "verdict_facts.relative_value_below_ceiling",
            "verdict_facts.appreciation_at_or_above_double",
            "verdict_facts.quality_value_candidate",
        ];

        // Same whole-word matcher as the ssg gate — which is exactly why the exempt paths
        // cannot live in that inventory: `_` is a word boundary, so "buy_top" contains "buy".
        let contains_word = |haystack: &str, needle: &str| {
            let h = haystack.to_lowercase();
            let n = needle.to_lowercase();
            h.match_indices(&n).any(|(i, _)| {
                let before_ok = i == 0
                    || !h[..i]
                        .chars()
                        .next_back()
                        .is_some_and(|c| c.is_alphanumeric());
                let after = i + n.len();
                let after_ok = after == h.len()
                    || !h[after..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_alphanumeric());
                before_ok && after_ok
            })
        };

        for s in emitted {
            assert!(
                !exempt.contains(&s),
                "exempt strings must not be in the checked inventory: {s}"
            );
            for banned in BANNED_VERBS_EN.iter().chain(BANNED_VERBS_FR.iter()) {
                assert!(
                    !contains_word(s, banned),
                    "golden-emitted string {s:?} contains banned verb {banned:?} (FR13)"
                );
            }
        }
    }
}
