//! Metamorphic & property suite for `core::ssg` (Story 1.8, AC 9) — completes the Story-1.7
//! handoff: the three normalization invariances (IFRS/GAAP equivalence, split invariance,
//! scale homogeneity) are driven THROUGH the engine (`normalize` → `compute`), plus the
//! engine's own properties: determinism, zone ordering (NFR-C3) and U/D non-negativity.
//!
//! Exactness discipline (1.7's lesson, sharpened): dimensionless outputs (ratios, CAGRs,
//! P/Es, U/D, zone classification, flags) are value-identical under scaling because every
//! quotient `(k·a)/(k·b)` is the same real number as `a/b` — asserted with **zero
//! tolerance**. Price-dimensioned outputs scale by exactly k only when every division on
//! their path terminates: the scale test constructs the forecast range as `3 × t` (option
//! (c)) so the §4 thirds terminate, keeps all 5 window years usable so the §2/§3 means
//! divide by 5, and the §5 horizon mean divides by 5 — all terminating, hence `assert_eq!`.

use proptest::collection::vec;
use proptest::option;
use proptest::prelude::*;
use rust_decimal::Decimal;
use steadyinvest_core::normalize::{RawAmount, RawFinancials, RawYear, SplitEvent, normalize};
use steadyinvest_core::ssg::{
    ForecastLowOption, JudgmentInputs, QuarterlyObservations, SsgOutputs, UpsideDownside, compute,
};

const NATIVE: &str = "USD";
const FIRST_YEAR: i32 = 2019;

fn amt(value: Decimal) -> Option<RawAmount> {
    Some(RawAmount {
        value,
        currency: NATIVE.to_string(),
    })
}

/// Exact-by-construction positive money (the Spike-C discipline shared with
/// `normalize_metamorphic.rs`): bounded mantissa/scale keeps every product far inside
/// `Decimal`'s 28 significant digits.
fn money() -> impl Strategy<Value = Decimal> + Clone {
    (1i64..=9_999_999, 0u32..=2).prop_map(|(m, s)| Decimal::new(m, s))
}

/// Signed (possibly zero/negative) values — the no-panic surface for the determinism sweep.
fn signed_money() -> impl Strategy<Value = Decimal> + Clone {
    (-9_999_999i64..=9_999_999, 0u32..=2).prop_map(|(m, s)| Decimal::new(m, s))
}

fn opt(of: impl Strategy<Value = Decimal>) -> impl Strategy<Value = Option<Decimal>> {
    option::of(of)
}

fn forecast_option() -> impl Strategy<Value = ForecastLowOption> {
    prop::sample::select(vec![
        ForecastLowOption::AvgLowPeTimesEps,
        ForecastLowOption::AvgLowPriceLast5y,
        ForecastLowOption::RecentSevereLow,
        ForecastLowOption::DividendSupported,
    ])
}

/// Judgment inputs over a value strategy (positive for the metamorphic constructions,
/// signed for the no-panic determinism sweep).
fn judgment(
    values: impl Strategy<Value = Decimal> + Clone,
) -> impl Strategy<Value = JudgmentInputs> {
    (
        (
            opt(values.clone()),
            opt(values.clone()),
            opt((-500i64..=1000).prop_map(|m| Decimal::new(m, 1))), // growth % −50.0..=100.0
            opt((-500i64..=1000).prop_map(|m| Decimal::new(m, 1))),
            opt(values.clone()),
        ),
        (
            opt(values.clone()),
            forecast_option(),
            opt(values.clone()),
            opt(values.clone()),
            opt(values),
        ),
    )
        .prop_map(
            |(
                (estimated_high_eps, estimated_low_eps, sales_g, eps_g, judged_avg_high_pe),
                (
                    judged_avg_low_pe,
                    forecast_low_option,
                    recent_severe_low,
                    current_price,
                    dividend,
                ),
            )| {
                let mut j = JudgmentInputs::empty();
                j.estimated_high_eps = estimated_high_eps;
                j.estimated_low_eps = estimated_low_eps;
                j.projected_sales_growth_pct = sales_g;
                j.projected_eps_growth_pct = eps_g;
                j.judged_avg_high_pe = judged_avg_high_pe;
                j.judged_avg_low_pe = judged_avg_low_pe;
                j.forecast_low_option = forecast_low_option;
                j.recent_severe_low = recent_severe_low;
                j.current_price = current_price;
                j.present_full_year_dividend = dividend;
                j
            },
        )
}

fn observations(
    values: impl Strategy<Value = Decimal> + Clone,
) -> impl Strategy<Value = QuarterlyObservations> {
    (
        option::of([
            values.clone(),
            values.clone(),
            values.clone(),
            values.clone(),
        ]),
        opt(values.clone()),
        opt(values.clone()),
        opt(values.clone()),
        opt(values),
    )
        .prop_map(|(ttm, ls, le, ys, ye)| {
            let mut o = QuarterlyObservations::empty();
            o.ttm_quarterly_eps = ttm;
            o.latest_quarter_sales = ls;
            o.latest_quarter_eps = le;
            o.year_ago_quarter_sales = ys;
            o.year_ago_quarter_eps = ye;
            o
        })
}

/// Optional values for one year, in `CanonicalYear` field order:
/// (sales, eps, high, low, dividend, pre_tax_profit, book_value).
type YearParts = (
    Option<Decimal>,
    Option<Decimal>,
    Option<Decimal>,
    Option<Decimal>,
    Option<Decimal>,
    Option<Decimal>,
    Option<Decimal>,
);

fn year_parts(values: impl Strategy<Value = Decimal> + Clone) -> impl Strategy<Value = YearParts> {
    (
        opt(values.clone()),
        opt(values.clone()),
        opt(values.clone()),
        opt(values.clone()),
        opt(values.clone()),
        opt(values.clone()),
        opt(values),
    )
}

fn build_years(parts: &[YearParts]) -> Vec<RawYear> {
    parts
        .iter()
        .enumerate()
        .map(|(i, (sales, eps, high, low, dividend, ptp, book_value))| {
            let mut y = RawYear::empty(FIRST_YEAR + i32::try_from(i).unwrap_or(0));
            y.sales = sales.and_then(amt);
            y.eps = eps.and_then(amt);
            y.high_price = high.and_then(amt);
            y.low_price = low.and_then(amt);
            y.dividend_per_share = dividend.and_then(amt);
            y.pre_tax_profit = ptp.and_then(amt);
            y.book_value_per_share = book_value.and_then(amt);
            y
        })
        .collect()
}

/// A fully-populated year from 7 positive values — every window year usable, every mean
/// over exactly 5 values (terminating `/5`).
fn full_year(i: usize, v: &[Decimal; 7]) -> RawYear {
    let mut y = RawYear::empty(FIRST_YEAR + i32::try_from(i).unwrap_or(0));
    y.sales = amt(v[0]);
    y.eps = amt(v[1]);
    y.high_price = amt(v[2]);
    y.low_price = amt(v[3]);
    y.dividend_per_share = amt(v[4]);
    y.pre_tax_profit = amt(v[5]);
    y.book_value_per_share = amt(v[6]);
    y
}

fn raw(years: Vec<RawYear>, splits: Vec<SplitEvent>) -> RawFinancials {
    RawFinancials {
        native_currency: NATIVE.to_string(),
        years,
        splits,
    }
}

fn engine(input: RawFinancials, j: &JudgmentInputs, o: &QuarterlyObservations) -> SsgOutputs {
    let canonical = normalize(input).expect("structurally valid metamorphic input");
    compute(&canonical, j, o)
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

/// Multiply the MONETARY judgment inputs by `k`; the dimensionless judgments (P/Es, growth
/// %) stay fixed — they are ratios, not money (AC 9).
fn scale_judgment(j: &JudgmentInputs, k: Decimal) -> JudgmentInputs {
    let mut s = j.clone();
    s.estimated_high_eps = j.estimated_high_eps.map(|v| v * k);
    s.estimated_low_eps = j.estimated_low_eps.map(|v| v * k);
    s.recent_severe_low = j.recent_severe_low.map(|v| v * k);
    s.current_price = j.current_price.map(|v| v * k);
    s.present_full_year_dividend = j.present_full_year_dividend.map(|v| v * k);
    s
}

/// Multiply every quarterly observation by `k` (they are all monetary).
fn scale_observations(o: &QuarterlyObservations, k: Decimal) -> QuarterlyObservations {
    let mut s = o.clone();
    s.ttm_quarterly_eps = o.ttm_quarterly_eps.map(|q| q.map(|v| v * k));
    s.latest_quarter_sales = o.latest_quarter_sales.map(|v| v * k);
    s.latest_quarter_eps = o.latest_quarter_eps.map(|v| v * k);
    s.year_ago_quarter_sales = o.year_ago_quarter_sales.map(|v| v * k);
    s.year_ago_quarter_eps = o.year_ago_quarter_eps.map(|v| v * k);
    s
}

proptest! {
    /// AC 9 — Determinism: `compute(x) == compute(x)`, swept over signed/zero/missing inputs.
    /// This is also the engine's no-panic sweep: every degenerate denominator, sign-crossing
    /// CAGR base and absent judgment must resolve to a typed unknown, never a panic (spec §9).
    #[test]
    fn determinism_and_no_panic(
        parts in vec(year_parts(signed_money()), 0..7),
        j in judgment(signed_money()),
        o in observations(signed_money()),
    ) {
        let input = raw(build_years(&parts), vec![]);
        let canonical = normalize(input).expect("no duplicate years by construction");
        prop_assert_eq!(
            compute(&canonical, &j, &o),
            compute(&canonical, &j, &o),
            "identical inputs must produce identical outputs"
        );
    }

    /// AC 9 / NFR-C3 — Zones ordered: whenever the engine emits zone bounds, they are
    /// strictly ordered `forecast_low < buy_top < neutral_top < forecast_high`.
    #[test]
    fn zones_are_strictly_ordered_whenever_present(
        parts in vec(year_parts(money()), 0..7),
        j in judgment(money()),
        o in observations(money()),
    ) {
        let out = engine(raw(build_years(&parts), vec![]), &j, &o);
        if let Some(z) = &out.risk_reward.zones {
            prop_assert!(
                z.forecast_low < z.buy_top && z.buy_top < z.neutral_top && z.neutral_top < z.forecast_high,
                "zone bounds must be strictly ordered: {z:?}"
            );
        }
    }

    /// AC 9 — U/D ≥ 0 for a current price within `(forecast_low, forecast_high]`: the price
    /// is re-derived INSIDE the emitted range, so the ratio of two non-negatives holds.
    #[test]
    fn ud_is_nonnegative_for_in_range_prices(
        parts in vec(year_parts(money()), 0..7),
        j in judgment(money()),
        o in observations(money()),
        fraction_mantissa in 1i64..=100,
    ) {
        let probe = engine(raw(build_years(&parts), vec![]), &j, &o);
        let Some(z) = probe.risk_reward.zones else { return Ok(()) };
        // current = low + range × f with f ∈ (0.01..=1.00] ⇒ strictly above low, at most high.
        let f = Decimal::new(fraction_mantissa, 2);
        let mut in_range = j.clone();
        in_range.current_price = Some(z.forecast_low + (z.forecast_high - z.forecast_low) * f);
        let out = engine(raw(build_years(&parts), vec![]), &in_range, &o);
        match out.risk_reward.upside_downside {
            UpsideDownside::Ratio(ud) => prop_assert!(
                ud >= Decimal::ZERO,
                "U/D must be ≥ 0 for an in-range price (f = {f}): got {ud}"
            ),
            UpsideDownside::Undefined => prop_assert!(
                false, "denominator is positive by construction (f = {f})"
            ),
            UpsideDownside::Unknown => {} // zones can vanish at Decimal's precision edge
        }
    }

    /// AC 9 — 1.7 handoff, IFRS/GAAP equivalence THROUGH the engine: direct-PTP vs
    /// net+tax_rate representations of the same economics yield identical `SsgOutputs`.
    #[test]
    fn ifrs_gaap_equivalence_through_engine(
        parts in vec(year_parts(money()), 4),
        ptps in vec(money(), 4),
        rate_mantissas in vec(0i64..100, 4),
        j in judgment(money()),
        o in observations(money()),
    ) {
        let base = build_years(&parts);
        let direct_years: Vec<RawYear> = base
            .iter()
            .zip(&ptps)
            .map(|(y, ptp)| {
                let mut d = y.clone();
                d.pre_tax_profit = amt(*ptp);
                d.net_profit = None;
                d.tax_rate = None;
                d
            })
            .collect();
        let grossed_years: Vec<RawYear> = base
            .iter()
            .zip(&ptps)
            .zip(&rate_mantissas)
            .map(|((y, ptp), rate_m)| {
                let rate = Decimal::new(*rate_m, 2); // 0.00..=0.99 — always < 1
                let mut g = y.clone();
                g.pre_tax_profit = None;
                g.net_profit = amt(*ptp * (Decimal::ONE - rate)); // exact product
                g.tax_rate = Some(rate);
                g
            })
            .collect();
        prop_assert_eq!(
            engine(raw(direct_years, vec![]), &j, &o),
            engine(raw(grossed_years, vec![]), &j, &o),
            "equivalent IFRS/GAAP representations must produce identical SSG outputs"
        );
    }

    /// AC 9 — 1.7 handoff, split invariance THROUGH the engine: a declared 3:1 split applied
    /// to the inputs yields identical `SsgOutputs`. Quarterly observations and judgments are
    /// shared verbatim: both are post-split / forward-looking by the v1 caller contract.
    #[test]
    fn split_invariance_through_engine(
        parts in vec(year_parts(money()), 6),
        split_index in 1usize..6,
        j in judgment(money()),
        o in observations(money()),
    ) {
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
        prop_assert_eq!(
            engine(
                raw(split_years, vec![SplitEvent { effective_year, numerator: 3, denominator: 1 }]),
                &j,
                &o,
            ),
            engine(raw(unsplit_years, vec![]), &j, &o),
            "a declared 3:1 split (effective {}) must not change any SSG output",
            effective_year
        );
    }

    /// AC 9 — 1.7 handoff, scale homogeneity THROUGH the engine: ALL monetary inputs × k
    /// (canonical series + monetary judgments/observations), dimensionless judgments fixed ⇒
    /// dimensionless outputs (ratios, CAGRs, P/Es, U/D, relative value, zone classification,
    /// flags, findings, verdict facts) UNCHANGED and price-dimensioned outputs scaled by
    /// exactly k — zero tolerance (see the module doc for why every division terminates).
    #[test]
    fn scale_homogeneity_through_engine(
        year_values in vec([money(), money(), money(), money(), money(), money(), money()], 5),
        k_mantissa in 1i64..=999,
        k_scale in 0u32..=2,
        eps_estimate in money(),
        high_pe_int in 1u32..=50,
        low_pe in money(),
        range_third in money(),
        current in money(),
        dividend in money(),
        // Integer percent: the projection factor stays at scale 2, so (1+g)^5 has ≤ 11
        // significant digits and every §5 product stays inside Decimal's 28 (see module doc).
        eps_growth_pct in 0i64..=50,
        // Terminating payout: dividend = eps × p with a 2-decimal p makes every payout %
        // exact, keeping avg_annual_dividend's product chain inside 28 digits too.
        payout_fraction in prop::sample::select(vec![
            Decimal::ZERO,
            Decimal::new(10, 2),
            Decimal::new(25, 2),
            Decimal::new(30, 2),
            Decimal::new(50, 2),
        ]),
        ttm in [money(), money(), money(), money()],
    ) {
        let k = Decimal::new(k_mantissa, k_scale);
        let years: Vec<RawYear> = year_values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let mut values = *v;
                values[4] = values[1] * payout_fraction; // dividend = eps × p (exact)
                full_year(i, &values)
            })
            .collect();

        // Terminating-thirds construction: forecast_high = pe × eps (exact product), option
        // (c) low = high − 3·t (> 0 assumed) ⇒ third = t exactly, in BOTH worlds.
        let forecast_high = Decimal::from(high_pe_int) * eps_estimate;
        let forecast_low = forecast_high - Decimal::from(3u32) * range_third;
        prop_assume!(forecast_low > Decimal::ZERO);

        let mut j = JudgmentInputs::empty();
        j.estimated_high_eps = Some(eps_estimate);
        j.estimated_low_eps = Some(low_pe); // any positive money — only option (a) would use it
        j.projected_sales_growth_pct = Some(Decimal::from(eps_growth_pct));
        j.projected_eps_growth_pct = Some(Decimal::from(eps_growth_pct)); // 0..=50, integer
        j.judged_avg_high_pe = Some(Decimal::from(high_pe_int));
        j.judged_avg_low_pe = Some(low_pe);
        j.forecast_low_option = ForecastLowOption::RecentSevereLow;
        j.recent_severe_low = Some(forecast_low);
        j.current_price = Some(current);
        j.present_full_year_dividend = Some(dividend);

        let mut o = QuarterlyObservations::empty();
        o.ttm_quarterly_eps = Some(ttm);
        o.latest_quarter_sales = Some(year_values[4][0]);
        o.year_ago_quarter_sales = Some(year_values[3][0]);
        o.latest_quarter_eps = Some(year_values[4][1]);
        o.year_ago_quarter_eps = Some(year_values[3][1]);

        let base = engine(raw(years.clone(), vec![]), &j, &o);
        let scaled = engine(
            raw(years.iter().map(|y| scale_year(y, k)).collect(), vec![]),
            &scale_judgment(&j, k),
            &scale_observations(&o, k),
        );

        // ── Dimensionless / categorical outputs: value-identical ──
        prop_assert_eq!(&scaled.growth.sales_cagr_pct, &base.growth.sales_cagr_pct,
            "sales CAGR must be scale-invariant (k = {})", k);
        prop_assert_eq!(&scaled.growth.eps_cagr_pct, &base.growth.eps_cagr_pct,
            "EPS CAGR must be scale-invariant (k = {})", k);
        prop_assert_eq!(&scaled.growth.quarterly_sales_change_pct, &base.growth.quarterly_sales_change_pct);
        prop_assert_eq!(&scaled.growth.quarterly_eps_change_pct, &base.growth.quarterly_eps_change_pct);
        prop_assert_eq!(&scaled.management, &base.management,
            "every §2 ratio/average/trend is dimensionless — invariant under k = {}", k);
        prop_assert_eq!(&scaled.valuation.per_year, &base.valuation.per_year,
            "per-year P/Es, payout and yield are dimensionless (k = {})", k);
        prop_assert_eq!(&scaled.valuation.avg_high_pe, &base.valuation.avg_high_pe);
        prop_assert_eq!(&scaled.valuation.avg_low_pe, &base.valuation.avg_low_pe);
        prop_assert_eq!(&scaled.valuation.avg_pe, &base.valuation.avg_pe);
        prop_assert_eq!(&scaled.valuation.avg_payout_pct, &base.valuation.avg_payout_pct);
        prop_assert_eq!(&scaled.valuation.avg_high_yield_pct, &base.valuation.avg_high_yield_pct);
        prop_assert_eq!(&scaled.valuation.current_pe, &base.valuation.current_pe,
            "current P/E is a ratio of two k-scaled values (k = {})", k);
        prop_assert_eq!(&scaled.valuation.relative_value_pct, &base.valuation.relative_value_pct);
        prop_assert_eq!(&scaled.risk_reward.upside_downside, &base.risk_reward.upside_downside,
            "U/D is dimensionless (k = {})", k);
        prop_assert_eq!(&scaled.risk_reward.present_price_zone, &base.risk_reward.present_price_zone,
            "zone classification must not move under scaling (k = {})", k);
        prop_assert_eq!(&scaled.returns.present_yield_pct, &base.returns.present_yield_pct);
        prop_assert_eq!(&scaled.returns.avg_yield_pct, &base.returns.avg_yield_pct);
        prop_assert_eq!(&scaled.returns.projected_appreciation_pct, &base.returns.projected_appreciation_pct);
        prop_assert_eq!(
            &scaled.returns.projected_total_annualized_return_pct,
            &base.returns.projected_total_annualized_return_pct
        );
        prop_assert_eq!(&scaled.quality_flags, &base.quality_flags,
            "flags compare dimensionless metrics to constants (k = {})", k);
        prop_assert_eq!(&scaled.findings, &base.findings,
            "findings are sign/threshold facts about ratios (k = {})", k);
        prop_assert_eq!(scaled.low_confidence, base.low_confidence);
        prop_assert_eq!(&scaled.verdict_facts, &base.verdict_facts,
            "the verdict facts must be invariant under scaling (k = {})", k);

        // ── Price-dimensioned outputs: scaled by exactly k ──
        let times_k = |v: Option<Decimal>| v.map(|x| x * k);
        prop_assert_eq!(scaled.growth.estimated_high_eps, times_k(base.growth.estimated_high_eps),
            "estimated high EPS is monetary (k = {})", k);
        prop_assert_eq!(scaled.growth.estimated_low_eps, times_k(base.growth.estimated_low_eps));
        prop_assert_eq!(scaled.valuation.avg_low_price, times_k(base.valuation.avg_low_price),
            "average low price is monetary, /5 terminates (k = {})", k);
        prop_assert_eq!(scaled.valuation.ttm_eps, times_k(base.valuation.ttm_eps));
        prop_assert_eq!(scaled.risk_reward.forecast_high, times_k(base.risk_reward.forecast_high),
            "forecast high = P/E × (k·EPS) (k = {})", k);
        prop_assert_eq!(scaled.risk_reward.forecast_low, times_k(base.risk_reward.forecast_low));
        match (&scaled.risk_reward.zones, &base.risk_reward.zones) {
            (Some(s), Some(b)) => {
                prop_assert_eq!(s.forecast_low, b.forecast_low * k);
                prop_assert_eq!(s.buy_top, b.buy_top * k,
                    "thirds terminate by construction, so zone bounds scale exactly (k = {})", k);
                prop_assert_eq!(s.neutral_top, b.neutral_top * k);
                prop_assert_eq!(s.forecast_high, b.forecast_high * k);
            }
            (None, None) => {}
            (s, b) => prop_assert!(false, "zones presence diverged under scaling: {s:?} vs {b:?}"),
        }
        prop_assert_eq!(scaled.returns.avg_annual_eps, times_k(base.returns.avg_annual_eps),
            "projected EPS mean is monetary, /5 terminates (k = {})", k);
        prop_assert_eq!(scaled.returns.avg_annual_dividend, times_k(base.returns.avg_annual_dividend));
    }
}
