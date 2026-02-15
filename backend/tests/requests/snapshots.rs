use backend::app::App;
use backend::models::_entities::{analysis_snapshots, tickers, users};
use loco_rs::prelude::*;
use loco_rs::testing::prelude::request;
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
use serial_test::serial;

/// A minimal 1x1 red pixel PNG encoded as base64.
const TINY_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwADhQGAWjR9awAAAABJRU5ErkJggg==";

/// Ensure a user and ticker exist for FK constraints.
/// Returns the ticker_id for use in snapshot requests.
/// Note: Loco boot already seeds users from fixtures, so we query the
/// existing user rather than inserting (which would cause a duplicate key error).
async fn seed_user_and_ticker(ctx: &AppContext) -> i32 {
    // User id=1 is already seeded by Loco boot from fixtures/users.yaml
    let _user = users::Entity::find_by_id(1)
        .one(&ctx.db)
        .await
        .unwrap()
        .expect("User id=1 should exist from fixture seed");

    // Find existing AAPL ticker (seeded via tickers migration)
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
        // chart_image is skip_serializing — verify absence via endpoint
        let res = request.get(&format!("/api/v1/snapshots/{}/chart-image", created.id)).await;
        assert_eq!(res.status_code(), 404); // no chart_image supplied → 404
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
        // Summary MUST contain ticker_symbol and key metrics (Story 7.6)
        assert_eq!(all[0]["ticker_symbol"].as_str().unwrap(), "AAPL");
        assert!((all[0]["projected_sales_cagr"].as_f64().unwrap() - 10.5).abs() < 0.01);
        assert!((all[0]["projected_eps_cagr"].as_f64().unwrap() - 12.0).abs() < 0.01);
        assert!((all[0]["projected_high_pe"].as_f64().unwrap() - 25.0).abs() < 0.01);
        assert!((all[0]["projected_low_pe"].as_f64().unwrap() - 15.0).abs() < 0.01);

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

// -----------------------------------------------------------------------
// Chart Image — POST with chart_image stores the decoded bytes
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn can_create_snapshot_with_chart_image() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": true,
            "chart_image": TINY_PNG_BASE64,
        });

        let res = request.post("/api/v1/snapshots").json(&body).await;
        res.assert_status_success();

        // chart_image is skip_serializing on Model — verify via /chart-image endpoint
        let created = res.json::<analysis_snapshots::Model>();
        let res = request.get(&format!("/api/v1/snapshots/{}/chart-image", created.id)).await;
        res.assert_status_success();
        assert_eq!(res.header("content-type"), "image/png");
    })
    .await;
}

// -----------------------------------------------------------------------
// Chart Image — POST without chart_image keeps NULL (AC #2)
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn can_create_snapshot_without_chart_image() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
            "chart_image": null,
        });

        let res = request.post("/api/v1/snapshots").json(&body).await;
        res.assert_status_success();

        // Verify chart-image endpoint returns 404 (no image stored)
        let created = res.json::<analysis_snapshots::Model>();
        let res = request.get(&format!("/api/v1/snapshots/{}/chart-image", created.id)).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

// -----------------------------------------------------------------------
// Chart Image — GET /chart-image returns raw PNG with correct content-type
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn can_retrieve_chart_image() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": true,
            "chart_image": TINY_PNG_BASE64,
        });
        let res = request.post("/api/v1/snapshots").json(&body).await;
        let created = res.json::<analysis_snapshots::Model>();

        let res = request.get(&format!("/api/v1/snapshots/{}/chart-image", created.id)).await;
        res.assert_status_success();
        // Verify content-type header
        assert_eq!(res.header("content-type"), "image/png");
    })
    .await;
}

// -----------------------------------------------------------------------
// Chart Image — GET /chart-image returns 404 when no image stored
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn returns_404_for_missing_chart_image() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
        });
        let res = request.post("/api/v1/snapshots").json(&body).await;
        let created = res.json::<analysis_snapshots::Model>();

        let res = request.get(&format!("/api/v1/snapshots/{}/chart-image", created.id)).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

// -----------------------------------------------------------------------
// Chart Image — POST with oversized chart_image is rejected
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn rejects_oversized_chart_image() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        // Create a base64 string larger than 5 MB.
        // Note: Axum's default JSON body limit (~2MB) may reject the payload
        // with 413 before our handler's size check returns 400.
        // Both responses correctly protect the server from oversized images.
        let oversized = "A".repeat(5 * 1024 * 1024 + 1);
        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
            "chart_image": oversized,
        });

        let res = request.post("/api/v1/snapshots").json(&body).await;
        let status = res.status_code();
        assert!(
            status == 400 || status == 413,
            "Expected 400 or 413, got {status}"
        );
    })
    .await;
}

// -----------------------------------------------------------------------
// Ticker Resolution — POST with ticker symbol resolves to ticker_id
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn can_create_snapshot_with_ticker_symbol() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        // Send ticker symbol instead of ticker_id — backend resolves it
        let body = serde_json::json!({
            "ticker": "AAPL",
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
            "notes": "Created via ticker symbol"
        });

        let res = request.post("/api/v1/snapshots").json(&body).await;
        res.assert_status_success();

        let created = res.json::<analysis_snapshots::Model>();
        assert!(created.id > 0);
        assert_eq!(created.ticker_id, ticker_id);
        assert_eq!(created.notes.as_deref(), Some("Created via ticker symbol"));
    })
    .await;
}

// -----------------------------------------------------------------------
// Ticker Resolution — POST without ticker_id or ticker returns 400
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn rejects_snapshot_without_ticker_id_or_symbol() {
    request::<App, _, _>(|request, ctx| async move {
        let _ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
        });

        let res = request.post("/api/v1/snapshots").json(&body).await;
        assert_eq!(res.status_code(), 400);
    })
    .await;
}
