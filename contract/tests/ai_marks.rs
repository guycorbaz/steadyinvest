//! Story 8.2b T1.4 — the AI marks (`Provenance.ai_origin`, `Judgment.ai_placed`) are additive:
//! legacy JSON reads them empty, a study without marks serializes byte-identically to the form
//! written before 8.2b, and marks round-trip.

use steadyinvest_contract::{
    AiOrigin, AiPlaced, Cell, Coverage, DraftOrigin, ForecastLowOption, Freshness, Judgment, Money,
    Provenance, Review, Source, Study, Timestamp, YearData,
};
use uuid::Uuid;

fn m(s: &str) -> Money {
    Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
}

fn prov(source: Source) -> Provenance {
    Provenance {
        source,
        logical_version: 3,
        timestamp: Timestamp("2026-09-27T10:00:00Z".into()),
        hash_of_dependencies: "h".into(),
        ai_origin: None,
    }
}

fn cell(v: &str) -> Cell {
    Cell {
        value: Some(m(v)),
        source: Source::Manual,
        freshness: Freshness::Current,
        review: Review::None,
        coverage: Coverage::Present,
        provenance: prov(Source::Manual),
        pending: None,
    }
}

fn origin() -> AiOrigin {
    AiOrigin {
        draft_id: Uuid::from_u128(7),
        client: "claude-code".into(),
        model: "claude-opus-5-5".into(),
        validated_at: Timestamp("2026-09-27T11:00:00Z".into()),
    }
}

fn study() -> Study {
    let judgment = Judgment {
        estimated_high_eps: Some(m("5")),
        estimated_low_eps: None,
        projected_sales_growth_pct: None,
        projected_eps_growth_pct: Some(m("12.5")),
        judged_avg_high_pe: None,
        judged_avg_low_pe: None,
        forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
        recent_severe_low: None,
        current_price: Some(m("100")),
        ttm_eps: None,
        present_full_year_dividend: None,
        ai_placed: AiPlaced::default(),
        current_price_origin: None,
    };
    let mut s = Study::new(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        "ACME",
        "USD",
        judgment,
        Timestamp("2026-09-27T09:00:00Z".into()),
    );
    s.years.push(YearData {
        year: 2024,
        sales: cell("1000"),
        eps: cell("2"),
        high_price: cell("50"),
        low_price: cell("30"),
        dividend_per_share: None,
        pre_tax_profit: None,
        book_value_per_share: None,
    });
    s
}

#[test]
fn a_study_without_marks_serializes_without_any_ai_key() {
    let json = serde_json::to_string(&study()).unwrap();
    assert!(!json.contains("ai_origin"), "{json}");
    assert!(!json.contains("ai_placed"), "{json}");
}

#[test]
fn legacy_json_reads_the_marks_empty_and_reserializes_byte_identically() {
    // The form a pre-8.2b build wrote (no `ai_origin`, no `ai_placed`).
    let legacy = serde_json::to_string(&study()).unwrap();
    let back: Study = serde_json::from_str(&legacy).unwrap();
    assert!(back.judgment.ai_placed.is_empty());
    assert!(back.years[0].sales.provenance.ai_origin.is_none());
    assert_eq!(serde_json::to_string(&back).unwrap(), legacy);
}

#[test]
fn marks_round_trip() {
    let mut s = study();
    s.years[0].eps.provenance.ai_origin = Some(origin());
    s.judgment.ai_placed.projected_eps_growth_pct = Some(origin());
    s.judgment.ai_placed.forecast_low_option = Some(origin());
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("\"ai_placed\""), "{json}");
    let back: Study = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
    assert!(!back.judgment.ai_placed.is_empty());
    // Only the set slots are written.
    assert!(!json.contains("\"estimated_low_eps\":{"), "{json}");
}

#[test]
fn draft_origin_is_its_own_type() {
    let d = DraftOrigin {
        client: "claude-code".into(),
        model: "m".into(),
    };
    let json = serde_json::to_string(&d).unwrap();
    assert_eq!(json, r#"{"client":"claude-code","model":"m"}"#);
    assert_eq!(serde_json::from_str::<DraftOrigin>(&json).unwrap(), d);
}
