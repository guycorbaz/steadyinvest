//! Story 8.8 T1 — the frozen verdict is additive: a study never validated serializes exactly as
//! before (no key), a legacy blob reads `None` and re-serializes byte-identically, a frozen verdict
//! round-trips, and its zone is a neutral code (no buy / sell word in the blob MCP returns).

use std::collections::BTreeMap;

use steadyinvest_contract::{
    AiPlaced, ForecastLowOption, FrozenCriterion, FrozenUpsideDownside, FrozenVerdict, FrozenZone,
    FrozenZoneBounds, Judgment, Money, Study, Timestamp,
};
use uuid::Uuid;

fn m(s: &str) -> Money {
    Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
}

fn study() -> Study {
    let judgment = Judgment {
        estimated_high_eps: Some(m("5")),
        estimated_low_eps: None,
        projected_sales_growth_pct: None,
        projected_eps_growth_pct: None,
        judged_avg_high_pe: None,
        judged_avg_low_pe: None,
        forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
        recent_severe_low: None,
        current_price: Some(m("100")),
        ttm_eps: None,
        present_full_year_dividend: None,
        ai_placed: AiPlaced::default(),
    };
    Study::new(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        "ACME",
        "USD",
        judgment,
        Timestamp("2026-09-30T09:00:00Z".into()),
    )
}

fn frozen() -> FrozenVerdict {
    let mut inputs = BTreeMap::new();
    inputs.insert("y2024.eps".to_string(), "2.00".to_string());
    inputs.insert("j.current_price".to_string(), "100".to_string());
    inputs.insert("q.ttm_eps".to_string(), "absent".to_string());
    FrozenVerdict {
        frozen_at: Timestamp("2026-09-30T10:00:00Z".into()),
        method_version: "ssg-1.2.0".into(),
        inputs_hash: "ab".repeat(32),
        quality_value_candidate: true,
        present_zone: Some(FrozenZone::Low),
        ud_at_or_above_target: FrozenCriterion::Met,
        relative_value_below_ceiling: FrozenCriterion::Met,
        present_price_in_low_zone: FrozenCriterion::Met,
        appreciation_at_or_above_double: FrozenCriterion::UnmetByInsufficiency,
        upside_downside: FrozenUpsideDownside::Ratio(m("3.2")),
        relative_value_pct: Some(m("85")),
        projected_appreciation_pct: Some(m("120")),
        five_year_potential_pct: Some(m("17.1")),
        zones: Some(FrozenZoneBounds {
            forecast_low: m("40"),
            low_zone_top: m("60"),
            middle_zone_top: m("80"),
            forecast_high: m("100"),
        }),
        inputs,
    }
}

#[test]
fn a_study_never_validated_has_no_frozen_key_and_legacy_blobs_are_byte_identical() {
    let json = serde_json::to_string(&study()).unwrap();
    assert!(!json.contains("frozen_verdict"), "absent, not null");
    let legacy: Study = serde_json::from_str(&json).unwrap();
    assert_eq!(legacy.frozen_verdict, None);
    assert_eq!(serde_json::to_string(&legacy).unwrap(), json);
}

#[test]
fn a_frozen_verdict_round_trips_and_its_zone_is_neutral() {
    let mut s = study();
    s.frozen_verdict = Some(frozen());
    let json = serde_json::to_string(&s).unwrap();
    let back: Study = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
    assert_eq!(
        serde_json::to_string(&back).unwrap(),
        json,
        "byte-identical"
    );
    assert!(json.contains("\"present_zone\":\"low\""));
    for word in ["buy", "sell", "hold", "achat", "vente"] {
        assert!(!json.to_lowercase().contains(word), "{word} in the blob");
    }
    assert!(json.contains("\"upside_downside\":{\"state\":\"ratio\",\"value\":\"3.2\"}"));
}
