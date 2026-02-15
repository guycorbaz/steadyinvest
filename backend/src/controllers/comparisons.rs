//! Comparison set and ad-hoc compare controller (Phase 1 API).
//!
//! Provides:
//! - `GET /api/v1/compare` — ad-hoc comparison of latest snapshots by ticker
//! - CRUD under `/api/v1/comparisons` for persisted comparison sets
//!
//! **Version pinning**: saved comparison items reference specific snapshot IDs.
//! Re-analyzing a stock creates a new snapshot; existing comparisons are unaffected.
//!
//! **Currency**: `base_currency` is stored but no conversion is performed (Story 8.3).

use loco_rs::prelude::*;
use sea_orm::{IntoActiveModel, PaginatorTrait, QueryOrder, TransactionTrait};
use serde::{Deserialize, Serialize};

use crate::models::_entities::{
    analysis_snapshots, comparison_set_items, comparison_sets, tickers,
};

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

/// Query parameters for the ad-hoc compare endpoint.
#[derive(Debug, Deserialize)]
pub struct CompareQueryParams {
    /// Comma-separated ticker IDs (e.g. "1,2,3").
    pub ticker_ids: Option<String>,
    /// Accepted and echoed in the response; no conversion performed yet.
    pub base_currency: Option<String>,
}

/// Request body for creating a comparison set.
#[derive(Debug, Deserialize)]
pub struct CreateComparisonSetRequest {
    pub name: String,
    pub base_currency: String,
    pub items: Vec<ComparisonSetItemInput>,
}

/// Request body for updating a comparison set (full replacement).
#[derive(Debug, Deserialize)]
pub struct UpdateComparisonSetRequest {
    pub name: String,
    pub base_currency: String,
    pub items: Vec<ComparisonSetItemInput>,
}

/// A single item in a create/update request.
#[derive(Debug, Deserialize)]
pub struct ComparisonSetItemInput {
    pub analysis_snapshot_id: i32,
    pub sort_order: i32,
}

/// Key metrics extracted from a snapshot for comparison views.
///
/// Used by both ad-hoc and persisted comparison endpoints.
#[derive(Debug, Serialize)]
pub struct ComparisonSnapshotSummary {
    pub id: i32,
    pub ticker_id: i32,
    pub ticker_symbol: String,
    pub thesis_locked: bool,
    pub captured_at: chrono::DateTime<chrono::FixedOffset>,
    pub notes: Option<String>,
    pub projected_sales_cagr: Option<f64>,
    pub projected_eps_cagr: Option<f64>,
    pub projected_high_pe: Option<f64>,
    pub projected_low_pe: Option<f64>,
    pub valuation_zone: Option<String>,
}

impl ComparisonSnapshotSummary {
    /// Build from a snapshot model and its related ticker.
    fn from_model_and_ticker(
        m: analysis_snapshots::Model,
        ticker: Option<tickers::Model>,
    ) -> Self {
        let ticker_symbol = ticker
            .map(|t| t.ticker)
            .unwrap_or_else(|| format!("ID:{}", m.ticker_id));
        Self {
            id: m.id,
            ticker_id: m.ticker_id,
            ticker_symbol,
            thesis_locked: m.thesis_locked,
            notes: m.notes,
            captured_at: m.captured_at,
            projected_sales_cagr: m
                .snapshot_data
                .get("projected_sales_cagr")
                .and_then(|v| v.as_f64()),
            projected_eps_cagr: m
                .snapshot_data
                .get("projected_eps_cagr")
                .and_then(|v| v.as_f64()),
            projected_high_pe: m
                .snapshot_data
                .get("projected_high_pe")
                .and_then(|v| v.as_f64()),
            projected_low_pe: m
                .snapshot_data
                .get("projected_low_pe")
                .and_then(|v| v.as_f64()),
            valuation_zone: m
                .snapshot_data
                .get("valuation_zone")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned()),
        }
    }
}

/// Response wrapper for the ad-hoc compare endpoint.
#[derive(Debug, Serialize)]
pub struct AdHocCompareResponse {
    pub base_currency: Option<String>,
    pub snapshots: Vec<ComparisonSnapshotSummary>,
}

/// Lightweight summary for the list endpoint.
#[derive(Debug, Serialize)]
pub struct ComparisonSetSummary {
    pub id: i32,
    pub name: String,
    pub base_currency: String,
    pub item_count: i64,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
}

/// Full detail for the get endpoint.
#[derive(Debug, Serialize)]
pub struct ComparisonSetDetail {
    pub id: i32,
    pub name: String,
    pub base_currency: String,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
    pub items: Vec<ComparisonSetItemDetail>,
}

/// A single item within a comparison set detail response.
#[derive(Debug, Serialize)]
pub struct ComparisonSetItemDetail {
    pub id: i32,
    pub sort_order: i32,
    pub snapshot: ComparisonSnapshotSummary,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a 422 Unprocessable Entity JSON response.
fn unprocessable_entity(message: &str) -> Result<Response> {
    Ok(Response::builder()
        .status(axum::http::StatusCode::UNPROCESSABLE_ENTITY)
        .header("Content-Type", "application/json")
        .body(
            serde_json::json!({ "error": message })
                .to_string()
                .into(),
        )
        .map_err(|e| Error::string(&e.to_string()))?)
}

// ---------------------------------------------------------------------------
// Handlers — Ad-hoc compare
// ---------------------------------------------------------------------------

/// Returns the latest non-deleted snapshot for each requested ticker.
///
/// **GET** `/api/v1/compare?ticker_ids=1,2,3&base_currency=CHF`
#[debug_handler]
pub async fn ad_hoc_compare(
    State(ctx): State<AppContext>,
    Query(params): Query<CompareQueryParams>,
) -> Result<Response> {
    let ticker_ids: Vec<i32> = params
        .ticker_ids
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| s.trim().parse::<i32>().ok())
        .collect();

    let mut snapshots = Vec::with_capacity(ticker_ids.len());

    for ticker_id in ticker_ids {
        let result = analysis_snapshots::Entity::find()
            .find_also_related(tickers::Entity)
            .filter(analysis_snapshots::Column::TickerId.eq(ticker_id))
            .filter(analysis_snapshots::Column::DeletedAt.is_null())
            .order_by_desc(analysis_snapshots::Column::CapturedAt)
            .one(&ctx.db)
            .await?;

        if let Some((snapshot, ticker)) = result {
            snapshots.push(ComparisonSnapshotSummary::from_model_and_ticker(
                snapshot, ticker,
            ));
        }
        // Non-existent or all-deleted ticker_ids are silently skipped
    }

    format::json(AdHocCompareResponse {
        base_currency: params.base_currency,
        snapshots,
    })
}

// ---------------------------------------------------------------------------
// Handlers — CRUD
// ---------------------------------------------------------------------------

/// Creates a new comparison set with items.
///
/// **POST** `/api/v1/comparisons`
#[debug_handler]
pub async fn create_comparison_set(
    State(ctx): State<AppContext>,
    Json(req): Json<CreateComparisonSetRequest>,
) -> Result<Response> {
    // Validate name non-empty
    if req.name.trim().is_empty() {
        return unprocessable_entity("Name must not be empty");
    }

    // Validate base_currency is exactly 3 characters
    if req.base_currency.len() != 3 {
        return unprocessable_entity("base_currency must be exactly 3 characters");
    }

    // Validate all snapshot IDs exist and are not deleted
    for item in &req.items {
        let snapshot = analysis_snapshots::Entity::find_by_id(item.analysis_snapshot_id)
            .filter(analysis_snapshots::Column::DeletedAt.is_null())
            .one(&ctx.db)
            .await?;
        if snapshot.is_none() {
            return unprocessable_entity(&format!(
                "Snapshot {} does not exist or is deleted",
                item.analysis_snapshot_id
            ));
        }
    }

    // Transaction: create set + items atomically
    let txn = ctx.db.begin().await?;

    let now: chrono::DateTime<chrono::FixedOffset> = chrono::Utc::now().into();
    let set = comparison_sets::ActiveModel {
        user_id: ActiveValue::set(1),
        name: ActiveValue::set(req.name),
        base_currency: ActiveValue::set(req.base_currency),
        created_at: ActiveValue::set(now),
        updated_at: ActiveValue::set(now),
        ..Default::default()
    };
    let set = set.insert(&txn).await?;

    for item in &req.items {
        let active = comparison_set_items::ActiveModel {
            comparison_set_id: ActiveValue::set(set.id),
            analysis_snapshot_id: ActiveValue::set(item.analysis_snapshot_id),
            sort_order: ActiveValue::set(item.sort_order),
            ..Default::default()
        };
        active.insert(&txn).await?;
    }

    txn.commit().await?;

    // Return full detail response (reads committed data)
    let detail = build_set_detail(&ctx, set).await?;
    format::json(detail)
}

/// Lists all comparison sets for the current user.
///
/// **GET** `/api/v1/comparisons`
#[debug_handler]
pub async fn list_comparison_sets(State(ctx): State<AppContext>) -> Result<Response> {
    let sets = comparison_sets::Entity::find()
        .filter(comparison_sets::Column::UserId.eq(1))
        .order_by_desc(comparison_sets::Column::CreatedAt)
        .all(&ctx.db)
        .await?;

    let mut summaries = Vec::with_capacity(sets.len());
    for set in sets {
        let item_count = comparison_set_items::Entity::find()
            .filter(comparison_set_items::Column::ComparisonSetId.eq(set.id))
            .count(&ctx.db)
            .await?;
        summaries.push(ComparisonSetSummary {
            id: set.id,
            name: set.name,
            base_currency: set.base_currency,
            item_count: item_count as i64,
            created_at: set.created_at,
        });
    }

    format::json(summaries)
}

/// Returns a comparison set with full snapshot data per item.
///
/// **GET** `/api/v1/comparisons/:id`
#[debug_handler]
pub async fn get_comparison_set(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let set = comparison_sets::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    if set.user_id != 1 {
        return Err(Error::NotFound);
    }

    let detail = build_set_detail(&ctx, set).await?;
    format::json(detail)
}

/// Updates a comparison set (full replacement of name, base_currency, and items).
///
/// **PUT** `/api/v1/comparisons/:id`
#[debug_handler]
pub async fn update_comparison_set(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
    Json(req): Json<UpdateComparisonSetRequest>,
) -> Result<Response> {
    let set = comparison_sets::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    if set.user_id != 1 {
        return Err(Error::NotFound);
    }

    // Validate name non-empty
    if req.name.trim().is_empty() {
        return unprocessable_entity("Name must not be empty");
    }

    // Validate base_currency is exactly 3 characters
    if req.base_currency.len() != 3 {
        return unprocessable_entity("base_currency must be exactly 3 characters");
    }

    // Validate all snapshot IDs exist and are not deleted
    for item in &req.items {
        let snapshot = analysis_snapshots::Entity::find_by_id(item.analysis_snapshot_id)
            .filter(analysis_snapshots::Column::DeletedAt.is_null())
            .one(&ctx.db)
            .await?;
        if snapshot.is_none() {
            return unprocessable_entity(&format!(
                "Snapshot {} does not exist or is deleted",
                item.analysis_snapshot_id
            ));
        }
    }

    // Transaction: update set + replace items atomically
    let txn = ctx.db.begin().await?;

    let mut active = set.into_active_model();
    active.name = ActiveValue::set(req.name);
    active.base_currency = ActiveValue::set(req.base_currency);
    active.updated_at = ActiveValue::set(chrono::Utc::now().into());
    let set = active.update(&txn).await?;

    // Replace items: delete existing, insert new
    comparison_set_items::Entity::delete_many()
        .filter(comparison_set_items::Column::ComparisonSetId.eq(set.id))
        .exec(&txn)
        .await?;

    for item in &req.items {
        let active = comparison_set_items::ActiveModel {
            comparison_set_id: ActiveValue::set(set.id),
            analysis_snapshot_id: ActiveValue::set(item.analysis_snapshot_id),
            sort_order: ActiveValue::set(item.sort_order),
            ..Default::default()
        };
        active.insert(&txn).await?;
    }

    txn.commit().await?;

    // Return full detail response (reads committed data)
    let detail = build_set_detail(&ctx, set).await?;
    format::json(detail)
}

/// Deletes a comparison set (items cascade).
///
/// **DELETE** `/api/v1/comparisons/:id`
#[debug_handler]
pub async fn delete_comparison_set(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let set = comparison_sets::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    if set.user_id != 1 {
        return Err(Error::NotFound);
    }

    comparison_sets::Entity::delete_by_id(set.id)
        .exec(&ctx.db)
        .await?;

    format::json(serde_json::json!({ "status": "deleted" }))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build a full detail response for a comparison set.
async fn build_set_detail(
    ctx: &AppContext,
    set: comparison_sets::Model,
) -> Result<ComparisonSetDetail> {
    let items = comparison_set_items::Entity::find()
        .filter(comparison_set_items::Column::ComparisonSetId.eq(set.id))
        .order_by_asc(comparison_set_items::Column::SortOrder)
        .all(&ctx.db)
        .await?;

    let mut item_details = Vec::with_capacity(items.len());
    for item in items {
        let (snapshot, ticker) = analysis_snapshots::Entity::find_by_id(
            item.analysis_snapshot_id,
        )
        .find_also_related(tickers::Entity)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| {
            Error::string(&format!(
                "Referenced snapshot {} not found",
                item.analysis_snapshot_id
            ))
        })?;

        item_details.push(ComparisonSetItemDetail {
            id: item.id,
            sort_order: item.sort_order,
            snapshot: ComparisonSnapshotSummary::from_model_and_ticker(snapshot, ticker),
        });
    }

    Ok(ComparisonSetDetail {
        id: set.id,
        name: set.name,
        base_currency: set.base_currency,
        created_at: set.created_at,
        updated_at: set.updated_at,
        items: item_details,
    })
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

/// Registers the ad-hoc compare route under `/api/v1/compare`.
pub fn compare_routes() -> Routes {
    Routes::new()
        .prefix("api/v1/compare")
        .add("/", get(ad_hoc_compare))
}

/// Registers comparison set CRUD routes under `/api/v1/comparisons`.
pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/v1/comparisons")
        .add("/", post(create_comparison_set))
        .add("/", get(list_comparison_sets))
        .add("/{id}", get(get_comparison_set))
        .add("/{id}", put(update_comparison_set))
        .add("/{id}", delete(delete_comparison_set))
}
