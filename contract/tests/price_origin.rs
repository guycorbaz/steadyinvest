//! G3 review (2026-10-01): `Judgment.current_price_origin` round-trips, absent stays absent.

use rust_decimal::Decimal;
use steadyinvest_contract::{
    Freshness, Money, PriceOrigin, Source, Study, Timestamp, from_export_json, to_export_json,
};
use uuid::Uuid;

fn study() -> Study {
    let judgment: steadyinvest_contract::Judgment =
        serde_json::from_str(r#"{"forecast_low_option":"avg_low_pe_times_eps"}"#)
            .expect("a minimal judgment");
    Study::new(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        "NESN.SW",
        "CHF",
        judgment,
        Timestamp("2026-10-01T09:00:00Z".to_string()),
    )
}

#[test]
fn a_recorded_origin_round_trips_and_an_absent_one_is_not_written() {
    let mut s = study();
    let bare = serde_json::to_string(&s.judgment).unwrap();
    assert!(
        !bare.contains("current_price_origin"),
        "absent: not serialized"
    );
    s.judgment.current_price = Some(Money::from(Decimal::new(22721, 2)));
    for origin in [
        PriceOrigin {
            source: Source::Provider,
            at: Timestamp("2026-10-01T09:00:00Z".to_string()),
            session_date: Some("2026-09-30".to_string()),
            freshness: Freshness::Stale,
            aged: false,
        },
        PriceOrigin {
            source: Source::Manual,
            at: Timestamp("2026-10-01T10:00:00Z".to_string()),
            session_date: None,
            freshness: Freshness::Current,
            aged: false,
        },
    ] {
        s.judgment.current_price_origin = Some(origin);
        let json = serde_json::to_string(&s).unwrap();
        let back: Study = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        let export = to_export_json(&s);
        assert_eq!(from_export_json(&export).unwrap(), s, "the export keeps it");
    }
}
