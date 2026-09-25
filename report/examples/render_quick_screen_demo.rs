//! Dev aid (not shipped): render a fabricated examination to a path so the layout can be eyeballed.
//! `cargo run -p steadyinvest-report --example render_quick_screen_demo -- out.pdf`
use steadyinvest_report::{
    QuickScreen, QuickScreenLadder, QuickScreenPriceRow, render_quick_screen,
};

fn main() {
    let ladder = |lines: [&str; 10], rate: &str| QuickScreenLadder {
        lines: lines.iter().map(|s| s.to_string()).collect(),
        years: vec!["2025".into(), "2024".into(), "2020".into(), "2019".into()],
        rate: rate.into(),
        span_years: "5".into(),
        nonpositive_base: false,
        unavailable: false,
    };
    let q = QuickScreen {
        ticker: "NESN.SW".into(),
        name: "Nestlé".into(),
        currency: "CHF".into(),
        date: "2026-09-24".into(),
        source: "depuis l'étude".into(),
        sales: ladder(
            [
                "89 490", "91 720", "181 210", "90 605", "84 681", "92 865", "177 546", "88 773",
                "1 832", "2,1 %",
            ],
            "0,4 %",
        ),
        eps: ladder(
            [
                "3,56",
                "4,13",
                "7,69",
                "3,85",
                "4,29",
                "8,84",
                "13,13",
                "6,57",
                "−2,72",
                "−41,5 %",
            ],
            "−10,2 %",
        ),
        eps_vs_sales: "sales".into(),
        reasons: "Cession d'activités, marge sous pression.".into(),
        factors_continue: "less".into(),
        price_rows: [
            ("2021", "128,90", "95,00", "8,20", "15,7", "11,6"),
            ("2022", "131,68", "104,26", "3,42", "38,5", "30,5"),
            ("2023", "116,50", "95,60", "4,24", "27,5", "22,5"),
            ("2024", "99,60", "72,50", "4,13", "24,1", "17,6"),
            ("2025", "90,20", "69,90", "3,56", "25,3", "19,6"),
        ]
        .iter()
        .map(|(y, h, l, e, ph, pl)| QuickScreenPriceRow {
            year: y.to_string(),
            high: h.to_string(),
            low: l.to_string(),
            eps: e.to_string(),
            pe_high: ph.to_string(),
            pe_low: pl.to_string(),
        })
        .collect(),
        pe_high_total: "131,1".into(),
        pe_low_total: "101,8".into(),
        pe_high_avg: "26,2".into(),
        pe_low_avg: "20,4".into(),
        pe_avg_of_avgs: "23,3".into(),
        pe_basis: "five".into(),
        pe_years: "5".into(),
        record_years: "5".into(),
        present_price: "77,10".into(),
        present_eps: "2,90".into(),
        present_pe: "26,6".into(),
        high_five_years_ago: "128,90".into(),
        high_basis: "five".into(),
        high_year: "2021".into(),
        price_vs_high_pct: "−40,2 %".into(),
        price_vs_high: "lower".into(),
        years_sold_as_high: "5".into(),
        sold_basis: "five".into(),
        sold_of: "5".into(),
        pe_position: "higher".into(),
        pe_absent: String::new(),
        pe_notes: String::new(),
        objective: "7 %".into(),
        sales_meets: "no".into(),
        eps_meets: "no".into(),
        eps_will_meet: "no".into(),
    };
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/quick-screen-demo.pdf".to_string());
    std::fs::write(&out, render_quick_screen(&q)).unwrap();
    println!("wrote {out}");
}
