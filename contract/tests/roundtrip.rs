//! Property test (AC5): for every public contract type, `parse(serialize(x)) == x`.

use proptest::prelude::*;
use rust_decimal::Decimal;
use steadyinvest_contract::{
    Cell, Coverage, ForecastLowOption, Freshness, Judgment, Money, Provenance, Review, Source,
    Study, Timestamp, YearData,
};
use uuid::Uuid;

fn money() -> impl Strategy<Value = Money> {
    (any::<i64>(), 0u32..=10u32).prop_map(|(m, s)| Money::from(Decimal::new(m, s)))
}

fn token() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[a-zA-Z0-9:_.+-]{0,24}").unwrap()
}

fn source() -> impl Strategy<Value = Source> {
    prop_oneof![
        Just(Source::Provider),
        Just(Source::Manual),
        Just(Source::Derived)
    ]
}

fn freshness() -> impl Strategy<Value = Freshness> {
    prop_oneof![Just(Freshness::Current), Just(Freshness::Stale)]
}

fn review() -> impl Strategy<Value = Review> {
    prop_oneof![
        Just(Review::None),
        Just(Review::ToReview),
        Just(Review::Validated)
    ]
}

fn coverage() -> impl Strategy<Value = Coverage> {
    prop_oneof![
        Just(Coverage::Present),
        Just(Coverage::ToFill),
        Just(Coverage::NotAvailableAccepted),
    ]
}

fn forecast_low_option() -> impl Strategy<Value = ForecastLowOption> {
    prop_oneof![
        Just(ForecastLowOption::AvgLowPeTimesEps),
        Just(ForecastLowOption::AvgLowPriceLast5y),
        Just(ForecastLowOption::RecentSevereLow),
        Just(ForecastLowOption::DividendSupported),
    ]
}

fn provenance() -> impl Strategy<Value = Provenance> {
    (source(), any::<u64>(), token(), token()).prop_map(|(source, lv, ts, hash)| Provenance {
        source,
        logical_version: lv,
        timestamp: Timestamp(ts),
        hash_of_dependencies: hash,
    })
}

fn cell() -> impl Strategy<Value = Cell> {
    (
        proptest::option::of(money()),
        source(),
        freshness(),
        review(),
        coverage(),
        provenance(),
    )
        .prop_map(
            |(value, source, freshness, review, coverage, provenance)| Cell {
                value,
                source,
                freshness,
                review,
                coverage,
                provenance,
            },
        )
}

fn year_data() -> impl Strategy<Value = YearData> {
    (
        any::<i32>(),
        cell(),
        cell(),
        cell(),
        cell(),
        proptest::option::of(cell()),
        proptest::option::of(cell()),
        proptest::option::of(cell()),
    )
        .prop_map(|(year, sales, eps, high, low, div, ptp, bv)| YearData {
            year,
            sales,
            eps,
            high_price: high,
            low_price: low,
            dividend_per_share: div,
            pre_tax_profit: ptp,
            book_value_per_share: bv,
        })
}

fn judgment() -> impl Strategy<Value = Judgment> {
    (
        proptest::option::of(money()),
        proptest::option::of(money()),
        proptest::option::of(money()),
        proptest::option::of(money()),
        forecast_low_option(),
        proptest::option::of(money()),
    )
        .prop_map(|(hi, lo, php, plp, opt, cur)| Judgment {
            estimated_high_eps: hi,
            estimated_low_eps: lo,
            judged_avg_high_pe: php,
            judged_avg_low_pe: plp,
            forecast_low_option: opt,
            current_price: cur,
        })
}

fn study() -> impl Strategy<Value = Study> {
    (
        any::<u128>(),
        any::<u128>(),
        token(),
        token(),
        proptest::collection::vec(year_data(), 0..3),
        judgment(),
        proptest::option::of(token()),
        token(),
        any::<u32>(),
    )
        .prop_map(
            |(id, jid, ticker, cur, years, judgment, rationale, ts, sv)| Study {
                id: Uuid::from_u128(id),
                journal_id: Uuid::from_u128(jid),
                security_ticker: ticker,
                native_currency: cur,
                years,
                judgment,
                rationale,
                created_at: Timestamp(ts),
                schema_version: sv,
            },
        )
}

macro_rules! roundtrip {
    ($name:ident, $strat:expr, $ty:ty) => {
        proptest! {
            #[test]
            fn $name(x in $strat) {
                let json = serde_json::to_string(&x).unwrap();
                let back: $ty = serde_json::from_str(&json).unwrap();
                prop_assert_eq!(back, x);
            }
        }
    };
}

roundtrip!(money_round_trips, money(), Money);
roundtrip!(source_round_trips, source(), Source);
roundtrip!(freshness_round_trips, freshness(), Freshness);
roundtrip!(review_round_trips, review(), Review);
roundtrip!(coverage_round_trips, coverage(), Coverage);
roundtrip!(provenance_round_trips, provenance(), Provenance);
roundtrip!(cell_round_trips, cell(), Cell);
roundtrip!(year_data_round_trips, year_data(), YearData);
roundtrip!(judgment_round_trips, judgment(), Judgment);
roundtrip!(study_round_trips, study(), Study);
