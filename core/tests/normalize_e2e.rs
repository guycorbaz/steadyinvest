//! Feature-level end-to-end suite for the public `core::normalize` API (Story 1.7 QA pass).
//!
//! The metamorphic/property suite (`normalize_metamorphic.rs`) pins the AC-7 invariances and
//! the submodule unit tests pin each pass in isolation; this suite drives realistic SSG input
//! shapes through the ONE public entry point ([`normalize`]) exactly as Epic-2 manual entry
//! and Epic-3 providers will call it, asserting the COMPLETE canonical output: values,
//! findings in their documented deterministic order (currency pass → fiscal pass →
//! split-break pass), per-year usability and structural errors. No UI or HTTP surface exists
//! in this story (pure `core` library), so browser/API-endpoint tests do not apply.

use rust_decimal::Decimal;
use steadyinvest_core::method::USABLE_YEARS_FLOOR;
use steadyinvest_core::normalize::{
    normalize, Finding, NormalizeError, PlausibilityKey, RawAmount, RawFinancials, RawYear,
    SplitEvent, YearUsability,
};

const NATIVE: &str = "USD";

fn d(mantissa: i64, scale: u32) -> Decimal {
    Decimal::new(mantissa, scale)
}

fn money(currency: &str, mantissa: i64, scale: u32) -> Option<RawAmount> {
    Some(RawAmount {
        value: d(mantissa, scale),
        currency: currency.to_string(),
    })
}

fn usd(mantissa: i64, scale: u32) -> Option<RawAmount> {
    money(NATIVE, mantissa, scale)
}

/// A year with all four load-bearing fields present in the native currency
/// (`sales` in whole units, the per-share fields in cents).
fn usable_year(y: i32, sales: i64, eps_cents: i64, high_cents: i64, low_cents: i64) -> RawYear {
    let mut year = RawYear::empty(y);
    year.sales = usd(sales, 0);
    year.eps = usd(eps_cents, 2);
    year.high_price = usd(high_cents, 2);
    year.low_price = usd(low_cents, 2);
    year
}

fn raw(years: Vec<RawYear>, splits: Vec<SplitEvent>) -> RawFinancials {
    RawFinancials {
        native_currency: NATIVE.to_string(),
        years,
        splits,
    }
}

/// One messy-but-realistic six-year SSG series, given OUT OF ORDER, carrying every
/// input-shape issue at once: a declared 2:1 split (effective 2022), one EUR amount
/// (2020 eps), a 9-month reported period (2021), a fiscal-year-end shift (2023), an
/// undeclared EPS jump (2024) and a missing load-bearing field (2024 low_price).
/// Asserts the complete canonical output, including the documented cross-pass findings
/// order: currency → fiscal (period then shift) → split-break.
#[test]
fn full_messy_series_normalizes_end_to_end_with_deterministic_findings_order() {
    // 2019–2021 are reported in PRE-split shares (×2 of the canonical per-share scale).
    let mut y2019 = usable_year(2019, 1000, 400, 6000, 4000);
    y2019.dividend_per_share = usd(200, 2);
    let mut y2020 = usable_year(2020, 1100, 0, 6600, 4400);
    y2020.eps = money("EUR", 440, 2); // foreign currency — flagged, NEVER converted
    y2020.dividend_per_share = usd(200, 2);
    let mut y2021 = usable_year(2021, 1210, 480, 7200, 4800);
    y2021.period_months = Some(9); // short fiscal period — flagged
    y2021.dividend_per_share = usd(200, 2);
    // 2022 onward is post-split.
    let mut y2022 = usable_year(2022, 1300, 260, 3900, 2600);
    y2022.dividend_per_share = usd(100, 2);
    let mut y2023 = usable_year(2023, 1400, 280, 4200, 2800);
    y2023.dividend_per_share = usd(100, 2);
    let mut y2024 = usable_year(2024, 1450, 560, 4400, 0); // eps ×2 with ~flat sales
    y2024.low_price = None; // missing load-bearing field
    y2024.dividend_per_share = usd(100, 2);
    for (year, fyem) in [
        (&mut y2019, 12),
        (&mut y2020, 12),
        (&mut y2021, 12),
        (&mut y2022, 12),
        (&mut y2023, 6), // fiscal-year-end shift — flagged on 2023
        (&mut y2024, 6),
    ] {
        year.fiscal_year_end_month = Some(fyem);
        if year.period_months.is_none() {
            year.period_months = Some(12);
        }
    }

    let out = normalize(raw(
        vec![y2022, y2019, y2024, y2020, y2023, y2021], // deliberately shuffled
        vec![SplitEvent {
            effective_year: 2022,
            numerator: 2,
            denominator: 1,
        }],
    ))
    .expect("structurally valid input must normalize");

    let order: Vec<i32> = out.years.iter().map(|y| y.year).collect();
    assert_eq!(
        order,
        vec![2019, 2020, 2021, 2022, 2023, 2024],
        "years must come out sorted ascending regardless of input order"
    );

    // Findings in the documented deterministic cross-pass order.
    assert_eq!(
        out.findings,
        vec![
            Finding {
                key: PlausibilityKey::CurrencyMismatch,
                year: 2020,
                context: "eps",
            },
            Finding {
                key: PlausibilityKey::FiscalPeriodMisalignment,
                year: 2021,
                context: "period_months",
            },
            Finding {
                key: PlausibilityKey::FiscalPeriodMisalignment,
                year: 2023,
                context: "fiscal_year_end_month",
            },
            Finding {
                key: PlausibilityKey::SplitSeriesBreak,
                year: 2024,
                context: "eps",
            },
        ],
        "findings must follow the pass order currency → fiscal → split-break: {:?}",
        out.findings
    );

    // Pre-split years are rebased into post-split shares; sales never are.
    let y = &out.years[0]; // 2019
    assert_eq!(
        y.sales,
        Some(d(1000, 0)),
        "sales must NOT be split-adjusted"
    );
    assert_eq!(y.eps, Some(d(200, 2)), "4.00 / 2 = 2.00 exactly");
    assert_eq!(y.high_price, Some(d(3000, 2)));
    assert_eq!(y.low_price, Some(d(2000, 2)));
    assert_eq!(y.dividend_per_share, Some(d(100, 2)));
    assert_eq!(
        out.years[1].eps,
        Some(d(220, 2)),
        "the EUR eps must be rebased (4.40 / 2) but NEVER FX-converted"
    );
    assert_eq!(
        out.years[3].eps,
        Some(d(260, 2)),
        "post-split years pass through untouched"
    );

    // The flagged-but-undeclared jump passes through unchanged (flag only, never corrected),
    // and the year missing low_price names exactly that field.
    let y = &out.years[5]; // 2024
    assert_eq!(y.eps, Some(d(560, 2)), "flagged values are never rewritten");
    assert_eq!(y.low_price, None, "missing stays None — never Some(0)");
    assert_eq!(
        y.usability,
        YearUsability::Insufficient {
            missing: vec!["low_price"],
        }
    );

    assert_eq!(
        out.usable_years, USABLE_YEARS_FLOOR,
        "5 of 6 years are usable — exactly the FR8 floor"
    );
}

#[test]
fn empty_input_yields_empty_canonical_output_without_panic() {
    let out = normalize(raw(vec![], vec![])).expect("an empty study is structurally valid");
    assert!(out.years.is_empty(), "no input years ⇒ no canonical years");
    assert!(
        out.findings.is_empty(),
        "nothing to flag: {:?}",
        out.findings
    );
    assert_eq!(
        out.usable_years, 0,
        "no years ⇒ zero usable years, not a panic"
    );
}

/// A declared 1:3 REVERSE split multiplies pre-split per-share values (×3), is exact for
/// terminating inputs, and must not trip the break detector.
#[test]
fn reverse_split_one_for_three_multiplies_pre_split_per_share_values() {
    let mut pre = usable_year(2020, 1000, 100, 1000, 600);
    pre.dividend_per_share = usd(50, 2);
    let post = usable_year(2021, 1020, 310, 3100, 1850);

    let out = normalize(raw(
        vec![pre, post],
        vec![SplitEvent {
            effective_year: 2021,
            numerator: 1,
            denominator: 3,
        }],
    ))
    .expect("structurally valid input must normalize");

    let y = &out.years[0];
    assert_eq!(y.eps, Some(d(300, 2)), "1.00 × 3 = 3.00 exactly");
    assert_eq!(y.high_price, Some(d(3000, 2)), "10.00 × 3 = 30.00");
    assert_eq!(y.low_price, Some(d(1800, 2)), "6.00 × 3 = 18.00");
    assert_eq!(y.dividend_per_share, Some(d(150, 2)), "0.50 × 3 = 1.50");
    assert_eq!(
        y.sales,
        Some(d(1000, 0)),
        "sales must NOT be split-adjusted"
    );
    assert!(
        out.findings.is_empty(),
        "a declared reverse split must not trip the break detector: {:?}",
        out.findings
    );
    assert_eq!(out.usable_years, 2);
}

/// Two declared splits (2:1 effective 2021, 3:1 effective 2023) compound: years before both
/// are rebased ÷6, years between ÷3, years after pass through — all exact by construction.
#[test]
fn multiple_splits_compound_cumulatively_across_the_series() {
    let years = vec![
        usable_year(2019, 1000, 1800, 27000, 12000), // ÷6 → 3.00 / 45.00 / 20.00
        usable_year(2020, 1010, 1920, 27600, 12600), // ÷6 → 3.20 / 46.00 / 21.00
        usable_year(2021, 1020, 1020, 14100, 6600),  // ÷3 → 3.40 / 47.00 / 22.00
        usable_year(2022, 1030, 1080, 14400, 6900),  // ÷3 → 3.60 / 48.00 / 23.00
        usable_year(2023, 1040, 380, 4900, 2400),    // post both splits — untouched
    ];
    let splits = vec![
        SplitEvent {
            effective_year: 2021,
            numerator: 2,
            denominator: 1,
        },
        SplitEvent {
            effective_year: 2023,
            numerator: 3,
            denominator: 1,
        },
    ];

    let out = normalize(raw(years, splits)).expect("structurally valid input must normalize");

    let eps: Vec<Option<Decimal>> = out.years.iter().map(|y| y.eps).collect();
    assert_eq!(
        eps,
        vec![
            Some(d(300, 2)),
            Some(d(320, 2)),
            Some(d(340, 2)),
            Some(d(360, 2)),
            Some(d(380, 2)),
        ],
        "cumulative ratios must compound exactly (÷6, ÷6, ÷3, ÷3, ÷1)"
    );
    let highs: Vec<Option<Decimal>> = out.years.iter().map(|y| y.high_price).collect();
    assert_eq!(
        highs,
        vec![
            Some(d(4500, 2)),
            Some(d(4600, 2)),
            Some(d(4700, 2)),
            Some(d(4800, 2)),
            Some(d(4900, 2)),
        ]
    );
    let lows: Vec<Option<Decimal>> = out.years.iter().map(|y| y.low_price).collect();
    assert_eq!(
        lows,
        vec![
            Some(d(2000, 2)),
            Some(d(2100, 2)),
            Some(d(2200, 2)),
            Some(d(2300, 2)),
            Some(d(2400, 2)),
        ]
    );
    assert!(
        out.findings.is_empty(),
        "a correctly declared split history leaves a smooth series: {:?}",
        out.findings
    );
    assert_eq!(out.usable_years, 5);
}

/// PTP — directly reported or derived by the §2 gross-up — is an aggregate like `sales`
/// and must NEVER be split-adjusted, even on rebased pre-split years.
#[test]
fn aggregate_ptp_and_gross_up_are_never_split_adjusted() {
    let mut pre = usable_year(2020, 1000, 400, 6000, 4000);
    pre.pre_tax_profit = usd(500, 0); // direct PTP on a pre-split year
    let mut post = usable_year(2021, 1050, 210, 3150, 2100);
    post.net_profit = usd(70, 0); // gross-up inputs on a post-split year
    post.tax_rate = Some(d(30, 2));

    let out = normalize(raw(
        vec![pre, post],
        vec![SplitEvent {
            effective_year: 2021,
            numerator: 2,
            denominator: 1,
        }],
    ))
    .expect("structurally valid input must normalize");

    assert_eq!(
        out.years[0].pre_tax_profit,
        Some(d(500, 0)),
        "direct PTP on a rebased year must stay 500 — never 250 (aggregates are share-count independent)"
    );
    assert_eq!(out.years[0].eps, Some(d(200, 2)), "…while eps IS rebased");
    assert_eq!(
        out.years[1].pre_tax_profit,
        Some(d(100, 0)),
        "gross-up 70 / (1 − 0.30) = 100 exactly"
    );
    assert!(out.findings.is_empty(), "smooth series: {:?}", out.findings);
}

/// A degenerate `tax_rate` (≥ 1) makes the derived PTP unknown (spec §9) — but PTP is not
/// load-bearing, so the year stays usable; nothing panics and nothing becomes 0.
#[test]
fn degenerate_tax_rate_yields_unknown_ptp_but_year_stays_usable() {
    let mut year = usable_year(2024, 1000, 400, 6000, 4000);
    year.net_profit = usd(70, 0);
    year.tax_rate = Some(Decimal::ONE);

    let out = normalize(raw(vec![year], vec![])).expect("structurally valid input");

    assert_eq!(
        out.years[0].pre_tax_profit, None,
        "tax_rate = 1 ⇒ PTP unknown — never a computed value, never 0"
    );
    assert_eq!(
        out.years[0].usability,
        YearUsability::Usable,
        "PTP is not load-bearing — the year remains usable"
    );
    assert_eq!(out.usable_years, 1);
    assert!(
        out.findings.is_empty(),
        "degenerate inputs are typed unknowns, not findings"
    );
}

/// Duplicate years must be rejected even when not adjacent in the INPUT order — the
/// structural check runs on the sorted series.
#[test]
fn non_adjacent_duplicate_years_are_rejected_after_sorting() {
    let years = vec![
        usable_year(2022, 1000, 100, 2000, 1000),
        usable_year(2021, 990, 95, 1900, 950),
        usable_year(2022, 1001, 101, 2001, 1001),
    ];
    assert_eq!(
        normalize(raw(years, vec![])),
        Err(NormalizeError::DuplicateYear { year: 2022 }),
        "duplicates separated in input order must still be a structural error"
    );
}

/// A split effective at (or before) the first reported year rebases nothing: only years
/// strictly BEFORE the effective year are in pre-split shares.
#[test]
fn split_effective_at_first_year_rebases_nothing() {
    let years = vec![
        usable_year(2020, 1000, 200, 3000, 2000),
        usable_year(2021, 1010, 210, 3100, 2100),
    ];
    let out = normalize(raw(
        years,
        vec![SplitEvent {
            effective_year: 2020,
            numerator: 3,
            denominator: 1,
        }],
    ))
    .expect("structurally valid input must normalize");

    assert_eq!(
        out.years[0].eps,
        Some(d(200, 2)),
        "the effective year itself is already post-split — untouched"
    );
    assert_eq!(out.years[1].eps, Some(d(210, 2)));
    assert!(out.findings.is_empty(), "{:?}", out.findings);
}

/// A split effective after the last reported year rebases EVERY year by the same ratio —
/// values change, factors don't, so the break detector stays silent and usability holds.
#[test]
fn split_effective_after_last_year_rebases_every_year() {
    let years = vec![
        usable_year(2023, 1000, 400, 6000, 4400),
        usable_year(2024, 1020, 420, 6300, 4600),
    ];
    let out = normalize(raw(
        years,
        vec![SplitEvent {
            effective_year: 2025,
            numerator: 2,
            denominator: 1,
        }],
    ))
    .expect("structurally valid input must normalize");

    assert_eq!(out.years[0].eps, Some(d(200, 2)), "4.00 / 2 = 2.00");
    assert_eq!(out.years[0].high_price, Some(d(3000, 2)));
    assert_eq!(out.years[0].low_price, Some(d(2200, 2)));
    assert_eq!(out.years[1].eps, Some(d(210, 2)), "4.20 / 2 = 2.10");
    assert_eq!(out.years[0].sales, Some(d(1000, 0)), "sales never rebased");
    assert!(
        out.findings.is_empty(),
        "a uniform rebase preserves every y/y factor: {:?}",
        out.findings
    );
    assert_eq!(out.usable_years, 2, "rebased years keep their usability");
}
