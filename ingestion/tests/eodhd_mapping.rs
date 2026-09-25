//! Story 3.1 — the pure EODHD JSON → `RawFinancials` mapping, tested against recorded fixtures.
//!
//! Anti-circularity: the expected `RawFinancials` is hand-derived from the fixture values, never
//! echoed from the mapper's own output. The live HTTP fidelity (does a real EODHD response match
//! this assumed shape) is the manual GO/NO-GO with a real key — out of CI's reach.
//!
//! Issue #217 / G1 H (#237): the fixtures carry a REAL split — a 2:1 on 2024-06-01, served by the
//! `/splits` endpoint (`eodhd-splits-DEMO.json`), in the middle of fiscal 2024. The fundamentals'
//! own `SplitsDividends` block carries only `LastSplitFactor` / `LastSplitDate` (the live shape),
//! which the mapper does not read. The `/eod` fixture is RAW like the live series: the bars before
//! the split trade around 20–30, the post-split November bar around 14–17 (half the scale).

use chrono::NaiveDate;
use rust_decimal::Decimal;
use steadyinvest_core::normalize::RawYear;
use steadyinvest_ingestion::ProviderError;
use steadyinvest_ingestion::adapters::eodhd::map_eodhd;

const FUNDAMENTALS: &str = include_str!("fixtures/eodhd-fundamentals-DEMO.json");
const EOD: &str = include_str!("fixtures/eodhd-eod-DEMO.json");
const SPLITS: &str = include_str!("fixtures/eodhd-splits-DEMO.json");

fn dec(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}

/// A fixed fetch day, after every fixture date — never the wall clock in a test.
fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 25).unwrap()
}

fn json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap()
}

fn year(raw: &steadyinvest_core::normalize::RawFinancials, y: i32) -> &RawYear {
    raw.years
        .iter()
        .find(|r| r.year == y)
        .expect("year present")
}

#[test]
fn maps_eodhd_fundamentals_and_prices_to_raw_financials() {
    let raw = map_eodhd(
        &json(FUNDAMENTALS),
        &json(EOD),
        &json(SPLITS),
        day(),
        "DEMO",
    )
    .expect("maps");

    assert_eq!(raw.native_currency, "USD");
    assert_eq!(raw.years.len(), 2, "two reported fiscal years");

    // 2023 — every field present. Both bars predate the 2024-06-01 2:1 split: the raw high 25 and
    // low 18 are rebased into today's shares (÷2).
    let y23 = year(&raw, 2023);
    assert_eq!(y23.sales.as_ref().unwrap().value, dec("1000"));
    assert_eq!(
        y23.high_price.as_ref().unwrap().value,
        dec("12.5"),
        "25 ÷ 2"
    );
    assert_eq!(y23.low_price.as_ref().unwrap().value, dec("9"), "18 ÷ 2");
    assert_eq!(y23.pre_tax_profit.as_ref().unwrap().value, dec("200"));
    assert_eq!(y23.net_profit.as_ref().unwrap().value, dec("150"));
    assert_eq!(y23.sales.as_ref().unwrap().currency, "USD");

    // 2024 — the split falls INSIDE the year: the February bar (30 / 26) is pre-split → 15 / 13,
    // the November bar (17 / 14) is post-split and untouched. The yearly high is November's 17
    // (a raw, mixed-base max would read 30), the low the rebased February 13 (below November's 14).
    let y24 = year(&raw, 2024);
    assert_eq!(y24.sales.as_ref().unwrap().value, dec("1100"));
    assert!(y24.eps.is_none(), "a null epsActual maps to None, not 0");
    assert_eq!(
        y24.high_price.as_ref().unwrap().value,
        dec("17"),
        "not the raw 30"
    );
    assert_eq!(y24.low_price.as_ref().unwrap().value, dec("13"), "26 ÷ 2");

    // Issue #217: nothing is passed on to `normalize` — the prices are already in today's shares
    // and the per-share figures come restated (a second rebase would double-adjust them).
    assert!(raw.splits.is_empty());
}

/// The per-share FUNDAMENTALS across the same 2:1 split — what the mapper does today, pinned.
///
/// - EPS: `Earnings.Annual.epsActual` is served already restated into today's shares (verified on
///   NVDA in #217: EPS 2017 = 0.064 after the 4:1 and 10:1 splits) — taken as served, never
///   divided again.
/// - Dividend per share and book value per share are DERIVED here from totals over the balance
///   sheet's `commonStockSharesOutstanding`, and are also taken as computed, un-rebased. That is
///   right ONLY IF EODHD restates the balance-sheet share counts of pre-split years into today's
///   shares — an ASSUMPTION #217 made, not yet verified. It is to be confirmed by ONE real
///   NVDA.US fetch with the owner present (G1 review, G5): if the pre-2021 share counts come back
///   in pre-split units, these two figures are overstated ×40 for those years and this test must
///   change with the fix.
#[test]
fn per_share_fundamentals_across_a_split_are_taken_as_served_pinned_on_the_share_count_assumption()
{
    let raw = map_eodhd(
        &json(FUNDAMENTALS),
        &json(EOD),
        &json(SPLITS),
        day(),
        "DEMO",
    )
    .expect("maps");
    let y23 = year(&raw, 2023);
    assert_eq!(
        y23.eps.as_ref().unwrap().value,
        dec("1.50"),
        "EPS as served (restated) — not ÷2"
    );
    // ASSUMPTION (G5): the 2023 share count 100 is already in post-split units.
    assert_eq!(
        y23.dividend_per_share.as_ref().unwrap().value,
        dec("1"),
        "|-100| / 100 — not ÷2"
    );
    assert_eq!(
        y23.book_value_per_share.as_ref().unwrap().value,
        dec("5"),
        "500 / 100 — not ÷2"
    );
    let y24 = year(&raw, 2024);
    assert_eq!(y24.dividend_per_share.as_ref().unwrap().value, dec("1.1"));
    assert_eq!(y24.book_value_per_share.as_ref().unwrap().value, dec("5.6"));
}

/// G1 H (#237): no split in the history (a company that never split) — the bars pass raw.
#[test]
fn an_empty_split_history_leaves_the_bars_raw() {
    let raw = map_eodhd(
        &json(FUNDAMENTALS),
        &json(EOD),
        &serde_json::json!([]),
        day(),
        "DEMO",
    )
    .expect("maps");
    assert_eq!(
        year(&raw, 2023).high_price.as_ref().unwrap().value,
        dec("25")
    );
    assert_eq!(
        year(&raw, 2024).high_price.as_ref().unwrap().value,
        dec("30")
    );
    assert_eq!(
        year(&raw, 2024).low_price.as_ref().unwrap().value,
        dec("14")
    );
}

/// G1 H (#237, owner decision 10): a 200 whose body is an error object instead of the split array
/// fails the mapping under the split history's name — never « no splits » (the bars would then
/// be served raw, at the pre-split scale, beside restated EPS: the #217 defect).
#[test]
fn a_split_body_that_is_not_the_array_fails_named_never_no_splits() {
    let body = serde_json::json!({ "code": 403, "message": "plan does not include splits" });
    let err = map_eodhd(&json(FUNDAMENTALS), &json(EOD), &body, day(), "DEMO").unwrap_err();
    match err {
        ProviderError::SplitHistory { cause } => {
            assert!(matches!(*cause, ProviderError::Parse { .. }), "{cause:?}")
        }
        other => panic!("expected the split history's named failure, got {other:?}"),
    }
}

/// G1 H (#237): a fractional ratio (3:2 served "1.500000/1.000000") is applied, 4 dp.
#[test]
fn a_fractional_split_ratio_is_applied_exactly() {
    let splits = serde_json::json!([{ "date": "2025-01-01", "split": "1.500000/1.000000" }]);
    let raw = map_eodhd(&json(FUNDAMENTALS), &json(EOD), &splits, day(), "DEMO").expect("maps");
    // Every bar predates the split: 2023 high 25 ÷ 1.5 = 16.6667 (4 dp), low 18 ÷ 1.5 = 12.
    assert_eq!(
        year(&raw, 2023).high_price.as_ref().unwrap().value,
        dec("16.6667")
    );
    assert_eq!(
        year(&raw, 2023).low_price.as_ref().unwrap().value,
        dec("12")
    );
    // 2024 high: the raw 30 ÷ 1.5 = 20 (this hypothetical split postdates every bar).
    assert_eq!(
        year(&raw, 2024).high_price.as_ref().unwrap().value,
        dec("20")
    );
}

#[test]
fn missing_currency_is_a_parse_error_not_a_panic() {
    let fundamentals = serde_json::json!({ "Financials": {} });
    let prices = serde_json::json!([]);
    let err = map_eodhd(
        &fundamentals,
        &prices,
        &serde_json::json!([]),
        day(),
        "DEMO",
    )
    .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("currencycode")
            || matches!(err, ProviderError::Parse { .. })
    );
}

#[test]
fn the_mapped_raw_normalizes_through_core() {
    // The mapped raw is accepted by core::normalize (no structural error), and — because the
    // mapper hands on NO splits — normalize rebases nothing a second time: the canonical 2023
    // high is the mapper's already-rebased 12.5 (not 6.25) and the EPS the served 1.50 (not 0.75).
    let raw = map_eodhd(
        &json(FUNDAMENTALS),
        &json(EOD),
        &json(SPLITS),
        day(),
        "DEMO",
    )
    .unwrap();
    let canonical = steadyinvest_core::normalize::normalize(raw).expect("normalizes");
    assert_eq!(canonical.years.len(), 2);
    let y23 = &canonical.years[0];
    assert_eq!(y23.year, 2023);
    assert_eq!(
        y23.sales,
        Some(dec("1000")),
        "sales is never split-adjusted"
    );
    assert_eq!(y23.high_price, Some(dec("12.5")), "no double rebase");
    assert_eq!(y23.eps, Some(dec("1.50")), "no double rebase");
    assert_eq!(y23.book_value_per_share, Some(dec("5")));
}
