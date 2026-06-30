//! Preview tool (Story 5.6): render a representative demo study to a PDF so the faithful, neutral,
//! greyscale layout can be eyeballed. `cargo run -p steadyinvest-report --example study_pdf [PATH]`
//! (defaults to `study-demo.pdf` in the current directory).

use steadyinvest_contract::{
    Cell, Coverage, ForecastLowOption, Freshness, Judgment, Money, Provenance, Review, Source,
    Study, Timestamp, YearData,
};
use uuid::Uuid;

fn money(s: &str) -> Money {
    Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
}

fn cell(value: &str) -> Cell {
    Cell {
        value: Some(money(value)),
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

fn main() {
    let judgment = Judgment {
        estimated_high_eps: Some(money("9")),
        estimated_low_eps: Some(money("4")),
        projected_sales_growth_pct: None,
        projected_eps_growth_pct: None,
        judged_avg_high_pe: Some(money("18")),
        judged_avg_low_pe: Some(money("10")),
        forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
        recent_severe_low: None,
        current_price: Some(money("80")),
        present_full_year_dividend: Some(money("2")),
    };
    let mut study = Study::new(
        Uuid::from_u128(0x56),
        Uuid::from_u128(0x1),
        "NESN",
        "CHF",
        judgment,
        Timestamp("2026-03-09T09:30:00Z".to_string()),
    );
    // A varied 8-year series so the §1/§2/§3 tables and growth figures show real numbers.
    let rows = [
        (2018, "820", "3.6", "78", "55", "150", "32"),
        (2019, "860", "3.9", "86", "60", "165", "34"),
        (2020, "910", "4.2", "94", "58", "175", "35"),
        (2021, "1000", "4.8", "104", "70", "200", "38"),
        (2022, "1080", "5.3", "118", "88", "215", "40"),
        (2023, "1130", "5.7", "126", "95", "228", "42"),
        (2024, "1190", "6.1", "134", "101", "240", "44"),
        (2025, "1255", "6.6", "142", "108", "255", "46"),
    ];
    study.years = rows
        .iter()
        .map(|(y, sales, eps, hi, lo, ptp, bv)| YearData {
            year: *y,
            sales: cell(sales),
            eps: cell(eps),
            high_price: cell(hi),
            low_price: cell(lo),
            dividend_per_share: Some(cell("2")),
            pre_tax_profit: Some(cell(ptp)),
            book_value_per_share: Some(cell(bv)),
        })
        .collect();

    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "study-demo.pdf".to_string());
    let bytes = steadyinvest_report::render_study_pdf(&study).expect("the demo study renders");
    std::fs::write(&path, &bytes).expect("write the PDF");
    println!("wrote {} ({} bytes)", path, bytes.len());
}
