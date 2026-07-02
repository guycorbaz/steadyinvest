//! The **section walkers** of the golden self-check: one `compare_*` function per SSG output
//! section (§1 growth, §2 management, §3 valuation, §4 risk & reward, §5 returns, plus
//! verdict facts, calc findings and normalize findings), each pushing its deviations through
//! the primitives in `primitives`. [`compare_outputs`] is the section dispatcher `check`
//! drives; field paths are dotted into the expected/actual output surface.

use super::primitives::{
    calc_finding_str, compare_exact, compare_list, compare_numeric, criterion_str,
    expected_criterion, expected_trend, expected_zone, opt_trend, opt_zone, ud_str,
};
use super::GoldenDeviation;
use crate::golden::schema::{
    ExpectedCalcFinding, ExpectedGrowth, ExpectedManagement, ExpectedNormalizeFinding,
    ExpectedOutputs, ExpectedReturns, ExpectedRiskReward, ExpectedUpsideDownside,
    ExpectedValuation, ExpectedVerdictFacts,
};
use crate::normalize::Finding;
use crate::ssg::{CalcFinding, SsgOutputs, UpsideDownside};

pub(super) fn compare_outputs(
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

pub(super) fn compare_normalize_findings(
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
