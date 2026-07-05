//! Explicit-case suite for `core::ssg` (Story 1.8, AC 2–8): a hand-computed Tutorial-style
//! worked example covering all five sections, the binding spec-§9 degenerate-input table
//! (each engine row), and the normative boundary comparators (spec §7).
//!
//! Every expected value is hand-computed once from the spec formulas and cross-checked
//! against the other sections (1.7 lesson: internally-consistent fixtures). Exact arithmetic
//! is asserted with `assert_eq!`; values that pass through a fractional `powd` or a
//! non-terminating quotient are asserted within an explicit tiny epsilon (Spike C: the
//! series error is ≤ ~1e-27 relative — far inside 1e-18).

use rust_decimal::Decimal;
use steadyinvest_core::normalize::{
    CanonicalFinancials, PlausibilityKey, RawAmount, RawYear, normalize,
};
use steadyinvest_core::ssg::{
    CalcFinding, CriterionFact, ForecastLowOption, JudgmentInputs, QualityFlagKey,
    QuarterlyObservations, UpsideDownside, Zone, compute,
};

fn d(s: &str) -> Decimal {
    s.parse()
        .unwrap_or_else(|_| panic!("bad decimal literal {s:?}"))
}

fn amt(s: &str) -> Option<RawAmount> {
    Some(RawAmount {
        value: d(s),
        currency: "USD".to_string(),
    })
}

/// One fully-populated raw year (all 7 value fields).
#[allow(clippy::too_many_arguments)]
fn full_year(
    year: i32,
    sales: &str,
    eps: &str,
    high: &str,
    low: &str,
    dividend: &str,
    ptp: &str,
    book_value: &str,
) -> RawYear {
    let mut y = RawYear::empty(year);
    y.sales = amt(sales);
    y.eps = amt(eps);
    y.high_price = amt(high);
    y.low_price = amt(low);
    y.dividend_per_share = amt(dividend);
    y.pre_tax_profit = amt(ptp);
    y.book_value_per_share = amt(book_value);
    y
}

fn canonical(years: Vec<RawYear>) -> CanonicalFinancials {
    normalize(steadyinvest_core::normalize::RawFinancials {
        native_currency: "USD".to_string(),
        years,
        splits: vec![],
    })
    .expect("structurally valid fixture")
}

/// The worked example: 5 usable years of 10%/yr sales & EPS growth (exact powers of 1.1),
/// constant high P/E 20 / low P/E 10 / payout 30% / ROE 20%, PTP marching down 12→8%.
fn tutorial_financials() -> CanonicalFinancials {
    canonical(vec![
        //              sales      eps       high      low       div       ptp        bvps
        full_year(
            2021, "1000", "1.00", "20.00", "10.00", "0.30", "120", "5.00",
        ),
        full_year(
            2022, "1100", "1.10", "22.00", "11.00", "0.33", "121", "5.50",
        ),
        full_year(
            2023, "1210", "1.21", "24.20", "12.10", "0.363", "121", "6.05",
        ),
        full_year(
            2024, "1331", "1.331", "26.62", "13.31", "0.3993", "119.79", "6.655",
        ),
        full_year(
            2025, "1464.1", "1.4641", "29.282", "14.641", "0.43923", "117.128", "7.3205",
        ),
    ])
}

fn tutorial_judgment() -> JudgmentInputs {
    let mut j = JudgmentInputs::empty();
    // Direct high-EPS judgment (wins over the derived 1.4641 × 1.1^5 = 2.357947691);
    // 2.48832 = 18 × 1.2^5 / 18 — chosen so the §5 annualised rate is exactly 20%.
    j.estimated_high_eps = Some(d("2.48832"));
    j.estimated_low_eps = Some(d("1.40"));
    j.projected_sales_growth_pct = Some(d("10"));
    j.projected_eps_growth_pct = Some(d("10"));
    j.judged_avg_high_pe = Some(d("18"));
    j.judged_avg_low_pe = Some(d("9"));
    j.forecast_low_option = ForecastLowOption::AvgLowPeTimesEps;
    j.current_price = Some(d("18"));
    j.present_full_year_dividend = Some(d("0.45"));
    j
}

fn tutorial_observations() -> QuarterlyObservations {
    let mut o = QuarterlyObservations::empty();
    o.ttm_quarterly_eps = Some([d("0.40"), d("0.40"), d("0.40"), d("0.40")]); // TTM = 1.60
    o.latest_quarter_sales = Some(d("400"));
    o.year_ago_quarter_sales = Some(d("320")); // +25%
    o.latest_quarter_eps = Some(d("0.40"));
    o.year_ago_quarter_eps = Some(d("0.32")); // +25%
    o
}

#[track_caller]
fn assert_close(actual: Option<Decimal>, expected: &str, eps: &str, label: &str) {
    let actual = actual.unwrap_or_else(|| panic!("{label}: expected ~{expected}, got None"));
    let diff = (actual - d(expected)).abs();
    assert!(
        diff <= d(eps),
        "{label}: expected {expected} ± {eps}, got {actual} (diff {diff})"
    );
}

// ── The worked example, section by section ──

#[test]
fn worked_example_section_1_growth() {
    let out = compute(
        &tutorial_financials(),
        &tutorial_judgment(),
        &tutorial_observations(),
    );
    // Endpoints: 1000 → 1464.1 over 2025−2021 = 4 years; ratio 1.4641 = 1.1^4 ⇒ exactly 10%.
    assert_close(
        out.growth.sales_cagr_pct,
        "10",
        "0.00000000000000000001",
        "sales CAGR",
    );
    assert_close(
        out.growth.eps_cagr_pct,
        "10",
        "0.00000000000000000001",
        "EPS CAGR",
    );
    assert_eq!(
        out.growth.quarterly_sales_change_pct,
        Some(d("25")),
        "(400 − 320) / 320 × 100 = 25 exactly"
    );
    assert_eq!(out.growth.quarterly_eps_change_pct, Some(d("25")));
    assert_eq!(
        out.growth.estimated_high_eps,
        Some(d("2.48832")),
        "the direct judgment must win over the derived projection"
    );
    assert_eq!(
        out.growth.estimated_low_eps,
        Some(d("1.40")),
        "low EPS is direct-only"
    );
}

#[test]
fn worked_example_derived_high_eps_when_no_direct_judgment() {
    let mut judgment = tutorial_judgment();
    judgment.estimated_high_eps = None;
    let out = compute(&tutorial_financials(), &judgment, &tutorial_observations());
    assert_eq!(
        out.growth.estimated_high_eps,
        Some(d("2.357947691")),
        "derived: 1.4641 × 1.1^5 (exact integer power)"
    );
}

#[test]
fn worked_example_section_2_management() {
    let out = compute(
        &tutorial_financials(),
        &tutorial_judgment(),
        &tutorial_observations(),
    );
    let m = &out.management;
    let ptp: Vec<Option<Decimal>> = m.per_year.iter().map(|y| y.ptp_pct).collect();
    assert_eq!(
        ptp,
        vec![
            Some(d("12")),
            Some(d("11")),
            Some(d("10")),
            Some(d("9")),
            Some(d("8"))
        ],
        "per-year PTP% = ptp/sales × 100"
    );
    for y in &m.per_year {
        assert_eq!(
            y.roe_pct,
            Some(d("20")),
            "year {}: eps/bvps = 0.2 exactly",
            y.year
        );
    }
    assert_eq!(m.avg_ptp_pct, Some(d("10")), "(12+11+10+9+8)/5");
    assert_eq!(m.avg_roe_pct, Some(d("20")));
    assert_eq!(m.latest_ptp_pct, Some(d("8")));
    assert_eq!(m.latest_roe_pct, Some(d("20")));
    assert_eq!(
        m.ptp_trend,
        Some(steadyinvest_core::ssg::Trend::Down),
        "latest 8 vs avg 10: below the ±0.5 pp band"
    );
    assert_eq!(
        m.roe_trend,
        Some(steadyinvest_core::ssg::Trend::Even),
        "latest 20 == avg 20"
    );
}

#[test]
fn worked_example_section_3_valuation() {
    let out = compute(
        &tutorial_financials(),
        &tutorial_judgment(),
        &tutorial_observations(),
    );
    let v = &out.valuation;
    assert_eq!(v.per_year.len(), 5);
    for y in &v.per_year {
        assert_eq!(y.high_pe, Some(d("20")), "year {}: high/eps", y.year);
        assert_eq!(y.low_pe, Some(d("10")), "year {}: low/eps", y.year);
        assert_eq!(
            y.payout_pct,
            Some(d("30")),
            "year {}: div/eps × 100",
            y.year
        );
        assert_eq!(
            y.high_yield_pct,
            Some(d("3")),
            "year {}: div/low × 100",
            y.year
        );
    }
    assert_eq!(v.avg_high_pe, Some(d("20")));
    assert_eq!(v.avg_low_pe, Some(d("10")));
    assert_eq!(v.avg_pe, Some(d("15")), "(20 + 10) / 2");
    assert_eq!(v.avg_payout_pct, Some(d("30")));
    assert_eq!(v.avg_high_yield_pct, Some(d("3")));
    assert_eq!(
        v.avg_low_price,
        Some(d("12.2102")),
        "(10 + 11 + 12.1 + 13.31 + 14.641) / 5 = 61.051 / 5"
    );
    assert_eq!(v.ttm_eps, Some(d("1.60")), "Σ of the 4 quarterly EPS");
    assert_eq!(v.current_pe, Some(d("11.25")), "18 / 1.60");
    assert_eq!(v.relative_value_pct, Some(d("75")), "11.25 / 15 × 100");
}

#[test]
fn worked_example_section_4_risk_reward() {
    let out = compute(
        &tutorial_financials(),
        &tutorial_judgment(),
        &tutorial_observations(),
    );
    let r = &out.risk_reward;
    assert_eq!(r.forecast_high, Some(d("44.78976")), "18 × 2.48832");
    assert_eq!(r.forecast_low, Some(d("12.6")), "option (a): 9 × 1.40");
    let zones = r.zones.as_ref().expect("positive range 32.18976");
    assert_eq!(zones.buy_top, d("23.32992"), "12.6 + 32.18976/3");
    assert_eq!(zones.neutral_top, d("34.05984"), "12.6 + 2 × 10.72992");
    assert_eq!(
        r.present_price_zone,
        Some(Zone::Buy),
        "18 ∈ [12.6, 23.32992]"
    );
    match r.upside_downside {
        UpsideDownside::Ratio(ud) => {
            // (44.78976 − 18) / (18 − 12.6) = 26.78976 / 5.4 = 4.96106̄ (non-terminating).
            let diff = (ud - d("4.961066666666666666666666667")).abs();
            assert!(
                diff <= d("0.000000000000000001"),
                "U/D: got {ud} (diff {diff})"
            );
        }
        ref other => panic!("U/D must be a measured ratio, got {other:?}"),
    }
}

#[test]
fn worked_example_section_5_returns() {
    let out = compute(
        &tutorial_financials(),
        &tutorial_judgment(),
        &tutorial_observations(),
    );
    let r = &out.returns;
    assert_eq!(r.present_yield_pct, Some(d("2.5")), "0.45 / 18 × 100");
    assert_eq!(
        r.avg_annual_eps,
        Some(d("1.9664649202")),
        "mean of 1.4641 × 1.1^y for y = 1..=5 (exact integer powers, sum 9.832324601)"
    );
    assert_eq!(
        r.avg_annual_dividend,
        Some(d("0.58993947606")),
        "1.9664649202 × 30 / 100"
    );
    assert_close(
        r.avg_yield_pct,
        "3.277441533666666666666666667",
        "0.000000000000000001",
        "0.58993947606 / 18 × 100 (non-terminating)",
    );
    assert_eq!(
        r.projected_appreciation_pct,
        Some(d("148.832")),
        "26.78976 / 18 × 100"
    );
    // (44.78976/18)^(1/5) = 2.48832^0.2 = 1.2 exactly ⇒ 20% annualised, + avg yield.
    assert_close(
        r.projected_total_annualized_return_pct,
        "23.277441533666666666666666667",
        "0.000000000000001",
        "20% annualised appreciation + 3.2774…% average yield",
    );
}

#[test]
fn worked_example_flags_findings_confidence_verdict() {
    let out = compute(
        &tutorial_financials(),
        &tutorial_judgment(),
        &tutorial_observations(),
    );
    assert_eq!(
        out.quality_flags,
        vec![QualityFlagKey::PtpTrendDeclining],
        "only the declining PTP trend fires: ROE 20 ≥ 10, equal CAGRs do not lag, \
         judged P/E 18 ≤ 20, U/D 4.96 in [3, 15], relative value 75 < 100"
    );
    assert_eq!(
        out.findings,
        vec![],
        "a coherent study raises no plausibility findings"
    );
    assert!(!out.low_confidence, "5 usable years is full confidence");
    let v = &out.verdict_facts;
    assert_eq!(v.present_price_zone, Some(Zone::Buy));
    assert_eq!(v.ud_at_or_above_target, CriterionFact::Met, "4.96 ≥ 3.0");
    assert_eq!(
        v.relative_value_below_ceiling,
        CriterionFact::Met,
        "75 < 100"
    );
    assert_eq!(v.present_price_in_buy_zone, CriterionFact::Met);
    assert_eq!(
        v.appreciation_at_or_above_double,
        CriterionFact::Met,
        "148.832 ≥ 100"
    );
    assert!(v.quality_value_candidate, "all four criteria are Met");
}

// ── Spec §9 degenerate-input table — each engine row ──

/// Row 1: U/D denominator ≤ 0. Equality (`current == forecast_low`) is allowed by the §4
/// constraint ⇒ Undefined but NO finding; strictly below ⇒ Undefined + `low_price_above_current`.
#[test]
fn s9_ud_denominator_nonpositive_is_undefined_not_a_number() {
    let financials = tutorial_financials();
    let observations = tutorial_observations();

    let mut at_low = tutorial_judgment();
    at_low.current_price = Some(d("12.6")); // == forecast_low
    let out = compute(&financials, &at_low, &observations);
    assert_eq!(out.risk_reward.upside_downside, UpsideDownside::Undefined);
    assert!(
        out.findings.is_empty(),
        "equality satisfies the §4 constraint — no finding: {:?}",
        out.findings
    );
    assert_eq!(
        out.verdict_facts.ud_at_or_above_target,
        CriterionFact::UnmetByInsufficiency,
        "the verdict withholds the U/D criterion"
    );
    assert!(
        !out.quality_flags.contains(&QualityFlagKey::UdBelowTarget)
            && !out.quality_flags.contains(&QualityFlagKey::UdExtreme),
        "U/D flags are never raised on an undefined ratio: {:?}",
        out.quality_flags
    );

    let mut below_low = tutorial_judgment();
    below_low.current_price = Some(d("12.5")); // < forecast_low
    let out = compute(&financials, &below_low, &observations);
    assert_eq!(out.risk_reward.upside_downside, UpsideDownside::Undefined);
    assert!(
        out.findings.contains(&CalcFinding {
            key: PlausibilityKey::LowPriceAboveCurrent,
            year: None,
            context: "forecast_low",
        }),
        "current strictly below the selected forecast low must raise the finding: {:?}",
        out.findings
    );
    assert_eq!(
        out.risk_reward.present_price_zone, None,
        "a price below the range has no zone"
    );
}

/// Row 2: CAGR base ≤ 0 or sign-crossing ⇒ unknown, never 0; `eps_lags_sales` is then never
/// raised (one CAGR unknown is not a comparison result).
#[test]
fn s9_cagr_degenerate_bases_are_unknown_and_raise_no_flag() {
    // First usable year has negative EPS (sign-crossing to +1.4641): EPS CAGR unknown.
    let mut years = vec![
        full_year(
            2021, "1000", "-1.00", "20.00", "10.00", "0.30", "120", "5.00",
        ),
        full_year(
            2022, "1100", "1.10", "22.00", "11.00", "0.33", "121", "5.50",
        ),
        full_year(
            2023, "1210", "1.21", "24.20", "12.10", "0.363", "121", "6.05",
        ),
        full_year(
            2024, "1331", "1.331", "26.62", "13.31", "0.3993", "119.79", "6.655",
        ),
        full_year(
            2025, "1464.1", "1.4641", "29.282", "14.641", "0.43923", "117.128", "7.3205",
        ),
    ];
    let out = compute(
        &canonical(years.clone()),
        &tutorial_judgment(),
        &tutorial_observations(),
    );
    assert_eq!(
        out.growth.eps_cagr_pct, None,
        "sign-crossing start ⇒ unknown, never 0"
    );
    assert_close(
        out.growth.sales_cagr_pct,
        "10",
        "0.00000000000000000001",
        "sales CAGR",
    );
    assert!(
        !out.quality_flags.contains(&QualityFlagKey::EpsLagsSales),
        "eps_lags_sales must not be raised when a CAGR is unknown: {:?}",
        out.quality_flags
    );

    // n = 0: a single usable year has no span.
    years.truncate(1);
    years[0].eps = amt("1.00");
    let out = compute(
        &canonical(years),
        &tutorial_judgment(),
        &tutorial_observations(),
    );
    assert_eq!(out.growth.sales_cagr_pct, None, "n = 0 ⇒ unknown");
    assert_eq!(out.growth.eps_cagr_pct, None);
}

/// Row 3: TTM EPS ≤ 0 ⇒ current P/E, relative value and `relative_value_high` ALL unknown;
/// the relative-value criterion is unmet-by-insufficiency.
#[test]
fn s9_nonpositive_ttm_eps_makes_current_pe_and_relative_value_unknown() {
    let mut observations = tutorial_observations();
    observations.ttm_quarterly_eps = Some([d("-1.00"), d("0.20"), d("0.20"), d("0.20")]); // −0.40
    let out = compute(&tutorial_financials(), &tutorial_judgment(), &observations);
    assert_eq!(
        out.valuation.ttm_eps,
        Some(d("-0.40")),
        "the sum itself is reported"
    );
    assert_eq!(
        out.valuation.current_pe, None,
        "TTM ≤ 0 ⇒ current P/E unknown"
    );
    assert_eq!(out.valuation.relative_value_pct, None);
    assert!(
        !out.quality_flags
            .contains(&QualityFlagKey::RelativeValueHigh),
        "unknown is not ≥ 100 — the flag must not be raised: {:?}",
        out.quality_flags
    );
    assert_eq!(
        out.verdict_facts.relative_value_below_ceiling,
        CriterionFact::UnmetByInsufficiency
    );
    assert!(
        out.findings.contains(&CalcFinding {
            key: PlausibilityKey::NegativeOrZeroDenominator,
            year: None,
            context: "ttm_eps",
        }),
        "a non-positive TTM denominator is flagged: {:?}",
        out.findings
    );
    assert!(
        !out.verdict_facts.quality_value_candidate,
        "an insufficient criterion can never make a candidate"
    );
}

/// Rows 4 & 5: per-year EPS ≤ 0 / book value ≤ 0 ⇒ that year's ratios unknown
/// (`negative_or_zero_denominator`), excluded from the 5-yr averages.
#[test]
fn s9_per_year_nonpositive_denominators_are_excluded_from_averages() {
    // 2023 has EPS = −1.21 (still a usable year: the field is present) and bvps = 0.
    let years = vec![
        full_year(
            2021, "1000", "1.00", "20.00", "10.00", "0.30", "120", "5.00",
        ),
        full_year(
            2022, "1100", "1.10", "22.00", "11.00", "0.33", "121", "5.50",
        ),
        full_year(2023, "1210", "-1.21", "24.20", "12.10", "0.363", "121", "0"),
        full_year(
            2024, "1331", "1.331", "26.62", "13.31", "0.3993", "119.79", "6.655",
        ),
        full_year(
            2025, "1464.1", "1.4641", "29.282", "14.641", "0.43923", "117.128", "7.3205",
        ),
    ];
    let out = compute(
        &canonical(years),
        &tutorial_judgment(),
        &tutorial_observations(),
    );

    let y2023 = out
        .valuation
        .per_year
        .iter()
        .find(|y| y.year == 2023)
        .expect("2023 is usable (all load-bearing fields present)");
    assert_eq!(y2023.high_pe, None, "EPS ≤ 0 ⇒ P/Es unknown");
    assert_eq!(y2023.low_pe, None);
    assert_eq!(y2023.payout_pct, None, "EPS ≤ 0 ⇒ payout unknown");
    assert_eq!(
        out.valuation.avg_high_pe,
        Some(d("20")),
        "2023 is excluded: the other four years all have high P/E 20"
    );
    assert_eq!(out.valuation.avg_low_pe, Some(d("10")));
    assert_eq!(out.valuation.avg_payout_pct, Some(d("30")));

    let roe_2023 = out
        .management
        .per_year
        .iter()
        .find(|y| y.year == 2023)
        .expect("2023 present in per-year ratios");
    assert_eq!(roe_2023.roe_pct, None, "book value ≤ 0 ⇒ ROE unknown");
    assert_eq!(
        out.management.avg_roe_pct,
        Some(d("20")),
        "2023 excluded from the ROE average"
    );

    let finding = |context: &'static str| CalcFinding {
        key: PlausibilityKey::NegativeOrZeroDenominator,
        year: Some(2023),
        context,
    };
    for context in ["eps", "book_value_per_share"] {
        assert!(
            out.findings.contains(&finding(context)),
            "missing {context} finding for 2023: {:?}",
            out.findings
        );
    }
}

/// Row 7: forecast-low option (d) with average high yield ≤ 0 or unknown is NOT selectable.
#[test]
fn s9_dividend_supported_option_needs_a_positive_average_high_yield() {
    let mut judgment = tutorial_judgment();
    judgment.forecast_low_option = ForecastLowOption::DividendSupported;

    // Zero dividends across the window ⇒ avg high yield = 0 ⇒ not selectable.
    let zero_dividends = canonical(vec![
        full_year(2021, "1000", "1.00", "20.00", "10.00", "0", "120", "5.00"),
        full_year(2022, "1100", "1.10", "22.00", "11.00", "0", "121", "5.50"),
        full_year(2023, "1210", "1.21", "24.20", "12.10", "0", "121", "6.05"),
        full_year(
            2024, "1331", "1.331", "26.62", "13.31", "0", "119.79", "6.655",
        ),
        full_year(
            2025, "1464.1", "1.4641", "29.282", "14.641", "0", "117.128", "7.3205",
        ),
    ]);
    let out = compute(&zero_dividends, &judgment, &tutorial_observations());
    assert_eq!(out.valuation.avg_high_yield_pct, Some(d("0")));
    assert_eq!(
        out.risk_reward.forecast_low, None,
        "option (d) with yield ≤ 0 must degrade to unknown, never divide"
    );
    assert_eq!(out.risk_reward.zones, None);
    assert_eq!(out.risk_reward.upside_downside, UpsideDownside::Unknown);

    // The happy path of option (d): tutorial avg high yield is 3% ⇒ 0.45 / 0.03 = 15.
    let out = compute(&tutorial_financials(), &judgment, &tutorial_observations());
    assert_eq!(
        out.risk_reward.forecast_low,
        Some(d("15")),
        "0.45 / (3/100)"
    );
}

/// Degenerate range (`forecast_high ≤ forecast_low`) ⇒ zones and U/D unknown — never
/// inverted bands (AC 5).
#[test]
fn degenerate_forecast_range_yields_no_zones_and_unknown_ud() {
    let mut judgment = tutorial_judgment();
    judgment.judged_avg_high_pe = Some(d("5")); // forecast high = 5 × 2.48832 = 12.4416 < 12.6
    let out = compute(&tutorial_financials(), &judgment, &tutorial_observations());
    assert_eq!(out.risk_reward.forecast_high, Some(d("12.4416")));
    assert_eq!(out.risk_reward.forecast_low, Some(d("12.6")));
    assert_eq!(out.risk_reward.zones, None, "never inverted bands");
    assert_eq!(out.risk_reward.present_price_zone, None);
    assert_eq!(out.risk_reward.upside_downside, UpsideDownside::Unknown);
    assert_eq!(
        out.verdict_facts.present_price_in_buy_zone,
        CriterionFact::UnmetByInsufficiency
    );
}

/// AC 6 gate (recorded interpretation): `current_price ≤ 0` ⇒ every §5 output unknown.
#[test]
fn nonpositive_current_price_makes_all_section_5_outputs_unknown() {
    let mut judgment = tutorial_judgment();
    judgment.current_price = Some(d("0"));
    let out = compute(&tutorial_financials(), &judgment, &tutorial_observations());
    let r = &out.returns;
    assert_eq!(
        (
            r.present_yield_pct,
            r.avg_annual_eps,
            r.avg_annual_dividend,
            r.avg_yield_pct,
            r.projected_appreciation_pct,
            r.projected_total_annualized_return_pct,
        ),
        (None, None, None, None, None, None),
        "current_price ≤ 0 gates ALL §5 outputs to unknown"
    );
}

/// Out-of-range present price (above forecast high) ⇒ zone unknown, no finding (the §4
/// constraint binds only the LOW side).
#[test]
fn present_price_above_forecast_high_has_no_zone() {
    let mut judgment = tutorial_judgment();
    judgment.current_price = Some(d("50")); // > forecast high 44.78976
    let out = compute(&tutorial_financials(), &judgment, &tutorial_observations());
    assert_eq!(out.risk_reward.present_price_zone, None);
    assert!(
        !out.findings
            .iter()
            .any(|f| f.key == PlausibilityKey::LowPriceAboveCurrent),
        "no low-side violation here: {:?}",
        out.findings
    );
    match out.risk_reward.upside_downside {
        UpsideDownside::Ratio(ud) => assert!(
            ud < Decimal::ZERO,
            "above the range the upside is negative, got {ud}"
        ),
        ref other => panic!("U/D is still measurable above the range, got {other:?}"),
    }
}

// ── Boundary comparators (spec §7: normative operators) ──

#[test]
fn boundary_projected_high_pe_flags_are_strict() {
    let financials = tutorial_financials();
    let observations = tutorial_observations();
    let with_pe = |pe: &str| {
        let mut j = tutorial_judgment();
        j.judged_avg_high_pe = Some(d(pe));
        compute(&financials, &j, &observations).quality_flags
    };
    let flags20 = with_pe("20");
    assert!(
        !flags20.contains(&QualityFlagKey::ProjectedHighPeAggressive),
        "> 20 excludes exactly 20: {flags20:?}"
    );
    let flags_above_20 = with_pe("20.01");
    assert!(flags_above_20.contains(&QualityFlagKey::ProjectedHighPeAggressive));
    assert!(!flags_above_20.contains(&QualityFlagKey::ProjectedHighPeImplausible));
    let flags25 = with_pe("25");
    assert!(
        !flags25.contains(&QualityFlagKey::ProjectedHighPeImplausible),
        "> 25 excludes exactly 25: {flags25:?}"
    );
    let flags_above_25 = with_pe("25.01");
    assert!(
        flags_above_25.contains(&QualityFlagKey::ProjectedHighPeAggressive)
            && flags_above_25.contains(&QualityFlagKey::ProjectedHighPeImplausible),
        "both rows fire above 25 (independent catalog rows): {flags_above_25:?}"
    );
}

#[test]
fn boundary_ud_target_and_extreme() {
    let financials = tutorial_financials();
    let observations = tutorial_observations();
    // forecast_low = 12.6, forecast_high = 44.78976 (range 32.18976).
    let with_price = |price: &str| {
        let mut j = tutorial_judgment();
        j.current_price = Some(d(price));
        compute(&financials, &j, &observations)
    };
    // U/D = 3 exactly: (44.78976 − c) / (c − 12.6) = 3 ⇒ c = (44.78976 + 3×12.6)/4 = 20.64744.
    let out = with_price("20.64744");
    assert_eq!(
        out.risk_reward.upside_downside,
        UpsideDownside::Ratio(d("3")),
        "constructed for U/D exactly 3"
    );
    assert!(
        !out.quality_flags.contains(&QualityFlagKey::UdBelowTarget),
        "< 3.0 is strict — exactly 3.0 is not below target: {:?}",
        out.quality_flags
    );
    assert_eq!(
        out.verdict_facts.ud_at_or_above_target,
        CriterionFact::Met,
        "≥ 3.0 includes exactly 3.0"
    );
    // U/D = 15 exactly: c = (44.78976 + 15×12.6)/16 = 14.61186.
    let out = with_price("14.61186");
    assert_eq!(
        out.risk_reward.upside_downside,
        UpsideDownside::Ratio(d("15"))
    );
    assert!(
        !out.quality_flags.contains(&QualityFlagKey::UdExtreme),
        "> 15.0 is strict — exactly 15.0 is not extreme: {:?}",
        out.quality_flags
    );
    // Just inside the low end pushes U/D above 15.
    let out = with_price("14.5");
    match out.risk_reward.upside_downside {
        UpsideDownside::Ratio(ud) => {
            assert!(ud > d("15"), "got {ud}");
            assert!(out.quality_flags.contains(&QualityFlagKey::UdExtreme));
        }
        ref other => panic!("expected a ratio, got {other:?}"),
    }
}

#[test]
fn boundary_relative_value_ceiling_is_ge_for_flag_and_lt_for_criterion() {
    let financials = tutorial_financials();
    let observations = tutorial_observations(); // TTM 1.6, avg P/E 15
    let mut judgment = tutorial_judgment();
    judgment.current_price = Some(d("24")); // current P/E 15 ⇒ relative value exactly 100
    let out = compute(&financials, &judgment, &observations);
    assert_eq!(out.valuation.relative_value_pct, Some(d("100")));
    assert!(
        out.quality_flags
            .contains(&QualityFlagKey::RelativeValueHigh),
        "the flag is ≥ 100 — exactly 100 fires: {:?}",
        out.quality_flags
    );
    assert_eq!(
        out.verdict_facts.relative_value_below_ceiling,
        CriterionFact::Unmet,
        "the criterion is < 100 — exactly 100 is a measured miss, not insufficiency"
    );
}

#[test]
fn boundary_roe_low_is_strict_below_10() {
    let observations = tutorial_observations();
    // bvps = eps × 10 ⇒ ROE exactly 10 every year.
    let roe_10 = canonical(vec![
        full_year(
            2021, "1000", "1.00", "20.00", "10.00", "0.30", "120", "10.00",
        ),
        full_year(
            2022, "1100", "1.10", "22.00", "11.00", "0.33", "121", "11.00",
        ),
        full_year(
            2023, "1210", "1.21", "24.20", "12.10", "0.363", "121", "12.10",
        ),
        full_year(
            2024, "1331", "1.331", "26.62", "13.31", "0.3993", "119.79", "13.31",
        ),
        full_year(
            2025, "1464.1", "1.4641", "29.282", "14.641", "0.43923", "117.128", "14.641",
        ),
    ]);
    let out = compute(&roe_10, &tutorial_judgment(), &observations);
    assert_eq!(out.management.latest_roe_pct, Some(d("10")));
    assert!(
        !out.quality_flags.contains(&QualityFlagKey::RoeLow),
        "< 10 is strict — exactly 10 is not low: {:?}",
        out.quality_flags
    );
}

#[test]
fn boundary_appreciation_criterion_is_ge_100() {
    let financials = tutorial_financials();
    let observations = tutorial_observations();
    let mut judgment = tutorial_judgment();
    // forecast high 44.78976; appreciation exactly 100% needs current = high/2 = 22.39488.
    judgment.current_price = Some(d("22.39488"));
    let out = compute(&financials, &judgment, &observations);
    assert_eq!(out.returns.projected_appreciation_pct, Some(d("100")));
    assert_eq!(
        out.verdict_facts.appreciation_at_or_above_double,
        CriterionFact::Met,
        "≥ 100 includes exactly 100 (recorded interpretation of 'roughly doubling')"
    );
}

// ── Confidence & smaller windows (FR8) ──

#[test]
fn low_confidence_still_computes_on_available_data() {
    // Only 3 usable years — below USABLE_YEARS_FLOOR.
    let years = vec![
        full_year(
            2023, "1210", "1.21", "24.20", "12.10", "0.363", "121", "6.05",
        ),
        full_year(
            2024, "1331", "1.331", "26.62", "13.31", "0.3993", "119.79", "6.655",
        ),
        full_year(
            2025, "1464.1", "1.4641", "29.282", "14.641", "0.43923", "117.128", "7.3205",
        ),
    ];
    let out = compute(
        &canonical(years),
        &tutorial_judgment(),
        &tutorial_observations(),
    );
    assert!(out.low_confidence, "3 usable years < 5");
    assert_eq!(
        out.valuation.avg_high_pe,
        Some(d("20")),
        "averages still run on 3 years"
    );
    assert_eq!(out.management.avg_roe_pct, Some(d("20")));
    assert_close(
        out.growth.sales_cagr_pct,
        "10",
        "0.00000000000000000001",
        "2-year-span CAGR",
    );
}

/// Quarterly degenerate inputs: year-ago ≤ 0 or absent ⇒ unknown (recorded interpretation).
#[test]
fn quarterly_changes_with_nonpositive_year_ago_are_unknown() {
    let mut observations = tutorial_observations();
    observations.year_ago_quarter_sales = Some(d("0"));
    observations.year_ago_quarter_eps = None;
    let out = compute(&tutorial_financials(), &tutorial_judgment(), &observations);
    assert_eq!(out.growth.quarterly_sales_change_pct, None);
    assert_eq!(out.growth.quarterly_eps_change_pct, None);
}

/// Determinism smoke check at the explicit level (the property version lives in
/// `ssg_metamorphic.rs`).
#[test]
fn identical_inputs_produce_identical_outputs() {
    let financials = tutorial_financials();
    let judgment = tutorial_judgment();
    let observations = tutorial_observations();
    assert_eq!(
        compute(&financials, &judgment, &observations),
        compute(&financials, &judgment, &observations)
    );
}
