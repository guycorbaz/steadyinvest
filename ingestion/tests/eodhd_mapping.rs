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
//!
//! ssg-1.2.0: `*-JANFY.json` is an NVDA-shaped January fiscal year end (real FY2023–FY2025 net
//! income and diluted share counts, a 10:1 split inside FY2025, invented bar prices) and
//! `*-JUNFY.json` a June one. Their shape follows EODHD's documentation (fundamentals glossary:
//! `netIncomeApplicableToCommonShares`, `commonStockSharesOutstanding`). The real NVDA.US fetch of
//! 2026-09-26 confirmed the share counts are served in today's shares; that the balance sheet's
//! count is the DILUTED WEIGHTED-AVERAGE one rests on EODHD's glossary alone (spec §0, open point).

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
    // ssg-1.2.0: the reported diluted EPS (net income 180 ÷ 100 shares) — present although the
    // adjusted `epsActual` is null: that figure is no longer read at all.
    assert_eq!(y24.eps.as_ref().unwrap().value, dec("1.8"));
    assert_eq!(y24.fiscal_year_end_month, Some(12));
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
/// EPS (ssg-1.2.0: the reported diluted EPS = net income ÷ `commonStockSharesOutstanding`),
/// dividend per share and book value per share are all DERIVED here from totals over the balance
/// sheet's `commonStockSharesOutstanding`, and taken as computed, un-rebased. That is right ONLY
/// IF EODHD restates the balance-sheet share counts of pre-split years into today's shares — the
/// assumption #217 made, CONFIRMED by the real NVDA.US fetch of 2026-09-26 with the owner present
/// (G1 H, G5: the 2017–2020 counts came back ≈ 24–25 bn, today's shares). Should EODHD ever serve
/// pre-split units, these three figures would be overstated by the split ratio for those years.
/// (The adjusted `epsActual` 1.62 of the fixture is never read.)
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
        dec("1.5"),
        "150 ÷ 100 — not ÷2, not the adjusted 1.62"
    );
    // #217, confirmed at G5: the 2023 share count 100 is already in post-split units.
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
    // high is the mapper's already-rebased 12.5 (not 6.25) and the EPS the derived 1.5 (not 0.75).
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
    assert_eq!(y23.eps, Some(dec("1.5")), "no double rebase");
    assert_eq!(y23.book_value_per_share, Some(dec("5")));
}

// ── ssg-1.2.0: fiscal-year prices and reported diluted EPS ─────────────────────────────────────

const FUNDAMENTALS_JAN: &str = include_str!("fixtures/eodhd-fundamentals-JANFY.json");
const EOD_JAN: &str = include_str!("fixtures/eodhd-eod-JANFY.json");
const SPLITS_JAN: &str = include_str!("fixtures/eodhd-splits-JANFY.json");
const FUNDAMENTALS_JUN: &str = include_str!("fixtures/eodhd-fundamentals-JUNFY.json");
const EOD_JUN: &str = include_str!("fixtures/eodhd-eod-JUNFY.json");

fn high_low(raw: &steadyinvest_core::normalize::RawFinancials, y: i32) -> (Decimal, Decimal) {
    let r = year(raw, y);
    (
        r.high_price.as_ref().expect("a high").value,
        r.low_price.as_ref().expect("a low").value,
    )
}

/// A January fiscal year end (NVDA-shaped: the statements are keyed 2023-01-29, 2024-01-28, …),
/// with a 10:1 split on 2024-06-10 INSIDE fiscal 2025. Every expected value is hand-derived from
/// the fixture: the pre-split bars are ÷ 10, then reduced into the FISCAL periods.
#[test]
fn a_january_fiscal_year_pairs_its_eps_with_its_own_fiscal_year_prices() {
    let raw = map_eodhd(
        &json(FUNDAMENTALS_JAN),
        &json(EOD_JAN),
        &json(SPLITS_JAN),
        day(),
        "JANFY.US",
    )
    .expect("maps");
    // FY2023 = (2022-01-29 → 2023-01-29]: 2022-06-15 (19 / 15) and 2023-01-20 (18 / 16).
    assert_eq!(high_low(&raw, 2023), (dec("19"), dec("15")));
    // FY2024 = (2023-01-29 → 2024-01-28]: 2023-03-01 (25 / 23), 2023-12-15 (50 / 48) and the
    // CALENDAR-2024 bar 2024-01-25 (62 / 60). The calendar reduction read 150 / 60 here — the
    // prices of Feb–Dec 2024 beside the EPS of Feb 2023 – Jan 2024.
    assert_eq!(high_low(&raw, 2024), (dec("62"), dec("23")));
    // FY2025 = (2024-01-28 → 2025-01-26], the split inside: 2024-03-01 pre-split (90 / 85),
    // 2024-11-20 (150 / 140) and 2025-01-24 (148 / 135), post-split and untouched.
    assert_eq!(high_low(&raw, 2025), (dec("150"), dec("85")));
    // FY2026 = (2025-01-26 → 2026-01-25].
    assert_eq!(high_low(&raw, 2026), (dec("190"), dec("120")));

    // The reported diluted EPS = net income applicable to common ÷ the served share count,
    // 4 dp — never the adjusted `epsActual` (0.333 / 0.516 / 2.99 / 4.6 in the fixture).
    let eps = |y: i32| year(&raw, y).eps.as_ref().map(|a| a.value);
    assert_eq!(
        eps(2023),
        Some(dec("0.1742")),
        "4 368 000 000 ÷ 25 070 000 000"
    );
    assert_eq!(
        eps(2024),
        Some(dec("1.1933")),
        "29 760 000 000 ÷ 24 940 000 000"
    );
    assert_eq!(
        eps(2025),
        Some(dec("2.9382")),
        "72 880 000 000 ÷ 24 804 000 000"
    );
    assert_eq!(eps(2026), Some(dec("4.4898")));
    for y in 2023..=2026 {
        assert_eq!(year(&raw, y).fiscal_year_end_month, Some(1), "{y}");
    }
    assert!(
        raw.splits.is_empty(),
        "nothing left for normalize to rebase"
    );
}

/// The fiscal year in progress (on the fetch day 2026-09-25: FY2027, 2026-01-26 → 2027-01-25)
/// takes the bars after the last reported end, labelled « 2027 » like the statements would label
/// it; it has prices and no statement — the row the refresh drops as the year in progress (#109).
#[test]
fn the_bars_after_the_last_reported_end_form_the_fiscal_year_in_progress() {
    let raw = map_eodhd(
        &json(FUNDAMENTALS_JAN),
        &json(EOD_JAN),
        &json(SPLITS_JAN),
        day(),
        "JANFY.US",
    )
    .expect("maps");
    assert_eq!(high_low(&raw, 2027), (dec("180"), dec("160")));
    let in_progress = year(&raw, 2027);
    assert!(in_progress.sales.is_none() && in_progress.eps.is_none());
    assert_eq!(in_progress.fiscal_year_end_month, None);
    // No calendar year « 2022 » row either: the 2022-06-15 bar belongs to FY2023.
    assert_eq!(
        raw.years.iter().map(|y| y.year).collect::<Vec<_>>(),
        vec![2023, 2024, 2025, 2026, 2027]
    );
}

/// A June fiscal year end: the bar ON the year end belongs to that year, the next trading day to
/// the next one; the reported EPS replaces the adjusted one (2.80 / 3.30 in the fixture).
#[test]
fn a_june_fiscal_year_reduces_july_to_june() {
    let raw = map_eodhd(
        &json(FUNDAMENTALS_JUN),
        &json(EOD_JUN),
        &serde_json::json!([]),
        day(),
        "JUNFY.US",
    )
    .expect("maps");
    // FY2023 = (2022-06-30 → 2023-06-30]: 2022-08-01 (42 / 38), 2023-06-30 (47 / 44).
    assert_eq!(high_low(&raw, 2023), (dec("47"), dec("38")));
    // FY2024 = (2023-06-30 → 2024-06-30]: 55 / 49, 62 / 58, 59 / 56 — calendar 2023 read 62 / 44.
    assert_eq!(high_low(&raw, 2024), (dec("62"), dec("49")));
    // FY2025, in progress.
    assert_eq!(high_low(&raw, 2025), (dec("70"), dec("63")));
    assert!(year(&raw, 2025).sales.is_none());
    let eps = |y: i32| year(&raw, y).eps.as_ref().map(|a| a.value);
    assert_eq!(eps(2023), Some(dec("2.5")), "400 ÷ 160");
    assert_eq!(eps(2024), Some(dec("3")), "480 ÷ 160");
    assert_eq!(year(&raw, 2024).fiscal_year_end_month, Some(6));
}

/// A December fiscal year end reduces exactly as the calendar years did (the regression anchor):
/// the DEMO fixture's prices are those pinned before ssg-1.2.0, and every year is the calendar
/// year.
#[test]
fn a_december_fiscal_year_keeps_the_calendar_prices() {
    let raw = map_eodhd(
        &json(FUNDAMENTALS),
        &json(EOD),
        &json(SPLITS),
        day(),
        "DEMO",
    )
    .expect("maps");
    assert_eq!(high_low(&raw, 2023), (dec("12.5"), dec("9")));
    assert_eq!(high_low(&raw, 2024), (dec("17"), dec("13")));
    assert_eq!(
        raw.years.iter().map(|y| y.year).collect::<Vec<_>>(),
        vec![2023, 2024]
    );
}
