use backend::app::App;
use backend::models::_entities::{analysis_snapshots, tickers, users};
use loco_rs::prelude::*;
use loco_rs::testing::prelude::request;
use sea_orm::{ActiveValue, EntityTrait, QueryFilter, ColumnTrait};
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

/// Build snapshot_data with specific metric values for history delta testing.
fn snapshot_data_with_metrics(
    sales_cagr: Option<f64>,
    eps_cagr: Option<f64>,
    high_pe: Option<f64>,
    low_pe: Option<f64>,
) -> serde_json::Value {
    let mut data = serde_json::json!({
        "historical_data": { "ticker": "AAPL", "currency": "USD", "records": [], "is_complete": false, "is_split_adjusted": false },
        "analyst_note": "",
        "captured_at": "2026-01-01T00:00:00Z"
    });
    if let Some(v) = sales_cagr {
        data["projected_sales_cagr"] = serde_json::json!(v);
    }
    if let Some(v) = eps_cagr {
        data["projected_eps_cagr"] = serde_json::json!(v);
    }
    if let Some(v) = high_pe {
        data["projected_high_pe"] = serde_json::json!(v);
    }
    if let Some(v) = low_pe {
        data["projected_low_pe"] = serde_json::json!(v);
    }
    data
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

// =======================================================================
// History endpoint — GET /api/v1/snapshots/:id/history (Story 8.4)
// =======================================================================

// -----------------------------------------------------------------------
// 4.1 — Multiple snapshots returned in captured_at ASC order
//        Inserts with explicit captured_at in non-chronological order
//        to genuinely test ORDER BY captured_at ASC.
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn can_get_history_for_ticker_with_multiple_snapshots() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        // Insert 3 snapshots directly via SeaORM with explicit captured_at
        // timestamps in NON-chronological insertion order:
        //   Insert order: Sept (2nd), June (1st), Dec (3rd)
        //   Expected ASC: June, Sept, Dec
        let sept = analysis_snapshots::ActiveModel {
            user_id: ActiveValue::set(1),
            ticker_id: ActiveValue::set(ticker_id),
            snapshot_data: ActiveValue::set(
                snapshot_data_with_metrics(Some(8.0), Some(10.0), Some(22.0), Some(14.0)),
            ),
            thesis_locked: ActiveValue::set(true),
            notes: ActiveValue::set(Some("Sept review".to_string())),
            captured_at: ActiveValue::set(
                chrono::DateTime::parse_from_rfc3339("2025-09-15T10:00:00+00:00").unwrap(),
            ),
            deleted_at: ActiveValue::set(None),
            ..Default::default()
        };
        let sept_model = sept.insert(&ctx.db).await.unwrap();

        let june = analysis_snapshots::ActiveModel {
            user_id: ActiveValue::set(1),
            ticker_id: ActiveValue::set(ticker_id),
            snapshot_data: ActiveValue::set(
                snapshot_data_with_metrics(Some(6.0), Some(8.5), Some(25.0), Some(15.0)),
            ),
            thesis_locked: ActiveValue::set(true),
            notes: ActiveValue::set(Some("June review".to_string())),
            captured_at: ActiveValue::set(
                chrono::DateTime::parse_from_rfc3339("2025-06-15T10:00:00+00:00").unwrap(),
            ),
            deleted_at: ActiveValue::set(None),
            ..Default::default()
        };
        let june_model = june.insert(&ctx.db).await.unwrap();

        let dec = analysis_snapshots::ActiveModel {
            user_id: ActiveValue::set(1),
            ticker_id: ActiveValue::set(ticker_id),
            snapshot_data: ActiveValue::set(
                snapshot_data_with_metrics(Some(4.5), Some(6.0), Some(20.0), Some(12.0)),
            ),
            thesis_locked: ActiveValue::set(false),
            notes: ActiveValue::set(Some("Dec review".to_string())),
            captured_at: ActiveValue::set(
                chrono::DateTime::parse_from_rfc3339("2025-12-15T10:00:00+00:00").unwrap(),
            ),
            deleted_at: ActiveValue::set(None),
            ..Default::default()
        };
        let dec_model = dec.insert(&ctx.db).await.unwrap();

        // Request history using the middle snapshot (sept) as anchor
        let res = request
            .get(&format!("/api/v1/snapshots/{}/history", sept_model.id))
            .await;
        res.assert_status_success();
        let history: serde_json::Value = res.json();

        // Verify ticker info
        assert_eq!(history["ticker_id"], ticker_id);
        assert_eq!(history["ticker_symbol"], "AAPL");

        // Verify all 3 snapshots returned in captured_at ASC order
        // (June < Sept < Dec) despite insertion order (Sept, June, Dec)
        let snapshots = history["snapshots"].as_array().unwrap();
        assert_eq!(snapshots.len(), 3);

        assert_eq!(snapshots[0]["id"], june_model.id);
        assert_eq!(snapshots[0]["notes"], "June review");
        assert_eq!(snapshots[1]["id"], sept_model.id);
        assert_eq!(snapshots[1]["notes"], "Sept review");
        assert_eq!(snapshots[2]["id"], dec_model.id);
        assert_eq!(snapshots[2]["notes"], "Dec review");

        // Verify metrics extracted correctly on first entry (June)
        assert!((snapshots[0]["projected_sales_cagr"].as_f64().unwrap() - 6.0).abs() < 0.01);
        assert!((snapshots[0]["projected_eps_cagr"].as_f64().unwrap() - 8.5).abs() < 0.01);
        assert!((snapshots[0]["projected_high_pe"].as_f64().unwrap() - 25.0).abs() < 0.01);
        assert!((snapshots[0]["projected_low_pe"].as_f64().unwrap() - 15.0).abs() < 0.01);
        assert_eq!(snapshots[0]["thesis_locked"], true);

        // Verify captured_at fields present
        assert!(snapshots[0]["captured_at"].as_str().is_some());

        // (M3) Verify N-1 = 2 deltas for 3 snapshots with correct pairings
        let deltas = history["metric_deltas"].as_array().unwrap();
        assert_eq!(deltas.len(), 2);

        // Delta 0: June → Sept
        assert_eq!(deltas[0]["from_snapshot_id"], june_model.id);
        assert_eq!(deltas[0]["to_snapshot_id"], sept_model.id);
        // sales_cagr: 8.0 - 6.0 = 2.0
        assert!((deltas[0]["sales_cagr_delta"].as_f64().unwrap() - 2.0).abs() < 0.01);
        // eps_cagr: 10.0 - 8.5 = 1.5
        assert!((deltas[0]["eps_cagr_delta"].as_f64().unwrap() - 1.5).abs() < 0.01);

        // Delta 1: Sept → Dec
        assert_eq!(deltas[1]["from_snapshot_id"], sept_model.id);
        assert_eq!(deltas[1]["to_snapshot_id"], dec_model.id);
        // sales_cagr: 4.5 - 8.0 = -3.5
        assert!((deltas[1]["sales_cagr_delta"].as_f64().unwrap() - (-3.5)).abs() < 0.01);
        // eps_cagr: 6.0 - 10.0 = -4.0
        assert!((deltas[1]["eps_cagr_delta"].as_f64().unwrap() - (-4.0)).abs() < 0.01);
    })
    .await;
}

// -----------------------------------------------------------------------
// 4.2 — Single snapshot returns single-item array (no error)
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn history_returns_single_item_for_one_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
            "notes": "Only snapshot"
        });
        let res = request.post("/api/v1/snapshots").json(&body).await;
        res.assert_status_success();
        let created = res.json::<analysis_snapshots::Model>();

        let res = request
            .get(&format!("/api/v1/snapshots/{}/history", created.id))
            .await;
        res.assert_status_success();
        let history: serde_json::Value = res.json();

        let snapshots = history["snapshots"].as_array().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0]["id"], created.id);

        // metric_deltas should be empty for single snapshot
        let deltas = history["metric_deltas"].as_array().unwrap();
        assert!(deltas.is_empty());
    })
    .await;
}

// -----------------------------------------------------------------------
// 4.3 — Metric deltas computed correctly (including None handling)
//        Uses direct SeaORM inserts with explicit captured_at timestamps.
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn history_returns_metric_deltas() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        // Snapshot A: all metrics present (June)
        let snap_a = analysis_snapshots::ActiveModel {
            user_id: ActiveValue::set(1),
            ticker_id: ActiveValue::set(ticker_id),
            snapshot_data: ActiveValue::set(
                snapshot_data_with_metrics(Some(6.0), Some(8.5), Some(25.0), Some(15.0)),
            ),
            thesis_locked: ActiveValue::set(true),
            notes: ActiveValue::set(Some("Snapshot A".to_string())),
            captured_at: ActiveValue::set(
                chrono::DateTime::parse_from_rfc3339("2025-06-15T10:00:00+00:00").unwrap(),
            ),
            deleted_at: ActiveValue::set(None),
            ..Default::default()
        };
        let model_a = snap_a.insert(&ctx.db).await.unwrap();

        // Snapshot B: sales_cagr missing (None), other metrics changed (Sept)
        let snap_b = analysis_snapshots::ActiveModel {
            user_id: ActiveValue::set(1),
            ticker_id: ActiveValue::set(ticker_id),
            snapshot_data: ActiveValue::set(
                snapshot_data_with_metrics(None, Some(6.0), Some(22.0), Some(14.0)),
            ),
            thesis_locked: ActiveValue::set(true),
            notes: ActiveValue::set(Some("Snapshot B".to_string())),
            captured_at: ActiveValue::set(
                chrono::DateTime::parse_from_rfc3339("2025-09-15T10:00:00+00:00").unwrap(),
            ),
            deleted_at: ActiveValue::set(None),
            ..Default::default()
        };
        let model_b = snap_b.insert(&ctx.db).await.unwrap();

        let res = request
            .get(&format!("/api/v1/snapshots/{}/history", model_a.id))
            .await;
        res.assert_status_success();
        let history: serde_json::Value = res.json();

        let deltas = history["metric_deltas"].as_array().unwrap();
        assert_eq!(deltas.len(), 1);

        let delta = &deltas[0];
        assert_eq!(delta["from_snapshot_id"], model_a.id);
        assert_eq!(delta["to_snapshot_id"], model_b.id);

        // sales_cagr_delta should be null (None in B, Some in A)
        assert!(delta["sales_cagr_delta"].is_null());

        // eps_cagr_delta: 6.0 - 8.5 = -2.5
        assert!((delta["eps_cagr_delta"].as_f64().unwrap() - (-2.5)).abs() < 0.01);

        // high_pe_delta: 22.0 - 25.0 = -3.0
        assert!((delta["high_pe_delta"].as_f64().unwrap() - (-3.0)).abs() < 0.01);

        // low_pe_delta: 14.0 - 15.0 = -1.0
        assert!((delta["low_pe_delta"].as_f64().unwrap() - (-1.0)).abs() < 0.01);
    })
    .await;
}

// -----------------------------------------------------------------------
// 4.4 — Non-existent snapshot returns 404
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn history_returns_404_for_nonexistent_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let _ticker_id = seed_user_and_ticker(&ctx).await;

        let res = request.get("/api/v1/snapshots/99999/history").await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

// -----------------------------------------------------------------------
// 4.5 — Soft-deleted snapshots excluded from history
//        Uses direct SeaORM inserts with explicit captured_at timestamps.
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn history_excludes_soft_deleted_snapshots() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        // Snapshot 1: will be soft-deleted (June)
        let snap1 = analysis_snapshots::ActiveModel {
            user_id: ActiveValue::set(1),
            ticker_id: ActiveValue::set(ticker_id),
            snapshot_data: ActiveValue::set(sample_snapshot_data()),
            thesis_locked: ActiveValue::set(false),
            notes: ActiveValue::set(Some("Will be deleted".to_string())),
            captured_at: ActiveValue::set(
                chrono::DateTime::parse_from_rfc3339("2025-06-15T10:00:00+00:00").unwrap(),
            ),
            deleted_at: ActiveValue::set(None),
            ..Default::default()
        };
        let model1 = snap1.insert(&ctx.db).await.unwrap();

        // Snapshot 2: kept (Sept)
        let snap2 = analysis_snapshots::ActiveModel {
            user_id: ActiveValue::set(1),
            ticker_id: ActiveValue::set(ticker_id),
            snapshot_data: ActiveValue::set(sample_snapshot_data()),
            thesis_locked: ActiveValue::set(true),
            notes: ActiveValue::set(Some("Kept".to_string())),
            captured_at: ActiveValue::set(
                chrono::DateTime::parse_from_rfc3339("2025-09-15T10:00:00+00:00").unwrap(),
            ),
            deleted_at: ActiveValue::set(None),
            ..Default::default()
        };
        let model2 = snap2.insert(&ctx.db).await.unwrap();

        // Soft-delete first snapshot via API
        let res = request
            .delete(&format!("/api/v1/snapshots/{}", model1.id))
            .await;
        res.assert_status_success();

        // History via kept snapshot should only show the kept one
        let res = request
            .get(&format!("/api/v1/snapshots/{}/history", model2.id))
            .await;
        res.assert_status_success();
        let history: serde_json::Value = res.json();

        let snapshots = history["snapshots"].as_array().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0]["id"], model2.id);
    })
    .await;
}

// -----------------------------------------------------------------------
// 4.6 — Monetary fields present in history entries with actual values
//        Uses snapshot data with a real historical record so
//        extract_snapshot_prices returns non-null prices.
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn history_includes_monetary_fields() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        // Snapshot with a real record: fiscal_year=2025, price_high=150, eps=5.0
        // projected_eps_cagr=10%, projected_high_pe=25, projected_low_pe=15
        // Expected: current_price=150, target_high=25*5*(1.1)^5, target_low=15*5*(1.1)^5
        let rich_data = serde_json::json!({
            "historical_data": {
                "ticker": "AAPL",
                "currency": "USD",
                "records": [{
                    "fiscal_year": 2025,
                    "sales": 100000,
                    "eps": 5.0,
                    "price_high": 150.0,
                    "price_low": 120.0,
                    "adjustment_factor": 1.0
                }],
                "is_complete": true,
                "is_split_adjusted": true
            },
            "projected_sales_cagr": 10.5,
            "projected_eps_cagr": 10.0,
            "projected_high_pe": 25.0,
            "projected_low_pe": 15.0,
            "analyst_note": "",
            "captured_at": "2026-01-01T00:00:00Z"
        });

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": rich_data,
            "thesis_locked": true,
            "notes": "Monetary test"
        });
        let res = request.post("/api/v1/snapshots").json(&body).await;
        res.assert_status_success();
        let created = res.json::<analysis_snapshots::Model>();

        let res = request
            .get(&format!("/api/v1/snapshots/{}/history", created.id))
            .await;
        res.assert_status_success();
        let history: serde_json::Value = res.json();

        let entry = &history["snapshots"][0];
        // native_currency from historical_data.currency
        assert_eq!(entry["native_currency"], "USD");
        // current_price = price_high of latest record = 150.0
        assert!((entry["current_price"].as_f64().unwrap() - 150.0).abs() < 0.01);
        // target prices should be non-null (projected from eps * (1+cagr)^5 * PE)
        assert!(entry["target_high_price"].as_f64().unwrap() > 0.0);
        assert!(entry["target_low_price"].as_f64().unwrap() > 0.0);
        // target_high > target_low (high_pe > low_pe)
        assert!(entry["target_high_price"].as_f64().unwrap() > entry["target_low_price"].as_f64().unwrap());
        // upside_downside_ratio should be computed
        assert!(entry["upside_downside_ratio"].as_f64().is_some());
    })
    .await;
}

// -----------------------------------------------------------------------
// 4.7 — Soft-deleted anchor snapshot returns 404
// -----------------------------------------------------------------------
#[tokio::test]
#[serial]
async fn history_returns_404_for_deleted_anchor_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "ticker_id": ticker_id,
            "snapshot_data": sample_snapshot_data(),
            "thesis_locked": false,
            "notes": "Will be deleted"
        });
        let res = request.post("/api/v1/snapshots").json(&body).await;
        let created = res.json::<analysis_snapshots::Model>();

        // Soft-delete the snapshot
        request
            .delete(&format!("/api/v1/snapshots/{}", created.id))
            .await;

        // History via deleted anchor should return 404
        let res = request
            .get(&format!("/api/v1/snapshots/{}/history", created.id))
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
