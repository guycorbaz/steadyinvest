//! Metamorphic & property suite for `core::normalize` (Story 1.7, AC 7).
//!
//! All invariances are asserted at the `CanonicalFinancials` level with **zero tolerance**
//! (`assert_eq!` / `prop_assert_eq!`): the inputs are exact-by-construction (the Spike-C
//! discipline) so every rebase/gross-up division is exact. The ACs phrased at verdict level
//! ("equivalent IFRS/GAAP inputs ⇒ same verdict", "ratios/verdict unchanged under scaling")
//! are satisfied here by identical/exactly-scaled canonical outputs — once the engine exists
//! (Story 1.8), identical canonical input deterministically implies the verdict-level
//! property; 1.8's suite extends these metamorphic tests through the engine.

use proptest::collection::vec;
use proptest::option;
use proptest::prelude::*;
use rust_decimal::Decimal;
use steadyinvest_core::normalize::{
    normalize, CanonicalFinancials, RawAmount, RawFinancials, RawYear, SplitEvent,
};

const NATIVE: &str = "USD";
const FIRST_YEAR: i32 = 2019;

fn amt(value: Decimal) -> Option<RawAmount> {
    Some(RawAmount {
        value,
        currency: NATIVE.to_string(),
    })
}

/// Exact-by-construction positive money: bounded mantissa/scale so every product/quotient in
/// the metamorphic constructions below stays far inside `Decimal`'s 28 significant digits —
/// no truncation, hence zero-tolerance equality.
fn money() -> impl Strategy<Value = Decimal> {
    (1i64..=9_999_999, 0u32..=2).prop_map(|(m, s)| Decimal::new(m, s))
}

fn opt_money() -> impl Strategy<Value = Option<Decimal>> {
    option::of(money())
}

/// Optional values for one year:
/// (sales, eps, high_price, low_price, dividend_per_share, book_value_per_share).
type YearParts = (
    Option<Decimal>,
    Option<Decimal>,
    Option<Decimal>,
    Option<Decimal>,
    Option<Decimal>,
    Option<Decimal>,
);

fn year_parts() -> impl Strategy<Value = YearParts> {
    (
        opt_money(),
        opt_money(),
        opt_money(),
        opt_money(),
        opt_money(),
        opt_money(),
    )
}

/// Build consecutive years starting at [`FIRST_YEAR`] from generated parts.
fn build_years(parts: &[YearParts]) -> Vec<RawYear> {
    parts
        .iter()
        .enumerate()
        .map(|(i, (sales, eps, high, low, dividend, book_value))| {
            let mut y = RawYear::empty(FIRST_YEAR + i32::try_from(i).unwrap_or(0));
            y.sales = sales.and_then(amt);
            y.eps = eps.and_then(amt);
            y.high_price = high.and_then(amt);
            y.low_price = low.and_then(amt);
            y.dividend_per_share = dividend.and_then(amt);
            y.book_value_per_share = book_value.and_then(amt);
            y
        })
        .collect()
}

fn raw(years: Vec<RawYear>, splits: Vec<SplitEvent>) -> RawFinancials {
    RawFinancials {
        native_currency: NATIVE.to_string(),
        years,
        splits,
    }
}

/// Multiply every monetary amount of a year by `k` (tax_rate is a fraction — never scaled).
fn scale_year(year: &RawYear, k: Decimal) -> RawYear {
    let scale = |a: &Option<RawAmount>| {
        a.as_ref().map(|x| RawAmount {
            value: x.value * k,
            currency: x.currency.clone(),
        })
    };
    let mut scaled = RawYear::empty(year.year);
    scaled.period_months = year.period_months;
    scaled.fiscal_year_end_month = year.fiscal_year_end_month;
    scaled.sales = scale(&year.sales);
    scaled.eps = scale(&year.eps);
    scaled.high_price = scale(&year.high_price);
    scaled.low_price = scale(&year.low_price);
    scaled.dividend_per_share = scale(&year.dividend_per_share);
    scaled.pre_tax_profit = scale(&year.pre_tax_profit);
    scaled.net_profit = scale(&year.net_profit);
    scaled.tax_rate = year.tax_rate;
    scaled.book_value_per_share = scale(&year.book_value_per_share);
    scaled
}

fn unwrap_ok(result: Result<CanonicalFinancials, impl std::fmt::Debug>) -> CanonicalFinancials {
    result.expect("structurally valid input must normalize")
}

proptest! {
    /// AC 7 — Split-invariance: a 3:1 split applied to the inputs (pre-split per-share values
    /// × 3 + the declared 3:1 event) yields the SAME canonical series, exactly. The split input
    /// is constructed FROM the unsplit one by exact multiplication, so `(v × 3) / 3 = v` holds
    /// with zero tolerance.
    #[test]
    fn split_invariance_3_for_1(parts in vec(year_parts(), 6), split_index in 1usize..6) {
        let unsplit_years = build_years(&parts);
        let effective_year = FIRST_YEAR + i32::try_from(split_index).unwrap_or(1);
        let three = Decimal::from(3u64);

        let split_years: Vec<RawYear> = unsplit_years
            .iter()
            .map(|y| {
                if y.year < effective_year {
                    let mut pre = scale_year(y, three);
                    // Aggregates are share-count independent: NOT multiplied in the split world.
                    pre.sales = y.sales.clone();
                    pre.pre_tax_profit = y.pre_tax_profit.clone();
                    pre.net_profit = y.net_profit.clone();
                    pre
                } else {
                    y.clone()
                }
            })
            .collect();

        let unsplit = unwrap_ok(normalize(raw(unsplit_years, vec![])));
        let split = unwrap_ok(normalize(raw(
            split_years,
            vec![SplitEvent { effective_year, numerator: 3, denominator: 1 }],
        )));
        prop_assert_eq!(
            &split, &unsplit,
            "declared 3:1 split (effective {}) must rebase to the identical canonical output",
            effective_year
        );
    }

    /// AC 7 — IFRS/GAAP-equivalence: direct-PTP vs net+tax_rate representations of the same
    /// economics yield IDENTICAL `CanonicalFinancials`. `net = ptp × (1 − rate)` is exact by
    /// construction, so the gross-up recovers `ptp` exactly.
    #[test]
    fn ifrs_gaap_equivalence(
        parts in vec(year_parts(), 4),
        ptps in vec(money(), 4),
        rate_mantissas in vec(0i64..100, 4),
    ) {
        let base = build_years(&parts);

        let direct_years: Vec<RawYear> = base
            .iter()
            .zip(&ptps)
            .map(|(y, ptp)| {
                let mut d = y.clone();
                d.pre_tax_profit = amt(*ptp);
                d
            })
            .collect();

        let grossed_years: Vec<RawYear> = base
            .iter()
            .zip(&ptps)
            .zip(&rate_mantissas)
            .map(|((y, ptp), rate_m)| {
                let rate = Decimal::new(*rate_m, 2); // 0.00 ..= 0.99 — always < 1
                let mut g = y.clone();
                g.net_profit = amt(*ptp * (Decimal::ONE - rate)); // exact product
                g.tax_rate = Some(rate);
                g
            })
            .collect();

        let direct = unwrap_ok(normalize(raw(direct_years, vec![])));
        let grossed = unwrap_ok(normalize(raw(grossed_years, vec![])));
        prop_assert_eq!(
            &direct, &grossed,
            "direct PTP and net+tax_rate encode the same economics — canonical output must match"
        );
    }

    /// AC 7 — Scale-homogeneity: multiplying all monetary amounts by k > 0 yields the canonical
    /// series scaled by exactly k, with identical findings and usability. (The ratios/verdict
    /// half of this AC completes in Story 1.8.) Splits and tax rates are drawn from
    /// terminating-quotient sets so both worlds stay exact.
    #[test]
    fn scale_homogeneity(
        parts in vec(year_parts(), 5),
        k_mantissa in 1i64..=999,
        k_scale in 0u32..=2,
        rate_choice in prop::sample::select(vec![None, Some((50i64, 2u32)), Some((20, 2)), Some((75, 2))]),
        with_split in any::<bool>(),
        net in money(),
    ) {
        let k = Decimal::new(k_mantissa, k_scale);
        let mut years = build_years(&parts);
        // Exercise the gross-up path under scaling on the first year.
        if let Some((m, s)) = rate_choice {
            years[0].net_profit = amt(net);
            years[0].tax_rate = Some(Decimal::new(m, s)); // 1−rate ∈ {0.5, 0.8, 0.25}: terminating
        }
        let splits = if with_split {
            vec![SplitEvent { effective_year: FIRST_YEAR + 2, numerator: 2, denominator: 1 }]
        } else {
            vec![]
        };

        let scaled_years: Vec<RawYear> = years.iter().map(|y| scale_year(y, k)).collect();

        let base = unwrap_ok(normalize(raw(years, splits.clone())));
        let scaled = unwrap_ok(normalize(raw(scaled_years, splits)));

        prop_assert_eq!(&scaled.findings, &base.findings,
            "findings must be invariant under scaling by k = {}", k);
        prop_assert_eq!(scaled.usable_years, base.usable_years,
            "usability must be invariant under scaling by k = {}", k);
        for (s, b) in scaled.years.iter().zip(&base.years) {
            prop_assert_eq!(&s.usability, &b.usability,
                "year {} usability must be invariant under scaling", b.year);
            let pairs: [(&str, Option<Decimal>, Option<Decimal>); 7] = [
                ("sales", s.sales, b.sales),
                ("eps", s.eps, b.eps),
                ("high_price", s.high_price, b.high_price),
                ("low_price", s.low_price, b.low_price),
                ("dividend_per_share", s.dividend_per_share, b.dividend_per_share),
                ("pre_tax_profit", s.pre_tax_profit, b.pre_tax_profit),
                ("book_value_per_share", s.book_value_per_share, b.book_value_per_share),
            ];
            for (field, scaled_v, base_v) in pairs {
                prop_assert_eq!(
                    scaled_v, base_v.map(|v| v * k),
                    "year {} field {}: scaled output must equal base × k (k = {})",
                    b.year, field, k
                );
            }
        }
    }

    /// AC 7 — Determinism: `normalize(x) == normalize(x)` over generated inputs (including
    /// structurally invalid ones — equal errors are equality too).
    #[test]
    fn determinism(
        parts in vec(year_parts(), 6),
        splits in vec(
            (2020i32..=2024, 1u32..=5, 1u32..=5).prop_map(|(y, n, d)| SplitEvent {
                effective_year: y,
                numerator: n,
                denominator: d,
            }),
            0..3
        ),
    ) {
        let input = raw(build_years(&parts), splits);
        prop_assert_eq!(
            normalize(input.clone()),
            normalize(input),
            "identical input must produce identical output"
        );
    }

    /// AC 7 — Never-0: a missing load-bearing field never appears as `Some(0)` (it stays `None`),
    /// and the usability state names exactly the missing fields.
    #[test]
    fn never_zero_for_missing_load_bearing(parts in vec(year_parts(), 6)) {
        let years = build_years(&parts);
        let out = unwrap_ok(normalize(raw(years.clone(), vec![])));
        prop_assert_eq!(out.years.len(), years.len());
        let mut usable = 0u32;
        for (raw_y, canon) in years.iter().zip(&out.years) {
            let load_bearing = [
                ("sales", raw_y.sales.is_some(), canon.sales),
                ("eps", raw_y.eps.is_some(), canon.eps),
                ("high_price", raw_y.high_price.is_some(), canon.high_price),
                ("low_price", raw_y.low_price.is_some(), canon.low_price),
            ];
            let mut expected_missing: Vec<&str> = Vec::new();
            for (field, raw_present, canon_value) in load_bearing {
                if raw_present {
                    prop_assert!(canon_value.is_some(),
                        "year {} field {}: present raw value must survive", raw_y.year, field);
                } else {
                    prop_assert_eq!(canon_value, None,
                        "year {} field {}: missing raw value must stay None — NEVER Some(0)",
                        raw_y.year, field);
                    expected_missing.push(field);
                }
            }
            use steadyinvest_core::normalize::YearUsability;
            if expected_missing.is_empty() {
                usable += 1;
                prop_assert_eq!(&canon.usability, &YearUsability::Usable,
                    "year {} has all load-bearing fields", raw_y.year);
            } else {
                prop_assert_eq!(
                    &canon.usability,
                    &YearUsability::Insufficient { missing: expected_missing.clone() },
                    "year {} must name exactly its missing fields", raw_y.year
                );
            }
        }
        prop_assert_eq!(out.usable_years, usable, "usable-year count must match per-year states");
    }
}
