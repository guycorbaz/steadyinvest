//! Throwaway seed for the issue #100 progress/cancel live check.
//!
//! Builds a journal with ~18 USD studies + one matching holding each, so a manual price refresh
//! enqueues ~18 paced jobs. On EODHD (200 ms/req + the real HTTP round-trip) the batch runs for
//! several seconds — long enough to watch the `n / t` counter climb and to catch the "Annuler"
//! button. Tickers are in EODHD `.US` form so the /eod close actually resolves.
//!
//! Run:  cargo run -p steadyinvest-persistence --example seed_bigbatch -- /tmp/bigbatch.db
//! Then open that .db in the app (Réglages → open journal) and click "Rafraîchir les prix".

use steadyinvest_contract::provenance::Timestamp;
use steadyinvest_contract::study::{ForecastLowOption, Judgment, Study};
use steadyinvest_persistence::Journal;
use uuid::Uuid;

fn empty_judgment() -> Judgment {
    Judgment {
        estimated_high_eps: None,
        estimated_low_eps: None,
        projected_sales_growth_pct: None,
        projected_eps_growth_pct: None,
        judged_avg_high_pe: None,
        judged_avg_low_pe: None,
        forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
        recent_severe_low: None,
        current_price: None,
        present_full_year_dividend: None,
        ttm_eps: None,
    }
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: seed_bigbatch <path-to-new.db>");
        std::process::exit(2);
    });

    // Distinct EODHD `.US` tickers → distinct linked studies → one paced job each.
    let tickers = [
        "AAPL.US", "MSFT.US", "GOOGL.US", "AMZN.US", "META.US", "NVDA.US", "TSLA.US", "JPM.US",
        "V.US", "JNJ.US", "WMT.US", "PG.US", "HD.US", "KO.US", "PEP.US", "DIS.US", "CSCO.US",
        "INTC.US",
    ];

    let created_at = Timestamp("2026-07-07T00:00:00Z".to_string());
    let journal_id = Uuid::from_u128(0x5100_0000_0000_0000_0000_0000_0000_0001);

    let mut journal = Journal::create(&path, journal_id, &created_at)
        .unwrap_or_else(|e| panic!("create journal at {path}: {e}"));

    let portfolio_id = Uuid::from_u128(0x5100_0000_0000_0000_0000_0000_0000_0002);
    journal
        .ensure_portfolio(portfolio_id, "Big batch test", &created_at)
        .expect("ensure portfolio");

    for (i, ticker) in tickers.iter().enumerate() {
        let n = (i as u128) + 1;
        let study = Study::new(
            Uuid::from_u128(0x5100_0000_0000_0000_0000_0000_0001_0000 + n),
            journal_id,
            *ticker,
            "USD",
            empty_judgment(),
            created_at.clone(),
        );
        journal.put_study(&study).expect("put study");

        journal
            .add_holding(
                Uuid::from_u128(0x5100_0000_0000_0000_0000_0000_0002_0000 + n),
                portfolio_id,
                ticker,
                "10",
                "100.00",
                "USD",
                None,
                &created_at,
            )
            .expect("add holding");
    }

    println!(
        "seeded {} linked USD holdings into {path}\nopen it in the app and click \"Rafraîchir les prix\"",
        tickers.len()
    );
}
