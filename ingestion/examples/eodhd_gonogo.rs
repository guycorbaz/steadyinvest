//! Manual GO/NO-GO for the EODHD adapter (Story 3.1) — the live-network check CI cannot run.
//!
//! The pure mapping ([`map_eodhd`]) is unit-tested; this exercises the thin HTTP layer against the
//! real API and confirms a live response maps faithfully. The key is read from the environment so
//! it never lands in source, history, or a process arg list.
//!
//! Run:
//!     EODHD_API_KEY=your_key cargo run -p steadyinvest-ingestion --example eodhd_gonogo -- AAPL.US
//!
//! `AAPL.US` works with EODHD's `demo` key too, so a smoke test needs no paid key:
//!     EODHD_API_KEY=demo cargo run -p steadyinvest-ingestion --example eodhd_gonogo -- AAPL.US

use steadyinvest_ingestion::adapters::eodhd::EodhdProvider;
use steadyinvest_ingestion::{Provider, fetch_canonical, install_crypto_provider};

#[tokio::main]
async fn main() {
    install_crypto_provider();

    let ticker = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "AAPL.US".to_string());
    let api_key = match std::env::var("EODHD_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("NO-GO: set EODHD_API_KEY in the environment (use `demo` for AAPL.US).");
            std::process::exit(2);
        }
    };

    println!("→ Fetching {ticker} from EODHD (live)…\n");

    let provider = Provider::Eodhd(EodhdProvider::new());
    match fetch_canonical(&provider, &ticker, Some(&api_key)).await {
        Ok(fetched) => {
            let c = &fetched.canonical;
            println!("GO ✓  {ticker}");
            println!("   years returned  : {}", c.years.len());
            println!("   usable years    : {}", c.usable_years);
            println!("   findings        : {}", c.findings.len());
            println!("   dependency hash : {}", fetched.digest);
            println!(
                "\n   {:<6} {:>14} {:>10} {:>10} {:>10} {:>10}",
                "year", "sales", "eps", "high", "low", "bvps"
            );
            for y in &c.years {
                let f = |o: Option<rust_decimal::Decimal>| {
                    o.map(|d| d.normalize().to_string())
                        .unwrap_or_else(|| "—".into())
                };
                println!(
                    "   {:<6} {:>14} {:>10} {:>10} {:>10} {:>10}",
                    y.year,
                    f(y.sales),
                    f(y.eps),
                    f(y.high_price),
                    f(y.low_price),
                    f(y.book_value_per_share),
                );
            }
            println!(
                "\nReview the rows above against a known source. If they look right, this is a GO."
            );
        }
        Err(e) => {
            eprintln!("NO-GO ✗  {ticker}: {e}");
            std::process::exit(1);
        }
    }
}
