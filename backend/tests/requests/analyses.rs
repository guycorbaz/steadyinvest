use backend::app::App;
use loco_rs::testing::prelude::*;
use backend::models::_entities::{locked_analyses, tickers};
use naic_logic::{AnalysisSnapshot, HistoricalData};
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

#[tokio::test]
async fn can_lock_and_get_analyses() {
    request::<App, _, _>(|request, ctx| async move {
        // 1. Prepare Ticker
        let _ticker: tickers::Model = tickers::Entity::find()
            .filter(tickers::Column::Ticker.eq("AAPL"))
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();

        let snapshot = AnalysisSnapshot {
            historical_data: HistoricalData::default(),
            projected_sales_cagr: 10.5,
            projected_eps_cagr: 12.0,
            projected_high_pe: 25.0,
            projected_low_pe: 15.0,
            analyst_note: String::new(),
            captured_at: chrono::Utc::now(),
        };

        let req = serde_json::json!({
            "ticker": "AAPL",
            "snapshot": snapshot,
            "analyst_note": "Bullish on hardware sales"
        });

        // 2. Test Lock
        let response = request.post("/api/analyses/lock").json(&req).await;
        response.assert_status_success();

        // 3. Test Get
        let response = request.get("/api/analyses/AAPL").await;
        response.assert_status_success();
        
        let list = response.json::<Vec<locked_analyses::Model>>();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].analyst_note, "Bullish on hardware sales");
        let analysis_id = list[0].id;

        // 4. Test Export
        let response = request.get(&format!("/api/analyses/export/{}", analysis_id)).await;
        response.assert_status_success();
        assert_eq!(response.headers().get("content-type").unwrap().to_str().unwrap(), "application/pdf");
        
        let body = response.as_bytes();
        assert!(body.starts_with(b"%PDF-"));
    })
    .await;
}
