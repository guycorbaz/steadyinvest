//! Analysis snapshot CRUD controller (Phase 1 API).
//!
//! Provides versioned endpoints under `/api/v1/snapshots` for creating,
//! listing, retrieving, and soft-deleting analysis snapshots.
//!
//! **Append-only model**: `POST` creates new rows; `PUT`/`PATCH` are rejected.
//! **Immutability contract**: locked snapshots reject deletion.

use base64::Engine;
use loco_rs::prelude::*;
use sea_orm::{IntoActiveModel, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::models::_entities::{analysis_snapshots, tickers};

/// Maximum base64-encoded chart image size (5 MB).
const MAX_CHART_IMAGE_BASE64_LEN: usize = 5 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

/// Request body for creating an analysis snapshot.
///
/// Either `ticker_id` or `ticker` must be provided. If `ticker` is given
/// without `ticker_id`, the server resolves the ticker symbol to its database
/// ID. This avoids requiring the frontend to know the numeric ticker ID.
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateSnapshotRequest {
    pub ticker_id: Option<i32>,
    pub ticker: Option<String>,
    pub snapshot_data: serde_json::Value,
    pub thesis_locked: bool,
    pub notes: Option<String>,
    pub chart_image: Option<String>,
}

/// Optional query-string filters for listing snapshots.
#[derive(Debug, Deserialize)]
pub struct SnapshotQueryParams {
    pub ticker_id: Option<i32>,
    pub thesis_locked: Option<bool>,
}

/// Lightweight summary returned by the list endpoint.
///
/// Excludes `snapshot_data` and `chart_image` to keep payloads small.
/// Includes ticker symbol (resolved via join) and key metrics extracted
/// from `snapshot_data` for Compact Analysis Card rendering.
#[derive(Debug, Serialize)]
pub struct SnapshotSummary {
    pub id: i32,
    pub ticker_id: i32,
    pub ticker_symbol: String,
    pub thesis_locked: bool,
    pub notes: Option<String>,
    pub captured_at: chrono::DateTime<chrono::FixedOffset>,
    pub projected_sales_cagr: Option<f64>,
    pub projected_eps_cagr: Option<f64>,
    pub projected_high_pe: Option<f64>,
    pub projected_low_pe: Option<f64>,
}

impl SnapshotSummary {
    /// Build a summary from a snapshot model and its related ticker.
    fn from_model_and_ticker(
        m: analysis_snapshots::Model,
        ticker: Option<tickers::Model>,
    ) -> Self {
        let ticker_symbol = ticker
            .map(|t| t.ticker)
            .unwrap_or_else(|| format!("ID:{}", m.ticker_id));
        let projected_sales_cagr = m
            .snapshot_data
            .get("projected_sales_cagr")
            .and_then(|v| v.as_f64());
        let projected_eps_cagr = m
            .snapshot_data
            .get("projected_eps_cagr")
            .and_then(|v| v.as_f64());
        let projected_high_pe = m
            .snapshot_data
            .get("projected_high_pe")
            .and_then(|v| v.as_f64());
        let projected_low_pe = m
            .snapshot_data
            .get("projected_low_pe")
            .and_then(|v| v.as_f64());
        Self {
            id: m.id,
            ticker_id: m.ticker_id,
            ticker_symbol,
            thesis_locked: m.thesis_locked,
            notes: m.notes,
            captured_at: m.captured_at,
            projected_sales_cagr,
            projected_eps_cagr,
            projected_high_pe,
            projected_low_pe,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a 403 Forbidden JSON response.
fn forbidden(message: &str) -> Result<Response> {
    Ok(Response::builder()
        .status(axum::http::StatusCode::FORBIDDEN)
        .header("Content-Type", "application/json")
        .body(
            serde_json::json!({ "error": message })
                .to_string()
                .into(),
        )
        .map_err(|e| Error::string(&e.to_string()))?)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Build a 400 Bad Request JSON response.
fn bad_request(message: &str) -> Result<Response> {
    Ok(Response::builder()
        .status(axum::http::StatusCode::BAD_REQUEST)
        .header("Content-Type", "application/json")
        .body(
            serde_json::json!({ "error": message })
                .to_string()
                .into(),
        )
        .map_err(|e| Error::string(&e.to_string()))?)
}

/// Creates a new analysis snapshot (append-only).
///
/// **POST** `/api/v1/snapshots`
#[debug_handler]
pub async fn create_snapshot(
    State(ctx): State<AppContext>,
    Json(req): Json<CreateSnapshotRequest>,
) -> Result<Response> {
    // Resolve ticker_id: use provided value or look up by ticker symbol
    let ticker_id = match (req.ticker_id, &req.ticker) {
        (Some(id), _) => {
            // Validate that the ticker exists
            tickers::Entity::find_by_id(id)
                .one(&ctx.db)
                .await?
                .ok_or_else(|| Error::NotFound)?;
            id
        }
        (None, Some(symbol)) => {
            let t = tickers::Entity::find()
                .filter(tickers::Column::Ticker.eq(symbol.as_str()))
                .one(&ctx.db)
                .await?
                .ok_or_else(|| Error::NotFound)?;
            t.id
        }
        (None, None) => {
            return bad_request("Either ticker_id or ticker must be provided");
        }
    };

    // Decode chart_image from base64 if provided
    let chart_image_bytes = if let Some(ref b64) = req.chart_image {
        if b64.len() > MAX_CHART_IMAGE_BASE64_LEN {
            return bad_request("Chart image exceeds maximum size");
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|_| Error::string("Invalid base64 in chart_image"))?;
        // Validate PNG magic number (0x89 P N G)
        if bytes.len() < 4 || bytes[..4] != [0x89, 0x50, 0x4E, 0x47] {
            return bad_request("Chart image is not a valid PNG");
        }
        Some(bytes)
    } else {
        None
    };

    let active = analysis_snapshots::ActiveModel {
        user_id: ActiveValue::set(1), // default single-user until Phase 3
        ticker_id: ActiveValue::set(ticker_id),
        snapshot_data: ActiveValue::set(req.snapshot_data),
        thesis_locked: ActiveValue::set(req.thesis_locked),
        chart_image: ActiveValue::set(chart_image_bytes),
        notes: ActiveValue::set(req.notes),
        captured_at: ActiveValue::set(chrono::Utc::now().into()),
        deleted_at: ActiveValue::set(None),
        ..Default::default()
    };

    let model = active.insert(&ctx.db).await?;
    format::json(model)
}

/// Lists snapshots with optional filters, returning summaries only.
///
/// **GET** `/api/v1/snapshots?ticker_id=X&thesis_locked=true`
///
/// Joins with `tickers` table to include the ticker symbol in each summary.
/// Extracts key metrics from `snapshot_data` JSON for Compact Analysis Cards.
#[debug_handler]
pub async fn list_snapshots(
    State(ctx): State<AppContext>,
    Query(params): Query<SnapshotQueryParams>,
) -> Result<Response> {
    let mut query = analysis_snapshots::Entity::find()
        .find_also_related(tickers::Entity)
        .filter(analysis_snapshots::Column::DeletedAt.is_null());

    if let Some(tid) = params.ticker_id {
        query = query.filter(analysis_snapshots::Column::TickerId.eq(tid));
    }
    if let Some(locked) = params.thesis_locked {
        query = query.filter(analysis_snapshots::Column::ThesisLocked.eq(locked));
    }

    let results = query
        .order_by_desc(analysis_snapshots::Column::CapturedAt)
        .all(&ctx.db)
        .await?;

    let summaries: Vec<SnapshotSummary> = results
        .into_iter()
        .map(|(snapshot, ticker)| SnapshotSummary::from_model_and_ticker(snapshot, ticker))
        .collect();
    format::json(summaries)
}

/// Retrieves a single snapshot with full `snapshot_data`.
///
/// **GET** `/api/v1/snapshots/:id`
#[debug_handler]
pub async fn get_snapshot(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let snapshot = analysis_snapshots::Entity::find_by_id(id)
        .filter(analysis_snapshots::Column::DeletedAt.is_null())
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    format::json(snapshot)
}

/// Soft-deletes an unlocked snapshot.
///
/// **DELETE** `/api/v1/snapshots/:id`
///
/// Locked snapshots are rejected with 403.
#[debug_handler]
pub async fn delete_snapshot(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let snapshot = analysis_snapshots::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    if snapshot.deleted_at.is_some() {
        return Err(Error::NotFound);
    }

    if snapshot.thesis_locked {
        return forbidden("Locked analyses cannot be deleted");
    }

    let mut active = snapshot.into_active_model();
    active.deleted_at = ActiveValue::set(Some(chrono::Utc::now().into()));
    active.update(&ctx.db).await?;

    format::json(serde_json::json!({ "status": "deleted" }))
}

/// Returns the raw chart image PNG for a given snapshot.
///
/// **GET** `/api/v1/snapshots/:id/chart-image`
#[debug_handler]
pub async fn get_snapshot_chart_image(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let snapshot = analysis_snapshots::Entity::find_by_id(id)
        .filter(analysis_snapshots::Column::DeletedAt.is_null())
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    match snapshot.chart_image {
        Some(bytes) => Ok(Response::builder()
            .header("Content-Type", "image/png")
            .header("Cache-Control", "public, max-age=31536000, immutable")
            .body(bytes.into())
            .map_err(|e| Error::string(&e.to_string()))?),
        None => Err(Error::NotFound),
    }
}

/// Rejects all modification attempts (append-only + immutability contract).
///
/// **PUT** `/api/v1/snapshots/:id`
#[debug_handler]
pub async fn update_snapshot(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let snapshot = analysis_snapshots::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    if snapshot.deleted_at.is_some() {
        return Err(Error::NotFound);
    }

    if snapshot.thesis_locked {
        return forbidden("Locked analyses cannot be modified");
    }

    forbidden("Snapshots are append-only and cannot be modified. Create a new snapshot instead.")
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

/// Registers snapshot routes under `/api/v1/snapshots`.
pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/v1/snapshots")
        .add("/", post(create_snapshot))
        .add("/", get(list_snapshots))
        .add("/{id}", get(get_snapshot))
        .add("/{id}/chart-image", get(get_snapshot_chart_image))
        .add("/{id}", delete(delete_snapshot))
        .add("/{id}", put(update_snapshot))
        .add("/{id}", patch(update_snapshot))
}
