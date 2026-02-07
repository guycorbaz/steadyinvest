use backend::app::App;
use loco_rs::testing::prelude::*;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn can_harvest_ticker() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.post("/api/harvest/AAPL").await;
        assert_eq!(res.status_code(), 200);

        let data: naic_logic::HistoricalData = res.json();
        assert_eq!(data.ticker, "AAPL");
        assert_eq!(data.records.len(), 10);
        assert!(data.is_complete);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn verify_split_adjustment_metadata() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.post("/api/harvest/AAPL").await;
        assert_eq!(res.status_code(), 200);

        let data: naic_logic::HistoricalData = res.json();
        assert_eq!(data.ticker, "AAPL");
        assert!(data.is_split_adjusted); // AAPL should be split-adjusted in mock
        for record in &data.records {
            if record.fiscal_year < 2020 {
                assert_eq!(record.adjustment_factor, rust_decimal::Decimal::from(4));
            }
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn verify_no_split_adjustment_for_standard_ticker() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.post("/api/harvest/MSFT").await;
        assert_eq!(res.status_code(), 200);

        let data: naic_logic::HistoricalData = res.json();
        assert_eq!(data.ticker, "MSFT");
        assert!(!data.is_split_adjusted); // MSFT should NOT be split-adjusted
        for record in &data.records {
            assert_eq!(record.adjustment_factor, rust_decimal::Decimal::ONE);
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn verify_currency_normalization_metadata() {
    request::<App, _, _>(|request, _ctx| async move {
        // NESN.SW should be in the system (seeded in migration) with currency CHF
        let res = request.post("/api/harvest/NESN.SW").await;
        assert_eq!(res.status_code(), 200);

        let data: naic_logic::HistoricalData = res.json();
        assert_eq!(data.ticker, "NESN.SW");
        assert_eq!(data.currency, "CHF");
        
        // Reporting is CHF, Display is USD (default in run_harvest)
        // Check that records have exchange rates
        for record in &data.records {
            assert!(record.exchange_rate.is_some(), "Exchange rate should be present for foreign ticker");
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn verify_normalization_math_consistency() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.post("/api/harvest/NESN.SW").await;
        assert_eq!(res.status_code(), 200);

        let mut data: naic_logic::HistoricalData = res.json();
        
        // Initially no display_currency is set in the response from run_harvest (None)
        // and records are NOT normalized yet in the response, but exchange rates are provided.
        let original_sales = data.records[0].sales;
        let rate = data.records[0].exchange_rate.expect("Exchange rate missing for foreign ticker");
        
        data.apply_normalization("USD");
        
        assert_eq!(data.display_currency, Some("USD".to_string()));
        let normalized_sales = data.records[0].sales;
        
        // Exactness check using rust_decimal
        assert!(normalized_sales != original_sales);
        assert_eq!(normalized_sales, (original_sales * rate).round_dp(2));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn cannot_harvest_empty_ticker() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.post("/api/harvest/").await;
        assert!(res.status_code() == 404 || res.status_code() == 405);
    })
    .await;
}
