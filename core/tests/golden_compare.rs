//! Behavior tests of the golden self-check entry points (`golden::check` / `check_all`) over
//! a self-contained minimal fixture — moved out of `golden/compare` when it became a directory
//! module (unit tests of the private `compare_numeric` primitive stay in
//! `golden/compare/primitives.rs`). Includes the module's FR13 posture gate over its emitted
//! strings.

use steadyinvest_core::golden::{check, check_all, FixtureSplit, GoldenStudy};
use steadyinvest_core::METHOD_VERSION;

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
    study.input.splits.push(FixtureSplit {
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

/// FR13 posture gate, golden-local (AC 2): no string the compare module emits contains a
/// banned imperative verb. The §6 zone-label exemption extends to the zone-derived field-path
/// nouns `buy_top` / `present_price_in_buy_zone` and to the zone labels themselves —
/// recorded interpretation (Story 1.9 issue). This inventory is hand-maintained: extend
/// it whenever `golden/compare` gains a new emitted string.
#[test]
fn golden_emitted_strings_contain_no_banned_verbs() {
    use steadyinvest_core::method::{contains_word, BANNED_VERBS_EN, BANNED_VERBS_FR};

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
