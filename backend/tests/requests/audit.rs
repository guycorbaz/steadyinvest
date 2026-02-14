use backend::app::App;
use loco_rs::testing::prelude::*;
use serial_test::serial;
use sea_orm::{ActiveModelTrait, ActiveValue};
use backend::models::tickers;

#[tokio::test]
#[serial]
async fn can_list_audit_logs() {
    request::<App, _, _>(|request, ctx| async move {
        // Seed some data using AuditService
        backend::services::audit_service::AuditService::log_anomaly(
            &ctx.db,
            "AAPL",
            "NASDAQ",
            "Price",
            "Sudden 10% drop detected",
        ).await.unwrap();

        backend::services::audit_service::AuditService::log_override(
            &ctx.db,
            "MSFT",
            "NASDAQ",
            "MarketCap",
            Some("2.5T".to_string()),
            Some("2.6T".to_string()),
        ).await.unwrap();

        let res = request.get("/api/v1/system/audit-logs").await;
        assert_eq!(res.status_code(), 200);

        let body: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
        assert!(body.is_array());
        let logs = body.as_array().unwrap();
        
        // We expect at least the 2 seeded logs
        assert!(logs.len() >= 2);

        // Check both entries exist (order may vary with same-second timestamps)
        let has_msft = logs.iter().any(|l| l["ticker"] == "MSFT" && l["event_type"] == "Override");
        let has_aapl = logs.iter().any(|l| l["ticker"] == "AAPL" && l["event_type"] == "Anomaly");
        assert!(has_msft, "Expected MSFT override log");
        assert!(has_aapl, "Expected AAPL anomaly log");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn test_harvest_anomaly_logging() {
    request::<App, _, _>(|request, ctx| async move {
        // 1. Seed a ticker
        let ticker = "ANOMALY";
        let _ = tickers::ActiveModel {
            ticker: ActiveValue::set(ticker.to_string()),
            name: ActiveValue::set("Anomaly Test".to_string()),
            exchange: ActiveValue::set("NASDAQ".to_string()),
            currency: ActiveValue::set("USD".to_string()),
            ..Default::default()
        }.insert(&ctx.db).await.unwrap();

        // 2. Run harvest (This triggers the anomaly logging)
        let res = request.post(&format!("/api/harvest/{}", ticker)).await;
        assert_eq!(res.status_code(), 200);

        // 3. Verify audit log existence
        let res = request.get("/api/v1/system/audit-logs").await;
        assert_eq!(res.status_code(), 200);
        let body: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
        let logs = body.as_array().expect("Audit logs should be an array");
        
        let found = logs.iter().any(|l| 
            l["ticker"] == "ANOMALY" && 
            l["event_type"] == "Anomaly" &&
            l["field"] == "Integrity"
        );
        assert!(found, "Expected anomaly log for ANOMALY ticker not found");
    })
    .await;
}
