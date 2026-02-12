use backend::app::App;
use backend::models::_entities::{analysis_snapshots, tickers, users};
use loco_rs::prelude::*;
use loco_rs::testing::prelude::request;
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
use serial_test::serial;

/// Seed a user and a ticker for FK constraints.
/// Returns the ticker_id for use in snapshot requests.
async fn seed_user_and_ticker(ctx: &AppContext) -> i32 {
    let _user = users::ActiveModel {
        id: ActiveValue::set(1),
        pid: ActiveValue::set(uuid::Uuid::new_v4()),
        email: ActiveValue::set("test@example.com".to_string()),
        password: ActiveValue::set("hashed".to_string()),
        api_key: ActiveValue::set("lo-test-key".to_string()),
        name: ActiveValue::set("Test User".to_string()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();

    // Find existing AAPL ticker (seeded via historicals fixture)
    let ticker: tickers::Model = tickers::Entity::find()
        .filter(tickers::Column::Ticker.eq("AAPL"))
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap();

    ticker.id
}

fn sample_snapshot_data() -> serde_json::Value {
    serde_json::json!({
        "historical_data": { "ticker": "AAPL", "currency": "USD", "records": [], "is_complete": false, "is_split_adjusted": false },
        "projected_sales_cagr": 10.5,
        "projected_eps_cagr": 12.0,
        "projected_high_pe": 25.0,
        "projected_low_pe": 15.0,
        "analyst_note": "",
        "captured_at": "2026-01-01T00:00:00Z"
    })
}

// -----------------------------------------------------------------------
// AC #1 — POST /api/v1/snapshots creates a new snapshot
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn can_create_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
            "notes": "Initial draft analysis"
        });

        let res = request.post("/api/v1/snapshots").json(&body).await;
        res.assert_status_success();

        let created = res.json::<analysis_snapshots::Model>();
        assert!(created.id > 0);
        assert_eq!(created.ticker_id, ticker_id);
        assert!(!created.thesis_locked);
        assert_eq!(created.notes.as_deref(), Some("Initial draft analysis"));
        assert!(created.deleted_at.is_none());
    })
    .await;
}

// -----------------------------------------------------------------------
// AC #1 — POST with invalid ticker_id returns error
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn cannot_create_snapshot_with_invalid_ticker() {
    request::<App, _, _>(|request, ctx| async move {
        let _ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": 99999,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
        });

        let res = request.post("/api/v1/snapshots").json(&body).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

// -----------------------------------------------------------------------
// AC #2 — GET /api/v1/snapshots returns summaries with filters
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn can_list_snapshots_with_filters() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        // Create one unlocked and one locked snapshot
        let unlocked = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
            "notes": "Draft"
        });
        let locked = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": true,
            "notes": "Locked thesis"
        });
        request.post("/api/v1/snapshots").json(&unlocked).await.assert_status_success();
        request.post("/api/v1/snapshots").json(&locked).await.assert_status_success();

        // List all — should get 2
        let res = request.get("/api/v1/snapshots").await;
        res.assert_status_success();
        let all: Vec<serde_json::Value> = res.json();
        assert_eq!(all.len(), 2);
        // Summary should NOT contain snapshot_data
        assert!(all[0].get("snapshot_data").is_none());

        // Filter by thesis_locked=true — should get 1
        let res = request.get(&format!("/api/v1/snapshots?thesis_locked=true")).await;
        res.assert_status_success();
        let locked_only: Vec<serde_json::Value> = res.json();
        assert_eq!(locked_only.len(), 1);
        assert_eq!(locked_only[0]["notes"], "Locked thesis");

        // Filter by ticker_id
        let res = request.get(&format!("/api/v1/snapshots?ticker_id={}", ticker_id)).await;
        res.assert_status_success();
        let by_ticker: Vec<serde_json::Value> = res.json();
        assert_eq!(by_ticker.len(), 2);
    })
    .await;
}

// -----------------------------------------------------------------------
// AC #3 — GET /api/v1/snapshots/:id returns full snapshot_data
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn can_get_full_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
            "notes": "Full data test"
        });
        let res = request.post("/api/v1/snapshots").json(&body).await;
        res.assert_status_success();
        let created = res.json::<analysis_snapshots::Model>();

        let res = request.get(&format!("/api/v1/snapshots/{}", created.id)).await;
        res.assert_status_success();
        let full = res.json::<analysis_snapshots::Model>();
        assert_eq!(full.id, created.id);
        // Full response includes snapshot_data
        assert_eq!(full.snapshot_data["projected_sales_cagr"], 10.5);
    })
    .await;
}

// -----------------------------------------------------------------------
// AC #4 — DELETE soft-deletes unlocked snapshots
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn can_soft_delete_unlocked_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
        });
        let res = request.post("/api/v1/snapshots").json(&body).await;
        let created = res.json::<analysis_snapshots::Model>();

        // DELETE should succeed
        let res = request.delete(&format!("/api/v1/snapshots/{}", created.id)).await;
        res.assert_status_success();
        let del: serde_json::Value = res.json();
        assert_eq!(del["status"], "deleted");

        // Verify: deleted snapshot not in list
        let res = request.get("/api/v1/snapshots").await;
        let all: Vec<serde_json::Value> = res.json();
        assert_eq!(all.len(), 0);

        // Verify: deleted snapshot returns 404 on get
        let res = request.get(&format!("/api/v1/snapshots/{}", created.id)).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

// -----------------------------------------------------------------------
// AC #4 — DELETE on locked snapshot returns 403
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn cannot_delete_locked_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": true,
            "notes": "Locked"
        });
        let res = request.post("/api/v1/snapshots").json(&body).await;
        let created = res.json::<analysis_snapshots::Model>();

        let res = request.delete(&format!("/api/v1/snapshots/{}", created.id)).await;
        assert_eq!(res.status_code(), 403);
        let body: serde_json::Value = res.json();
        assert_eq!(body["error"], "Locked analyses cannot be deleted");
    })
    .await;
}

// -----------------------------------------------------------------------
// AC #5 — PUT on locked snapshot returns 403
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn cannot_modify_locked_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": true,
        });
        let res = request.post("/api/v1/snapshots").json(&body).await;
        let created = res.json::<analysis_snapshots::Model>();

        let res = request.put(&format!("/api/v1/snapshots/{}", created.id)).json(&serde_json::json!({})).await;
        assert_eq!(res.status_code(), 403);
        let body: serde_json::Value = res.json();
        assert_eq!(body["error"], "Locked analyses cannot be modified");
    })
    .await;
}

// -----------------------------------------------------------------------
// AC #5 — PUT on unlocked snapshot also returns 403 (append-only)
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn cannot_modify_unlocked_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
        });
        let res = request.post("/api/v1/snapshots").json(&body).await;
        let created = res.json::<analysis_snapshots::Model>();

        let res = request.put(&format!("/api/v1/snapshots/{}", created.id)).json(&serde_json::json!({})).await;
        assert_eq!(res.status_code(), 403);
        let body: serde_json::Value = res.json();
        assert_eq!(body["error"], "Snapshots are append-only and cannot be modified. Create a new snapshot instead.");
    })
    .await;
}

// -----------------------------------------------------------------------
// Edge case — GET on non-existent ID returns 404
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn returns_404_for_nonexistent_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let _ticker_id = seed_user_and_ticker(&ctx).await;

        let res = request.get("/api/v1/snapshots/99999").await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

// -----------------------------------------------------------------------
// Edge case — GET on soft-deleted snapshot returns 404
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn returns_404_for_soft_deleted_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
        });
        let res = request.post("/api/v1/snapshots").json(&body).await;
        let created = res.json::<analysis_snapshots::Model>();

        // Soft-delete
        request.delete(&format!("/api/v1/snapshots/{}", created.id)).await;

        // GET should return 404
        let res = request.get(&format!("/api/v1/snapshots/{}", created.id)).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
