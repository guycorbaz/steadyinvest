use backend::app::App;
use backend::models::_entities::{analysis_snapshots, tickers, users};
use loco_rs::prelude::*;
use loco_rs::testing::prelude::request;
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
use serial_test::serial;

/// Ensure a user and ticker exist for FK constraints.
/// Returns the ticker_id for use in snapshot requests.
async fn seed_user_and_ticker(ctx: &AppContext) -> i32 {
    let _user = users::Entity::find_by_id(1)
        .one(&ctx.db)
        .await
        .unwrap()
        .expect("User id=1 should exist from fixture seed");

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
        "valuation_zone": "undervalued",
        "analyst_note": "",
        "captured_at": "2026-01-01T00:00:00Z"
    })
}

fn sample_snapshot_data_2() -> serde_json::Value {
    serde_json::json!({
        "historical_data": { "ticker": "MSFT", "currency": "USD", "records": [], "is_complete": false, "is_split_adjusted": false },
        "projected_sales_cagr": 8.0,
        "projected_eps_cagr": 9.5,
        "projected_high_pe": 30.0,
        "projected_low_pe": 20.0,
        "valuation_zone": "overvalued",
        "analyst_note": "",
        "captured_at": "2026-01-02T00:00:00Z"
    })
}

/// Create a snapshot via the API and return its id.
async fn create_snapshot(request: &loco_rs::TestServer, ticker_id: i32, data: serde_json::Value) -> i32 {
    let body = serde_json::json!({
        "ticker_id": ticker_id,
        "snapshot_data": data,
        "thesis_locked": false,
        "notes": "Test snapshot"
    });
    let res = request.post("/api/v1/snapshots").json(&body).await;
    res.assert_status_success();
    res.json::<analysis_snapshots::Model>().id
}

// -----------------------------------------------------------------------
// Ad-hoc compare — AC #3
// -----------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn ad_hoc_compare_returns_latest_snapshots() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        // Create two snapshots for the same ticker — should return only the latest
        create_snapshot(&request, ticker_id, sample_snapshot_data()).await;
        let latest_id = create_snapshot(&request, ticker_id, sample_snapshot_data_2()).await;

        let res = request
            .get(&format!("/api/v1/compare?ticker_ids={}&base_currency=CHF", ticker_id))
            .await;
        res.assert_status_success();

        let body: serde_json::Value = res.json();
        assert_eq!(body["base_currency"], "CHF");
        let snapshots = body["snapshots"].as_array().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0]["id"], latest_id);
        assert_eq!(snapshots[0]["ticker_id"], ticker_id);
        assert_eq!(snapshots[0]["ticker_symbol"], "AAPL");
        // Verify key metrics from sample_snapshot_data_2
        assert!((snapshots[0]["projected_sales_cagr"].as_f64().unwrap() - 8.0).abs() < 0.01);
        assert!((snapshots[0]["projected_eps_cagr"].as_f64().unwrap() - 9.5).abs() < 0.01);
        assert_eq!(snapshots[0]["valuation_zone"], "overvalued");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn ad_hoc_compare_empty_ticker_ids_returns_empty() {
    request::<App, _, _>(|request, ctx| async move {
        let _ticker_id = seed_user_and_ticker(&ctx).await;

        let res = request.get("/api/v1/compare").await;
        res.assert_status_success();

        let body: serde_json::Value = res.json();
        let snapshots = body["snapshots"].as_array().unwrap();
        assert_eq!(snapshots.len(), 0);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn ad_hoc_compare_nonexistent_ticker_is_skipped() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;
        create_snapshot(&request, ticker_id, sample_snapshot_data()).await;

        let res = request
            .get(&format!("/api/v1/compare?ticker_ids={},99999", ticker_id))
            .await;
        res.assert_status_success();

        let body: serde_json::Value = res.json();
        let snapshots = body["snapshots"].as_array().unwrap();
        // Only the existing ticker is returned; 99999 is silently skipped
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0]["ticker_id"], ticker_id);
    })
    .await;
}

// -----------------------------------------------------------------------
// Create comparison set — AC #4
// -----------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn can_create_comparison_set() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;
        let snap_id = create_snapshot(&request, ticker_id, sample_snapshot_data()).await;

        let body = serde_json::json!({
            "name": "My Comparison",
            "base_currency": "CHF",
            "items": [
                { "analysis_snapshot_id": snap_id, "sort_order": 1 }
            ]
        });

        let res = request.post("/api/v1/comparisons").json(&body).await;
        res.assert_status_success();

        let detail: serde_json::Value = res.json();
        assert_eq!(detail["name"], "My Comparison");
        assert_eq!(detail["base_currency"], "CHF");
        let items = detail["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["sort_order"], 1);
        assert_eq!(items[0]["snapshot"]["id"], snap_id);
        assert_eq!(items[0]["snapshot"]["ticker_symbol"], "AAPL");
        assert_eq!(items[0]["snapshot"]["valuation_zone"], "undervalued");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn create_comparison_rejects_empty_name() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;
        let snap_id = create_snapshot(&request, ticker_id, sample_snapshot_data()).await;

        let body = serde_json::json!({
            "name": "  ",
            "base_currency": "CHF",
            "items": [
                { "analysis_snapshot_id": snap_id, "sort_order": 1 }
            ]
        });

        let res = request.post("/api/v1/comparisons").json(&body).await;
        assert_eq!(res.status_code(), 422);
        let err: serde_json::Value = res.json();
        assert_eq!(err["error"], "Name must not be empty");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn create_comparison_rejects_nonexistent_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let _ticker_id = seed_user_and_ticker(&ctx).await;

        let body = serde_json::json!({
            "name": "Invalid Set",
            "base_currency": "USD",
            "items": [
                { "analysis_snapshot_id": 99999, "sort_order": 1 }
            ]
        });

        let res = request.post("/api/v1/comparisons").json(&body).await;
        assert_eq!(res.status_code(), 422);
        let err: serde_json::Value = res.json();
        assert!(err["error"].as_str().unwrap().contains("99999"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn create_comparison_preserves_sort_order() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;
        let snap1 = create_snapshot(&request, ticker_id, sample_snapshot_data()).await;
        let snap2 = create_snapshot(&request, ticker_id, sample_snapshot_data_2()).await;

        let body = serde_json::json!({
            "name": "Sorted Set",
            "base_currency": "EUR",
            "items": [
                { "analysis_snapshot_id": snap2, "sort_order": 1 },
                { "analysis_snapshot_id": snap1, "sort_order": 2 }
            ]
        });

        let res = request.post("/api/v1/comparisons").json(&body).await;
        res.assert_status_success();

        let detail: serde_json::Value = res.json();
        let items = detail["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        // Items ordered by sort_order
        assert_eq!(items[0]["sort_order"], 1);
        assert_eq!(items[0]["snapshot"]["id"], snap2);
        assert_eq!(items[1]["sort_order"], 2);
        assert_eq!(items[1]["snapshot"]["id"], snap1);
    })
    .await;
}

// -----------------------------------------------------------------------
// List comparison sets — AC #5
// -----------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn can_list_comparison_sets() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;
        let snap_id = create_snapshot(&request, ticker_id, sample_snapshot_data()).await;

        // Create two sets
        for name in ["Set A", "Set B"] {
            let body = serde_json::json!({
                "name": name,
                "base_currency": "USD",
                "items": [
                    { "analysis_snapshot_id": snap_id, "sort_order": 1 }
                ]
            });
            request.post("/api/v1/comparisons").json(&body).await.assert_status_success();
        }

        let res = request.get("/api/v1/comparisons").await;
        res.assert_status_success();

        let sets: Vec<serde_json::Value> = res.json();
        assert_eq!(sets.len(), 2);
        // Ordered by created_at desc — Set B created last, so first in list
        assert_eq!(sets[0]["name"], "Set B");
        assert_eq!(sets[1]["name"], "Set A");
        assert_eq!(sets[0]["item_count"], 1);
        assert_eq!(sets[0]["base_currency"], "USD");
    })
    .await;
}

// -----------------------------------------------------------------------
// Get comparison set — AC #6
// -----------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn can_get_comparison_set_detail() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;
        let snap_id = create_snapshot(&request, ticker_id, sample_snapshot_data()).await;

        let body = serde_json::json!({
            "name": "Detail Test",
            "base_currency": "CHF",
            "items": [
                { "analysis_snapshot_id": snap_id, "sort_order": 1 }
            ]
        });
        let res = request.post("/api/v1/comparisons").json(&body).await;
        let created: serde_json::Value = res.json();
        let set_id = created["id"].as_i64().unwrap();

        let res = request.get(&format!("/api/v1/comparisons/{}", set_id)).await;
        res.assert_status_success();

        let detail: serde_json::Value = res.json();
        assert_eq!(detail["id"], set_id);
        assert_eq!(detail["name"], "Detail Test");
        assert_eq!(detail["base_currency"], "CHF");
        assert!(detail["created_at"].is_string());
        assert!(detail["updated_at"].is_string());
        let items = detail["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["snapshot"]["id"], snap_id);
        assert_eq!(items[0]["snapshot"]["ticker_symbol"], "AAPL");
        assert!((items[0]["snapshot"]["projected_sales_cagr"].as_f64().unwrap() - 10.5).abs() < 0.01);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn get_nonexistent_comparison_set_returns_404() {
    request::<App, _, _>(|request, ctx| async move {
        let _ticker_id = seed_user_and_ticker(&ctx).await;

        let res = request.get("/api/v1/comparisons/99999").await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

// -----------------------------------------------------------------------
// Update comparison set — AC #6
// -----------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn can_update_comparison_set() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;
        let snap1 = create_snapshot(&request, ticker_id, sample_snapshot_data()).await;
        let snap2 = create_snapshot(&request, ticker_id, sample_snapshot_data_2()).await;

        // Create initial set with snap1
        let body = serde_json::json!({
            "name": "Original",
            "base_currency": "USD",
            "items": [
                { "analysis_snapshot_id": snap1, "sort_order": 1 }
            ]
        });
        let res = request.post("/api/v1/comparisons").json(&body).await;
        let created: serde_json::Value = res.json();
        let set_id = created["id"].as_i64().unwrap();

        // Update: change name, currency, and replace items with snap2
        let update = serde_json::json!({
            "name": "Updated",
            "base_currency": "EUR",
            "items": [
                { "analysis_snapshot_id": snap2, "sort_order": 1 },
                { "analysis_snapshot_id": snap1, "sort_order": 2 }
            ]
        });
        let res = request
            .put(&format!("/api/v1/comparisons/{}", set_id))
            .json(&update)
            .await;
        res.assert_status_success();

        let detail: serde_json::Value = res.json();
        assert_eq!(detail["name"], "Updated");
        assert_eq!(detail["base_currency"], "EUR");
        let items = detail["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["snapshot"]["id"], snap2);
        assert_eq!(items[1]["snapshot"]["id"], snap1);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn update_nonexistent_comparison_set_returns_404() {
    request::<App, _, _>(|request, ctx| async move {
        let _ticker_id = seed_user_and_ticker(&ctx).await;

        let update = serde_json::json!({
            "name": "Nope",
            "base_currency": "USD",
            "items": []
        });
        let res = request
            .put("/api/v1/comparisons/99999")
            .json(&update)
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

// -----------------------------------------------------------------------
// Delete comparison set — AC #6
// -----------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn can_delete_comparison_set() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;
        let snap_id = create_snapshot(&request, ticker_id, sample_snapshot_data()).await;

        let body = serde_json::json!({
            "name": "To Delete",
            "base_currency": "USD",
            "items": [
                { "analysis_snapshot_id": snap_id, "sort_order": 1 }
            ]
        });
        let res = request.post("/api/v1/comparisons").json(&body).await;
        let created: serde_json::Value = res.json();
        let set_id = created["id"].as_i64().unwrap();

        // Delete
        let res = request
            .delete(&format!("/api/v1/comparisons/{}", set_id))
            .await;
        res.assert_status_success();
        let del: serde_json::Value = res.json();
        assert_eq!(del["status"], "deleted");

        // Verify: GET returns 404
        let res = request
            .get(&format!("/api/v1/comparisons/{}", set_id))
            .await;
        assert_eq!(res.status_code(), 404);

        // Verify: list is empty
        let res = request.get("/api/v1/comparisons").await;
        let sets: Vec<serde_json::Value> = res.json();
        assert_eq!(sets.len(), 0);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn delete_nonexistent_comparison_set_returns_404() {
    request::<App, _, _>(|request, ctx| async move {
        let _ticker_id = seed_user_and_ticker(&ctx).await;

        let res = request.delete("/api/v1/comparisons/99999").await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

// -----------------------------------------------------------------------
// Version pinning — AC #4
// -----------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn version_pinning_preserves_original_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        let ticker_id = seed_user_and_ticker(&ctx).await;

        // Create first snapshot
        let snap1 = create_snapshot(&request, ticker_id, sample_snapshot_data()).await;

        // Save comparison referencing snap1
        let body = serde_json::json!({
            "name": "Pinned",
            "base_currency": "USD",
            "items": [
                { "analysis_snapshot_id": snap1, "sort_order": 1 }
            ]
        });
        let res = request.post("/api/v1/comparisons").json(&body).await;
        let created: serde_json::Value = res.json();
        let set_id = created["id"].as_i64().unwrap();

        // Create a NEW snapshot for the same ticker (simulates re-analysis)
        let _snap2 = create_snapshot(&request, ticker_id, sample_snapshot_data_2()).await;

        // Retrieve the comparison set — it should still reference snap1
        let res = request
            .get(&format!("/api/v1/comparisons/{}", set_id))
            .await;
        res.assert_status_success();

        let detail: serde_json::Value = res.json();
        let items = detail["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        // Still references the original snapshot, not the new one
        assert_eq!(items[0]["snapshot"]["id"], snap1);
        // Verify original metrics (from sample_snapshot_data, not sample_snapshot_data_2)
        assert!((items[0]["snapshot"]["projected_sales_cagr"].as_f64().unwrap() - 10.5).abs() < 0.01);
    })
    .await;
}
