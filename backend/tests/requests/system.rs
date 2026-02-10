use backend::app::App;
use loco_rs::testing::prelude::*;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn can_get_system_health() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/api/v1/system/health").await;
        assert_eq!(res.status_code(), 200);
        
        let body: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
        assert!(body.is_array());
        let providers = body.as_array().unwrap();
        assert_eq!(providers.len(), 3);
        assert_eq!(providers[0]["name"], "CH (SWX)");
        assert_eq!(providers[0]["status"], "Online");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_export_audit_logs_csv() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/api/v1/system/audit-logs/export").await;
        assert_eq!(res.status_code(), 200);
        
        // Correct header access
        let headers = res.headers();
        let ct = headers.get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or_default();
        assert_eq!(ct, "text/csv");
        let cd = headers.get("Content-Disposition").and_then(|v| v.to_str().ok()).unwrap_or_default();
        assert!(cd.contains("audit_logs.csv"));
        
        let body = res.text();
        assert!(body.contains("ID,Ticker,Exchange,Field,Old,New,Event,Source,Timestamp"));
    })
    .await;
}
