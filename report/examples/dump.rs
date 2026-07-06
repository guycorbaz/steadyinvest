//! Throwaway visual-check harness (issue #105): render a representative study to a PDF on disk so it
//! can be eyeballed / converted to an image. Not part of the build's tests.
use steadyinvest_contract::{
    Cell, Coverage, ForecastLowOption, Freshness, Judgment, Money, Provenance, Review, Source,
    Study, Timestamp, YearData,
};
use uuid::Uuid;

fn money(s: &str) -> Money {
    Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
}

fn cell(v: &str) -> Cell {
    Cell {
        value: Some(money(v)),
        source: Source::Manual,
        freshness: Freshness::Current,
        review: Review::Validated,
        coverage: Coverage::Present,
        provenance: Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: Timestamp("2026-03-09T00:00:00Z".to_string()),
            hash_of_dependencies: "manual".to_string(),
        },
        pending: None,
    }
}

fn year(y: i32, sales: &str, eps: &str, hi: &str, lo: &str) -> YearData {
    YearData {
        year: y,
        sales: cell(sales),
        eps: cell(eps),
        high_price: cell(hi),
        low_price: cell(lo),
        dividend_per_share: Some(cell("2")),
        pre_tax_profit: Some(cell("200")),
        book_value_per_share: Some(cell("40")),
    }
}

fn main() {
    let judgment = Judgment {
        estimated_high_eps: Some(money("9")),
        estimated_low_eps: Some(money("5")),
        projected_sales_growth_pct: None,
        projected_eps_growth_pct: None,
        judged_avg_high_pe: Some(money("18")),
        judged_avg_low_pe: Some(money("10")),
        forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
        recent_severe_low: None,
        current_price: Some(money("92")),
        present_full_year_dividend: Some(money("2")),
        ttm_eps: None,
    };
    let mut s = Study::new(
        Uuid::from_u128(0x56),
        Uuid::from_u128(0x1),
        "NESN",
        "CHF",
        judgment,
        Timestamp("2026-03-09T09:30:00Z".to_string()),
    );
    // AAPL-scale magnitudes: sales in hundreds of BILLIONS, EPS single-digit, price in hundreds — the
    // case a single shared log scale flattens (issue: per-series scales must keep each line readable).
    s.years = vec![
        year(2016, "215639000000", "2.07", "118.69", "89.47"),
        year(2017, "229234000000", "2.30", "177.20", "114.76"),
        year(2018, "265595000000", "2.97", "233.47", "146.59"),
        year(2019, "260174000000", "2.98", "293.97", "142.00"),
        year(2020, "274515000000", "3.27", "515.14", "103.10"),
        year(2021, "365817000000", "5.62", "182.13", "116.21"),
        year(2022, "394328000000", "6.11", "182.94", "125.87"),
        year(2023, "383285000000", "6.12", "199.62", "124.17"),
        year(2024, "391035000000", "6.08", "260.10", "164.08"),
        year(2025, "416161000000", "7.47", "288.62", "169.21"),
    ];
    let bytes = steadyinvest_report::render_study_pdf(&s).expect("renders");
    let path = std::env::args().nth(1).unwrap_or_else(|| "/tmp/study.pdf".into());
    std::fs::write(&path, &bytes).expect("write");
    eprintln!("wrote {} ({} bytes)", path, bytes.len());
}
