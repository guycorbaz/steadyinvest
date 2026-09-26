//! Dev aid (not shipped): render a realistic study to /tmp so the layout can be eyeballed.
use rust_decimal::Decimal;
use steadyinvest_contract::{
    Cell, Coverage, ForecastLowOption, Freshness, Judgment, Money, Provenance, Review, Source,
    Study, Timestamp, YearData,
};
use uuid::Uuid;

fn money_of(s: &str) -> Money {
    Money::from(Decimal::from_str_exact(s).unwrap())
}
fn cell(value: &str) -> Cell {
    Cell {
        value: Some(money_of(value)),
        source: Source::Provider,
        freshness: Freshness::Current,
        review: Review::Validated,
        coverage: Coverage::Present,
        provenance: Provenance {
            source: Source::Provider,
            logical_version: 1,
            timestamp: Timestamp("2026-09-23T00:00:00Z".to_string()),
            hash_of_dependencies: "eodhd:abc".to_string(),
        },
        pending: None,
    }
}
fn main() {
    let judgment = Judgment {
        estimated_high_eps: Some(money_of("6.2")),
        estimated_low_eps: Some(money_of("3.4")),
        projected_sales_growth_pct: Some(money_of("4")),
        projected_eps_growth_pct: Some(money_of("6")),
        judged_avg_high_pe: Some(money_of("24")),
        judged_avg_low_pe: Some(money_of("18.7")),
        forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
        recent_severe_low: Some(money_of("69.9")),
        current_price: Some(money_of("77.1")),
        present_full_year_dividend: None,
        ttm_eps: Some(money_of("2.9")),
    };
    let mut s = Study::new(
        Uuid::from_u128(0x5_6),
        Uuid::from_u128(0x1),
        "NESN.SW",
        "CHF",
        judgment,
        Timestamp("2026-09-23T09:30:00Z".to_string()),
    );
    s.company_name = Some("Nestlé".to_string());
    // (year, [sales, pre-tax profit, EPS, high, low, dividend/share, book value/share])
    let data: [(i32, [&str; 7]); 10] = [
        (
            2016,
            ["89786", "13296", "2.76", "80.05", "67", "2.2399", "20.86"],
        ),
        (
            2017,
            ["89922", "10284", "2.04", "86.4", "71.45", "2.3002", "19.68"],
        ),
        (
            2018,
            ["91750", "13907", "3.83", "86.5", "72.92", "2.3597", "19.00"],
        ),
        (
            2019,
            [
                "92865", "16063", "8.84", "113.2", "79.86", "2.4642", "17.74",
            ],
        ),
        (
            2020,
            [
                "84681", "15737", "4.29", "112.62", "83.37", "2.7027", "16.04",
            ],
        ),
        (
            2021,
            ["87470", "19457", "8.2", "128.9", "95", "2.7521", "19.04"],
        ),
        (
            2022,
            ["94780", "12326", "3.42", "131.68", "104.26", "2.8", "15.5"],
        ),
        (
            2023,
            ["93351", "13823", "4.24", "116.5", "95.6", "2.95", "13.5"],
        ),
        (
            2024,
            ["91720", "14488", "4.13", "99.6", "72.5", "3.0", "13.82"],
        ),
        (
            2025,
            ["89490", "11894", "3.56", "90.2", "69.9", "3.05", "12.74"],
        ),
    ];
    s.years = data
        .iter()
        .map(|(y, [sa, ptp, eps, hi, lo, dv, bv])| YearData {
            year: *y,
            sales: cell(sa),
            eps: cell(eps),
            high_price: cell(hi),
            low_price: cell(lo),
            dividend_per_share: Some(cell(dv)),
            pre_tax_profit: Some(cell(ptp)),
            book_value_per_share: Some(cell(bv)),
        })
        .collect();
    let bytes =
        steadyinvest_report::render_study_pdf(&s, steadyinvest_report::NumberStyle::default())
            .expect("renders");
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/demo.pdf".to_string());
    std::fs::write(&out, bytes).unwrap();
    println!("wrote {out}");
}
