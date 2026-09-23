//! Story 3.1 — the pure EODHD JSON → `RawFinancials` mapping, tested against recorded fixtures.
//!
//! Anti-circularity: the expected `RawFinancials` is hand-derived from the fixture values, never
//! echoed from the mapper's own output. The live HTTP fidelity (does a real EODHD response match
//! this assumed shape) is the manual GO/NO-GO with a real key — out of CI's reach.

use rust_decimal::Decimal;
use steadyinvest_core::normalize::RawYear;
use steadyinvest_ingestion::adapters::eodhd::map_eodhd;

const FUNDAMENTALS: &str = include_str!("fixtures/eodhd-fundamentals-DEMO.json");
const EOD: &str = include_str!("fixtures/eodhd-eod-DEMO.json");

fn dec(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}

fn year(raw: &steadyinvest_core::normalize::RawFinancials, y: i32) -> &RawYear {
    raw.years
        .iter()
        .find(|r| r.year == y)
        .expect("year present")
}

#[test]
fn maps_eodhd_fundamentals_and_prices_to_raw_financials() {
    let fundamentals: serde_json::Value = serde_json::from_str(FUNDAMENTALS).unwrap();
    let prices: serde_json::Value = serde_json::from_str(EOD).unwrap();

    // Issue #217: the split history comes from the `/splits` endpoint — a 2:1 split dated after
    // every bar rebases ALL the bars (÷2); the per-share fundamentals are taken as served.
    let splits = serde_json::json!([{ "date": "2025-01-01", "split": "2.000000/1.000000" }]);
    let raw = map_eodhd(&fundamentals, &prices, &splits, "DEMO").expect("maps");

    assert_eq!(raw.native_currency, "USD");
    assert_eq!(raw.years.len(), 2, "two reported fiscal years");

    // 2023 — every field present; high/low reduced from the daily bars (max 25, min 18), then ÷2.
    let y23 = year(&raw, 2023);
    assert_eq!(y23.sales.as_ref().unwrap().value, dec("1000"));
    assert_eq!(
        y23.eps.as_ref().unwrap().value,
        dec("1.50"),
        "EPS is served restated — untouched"
    );
    assert_eq!(y23.high_price.as_ref().unwrap().value, dec("12.5"));
    assert_eq!(y23.low_price.as_ref().unwrap().value, dec("9"));
    assert_eq!(y23.pre_tax_profit.as_ref().unwrap().value, dec("200"));
    assert_eq!(y23.net_profit.as_ref().unwrap().value, dec("150"));
    assert_eq!(y23.book_value_per_share.as_ref().unwrap().value, dec("5")); // 500 / 100
    assert_eq!(y23.sales.as_ref().unwrap().currency, "USD");

    // 2024 — epsActual is null → eps stays None (never coerced to 0); high 33, low 21, then ÷2.
    let y24 = year(&raw, 2024);
    assert_eq!(y24.sales.as_ref().unwrap().value, dec("1100"));
    assert!(y24.eps.is_none(), "a null epsActual maps to None, not 0");
    assert_eq!(y24.high_price.as_ref().unwrap().value, dec("16.5"));
    assert_eq!(y24.low_price.as_ref().unwrap().value, dec("10.5"));
    assert_eq!(y24.book_value_per_share.as_ref().unwrap().value, dec("5.6")); // 560 / 100

    // Issue #217: nothing is passed on to `normalize` — the prices are already in today's shares
    // and the per-share figures come restated (a second rebase would double-adjust them).
    assert!(raw.splits.is_empty());
}

#[test]
fn missing_currency_is_a_parse_error_not_a_panic() {
    let fundamentals = serde_json::json!({ "Financials": {} });
    let prices = serde_json::json!([]);
    let err = map_eodhd(&fundamentals, &prices, &serde_json::json!([]), "DEMO").unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("currencycode")
            || matches!(err, steadyinvest_ingestion::ProviderError::Parse { .. })
    );
}

#[test]
fn the_mapped_raw_normalizes_through_core() {
    // The whole point: the mapped raw is accepted by core::normalize (no structural error).
    let fundamentals: serde_json::Value = serde_json::from_str(FUNDAMENTALS).unwrap();
    let prices: serde_json::Value = serde_json::from_str(EOD).unwrap();
    let raw = map_eodhd(&fundamentals, &prices, &serde_json::json!([]), "DEMO").unwrap();
    let canonical = steadyinvest_core::normalize::normalize(raw).expect("normalizes");
    // Split-adjustment rebases pre-split (2023) per-share series by the 2:1 factor; sales is never
    // split-adjusted. Assert the canonical shape exists for both years (exact split math is core's).
    assert_eq!(canonical.years.len(), 2);
    assert_eq!(canonical.years[0].year, 2023);
    assert_eq!(canonical.years[0].sales, Some(dec("1000")));
}
