//! Dev aid (not shipped): render a fabricated comparison to a path so the landscape layout can
//! be eyeballed. `cargo run -p steadyinvest-report --example render_comparison_demo -- out.pdf`
use steadyinvest_report::{Comparison, ComparisonColumn, render_comparison};

fn column(ticker: &str, name: &str, currency: &str, unavailable: bool) -> ComparisonColumn {
    let rows: Vec<String> = [
        "46,6 %", "12,0 %", "58,7 %", "15,0 %", "47,6 % · ↑ hausse", "62,5 % · ↑ hausse", "",
        "31,25", "140,70 – 2 857,36", "177,20", "58,1", "39,7", "31,2", "24,3", "18,9", "28,9",
        "140,70 – 1 046,25", "1 046,25 – 1 951,81", "1 951,81 – 2 857,36", "", "31,0:1",
        "0,0 %", "66,4 %", "", "", "2,3 %",
        "3 : PER haut jugé au-dessus de la moyenne · PER bas jugé au-dessus de la moyenne · ratio hausse / baisse sous la cible",
        "", "2026-09-24", "US",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    ComparisonColumn {
        ticker: ticker.into(),
        name: name.into(),
        currency: currency.into(),
        date: "2026-09-24".into(),
        unavailable,
        rows,
        zone: "buy".into(),
        state: "provisional".into(),
        low_confidence: ticker.starts_with('N'),
        ptp_avg_years: 5,
        roe_avg_years: 5,
        ..Default::default()
    }
}

fn main() {
    let c = Comparison {
        date: "2026-09-24".into(),
        currency_mix: true,
        columns: vec![
            column("NVDA.US", "NVIDIA Corporation", "USD", false),
            column("NESN.SW", "Nestlé", "CHF", false),
            column("SCHN.SW", "Schindler Holding", "CHF", false),
            column("ROG.SW", "Roche", "CHF", true),
            column("ABBN.SW", "ABB", "CHF", false),
        ],
    };
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/comparison-demo.pdf".to_string());
    std::fs::write(&out, render_comparison(&c)).unwrap();
    println!("wrote {out}");
}
