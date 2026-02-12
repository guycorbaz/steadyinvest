//! Analysis snapshot CRUD controller (Phase 1 API).
//!
//! Provides versioned endpoints under `/api/v1/snapshots` for creating,
//! listing, retrieving, and soft-deleting analysis snapshots.
//!
//! **Append-only model**: `POST` creates new rows; `PUT`/`PATCH` are rejected.
//! **Immutability contract**: locked snapshots reject deletion.

use loco_rs::prelude::*;
use sea_orm::{IntoActiveModel, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::models::_entities::{analysis_snapshots, tickers};

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

/// Request body for creating an analysis snapshot.
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateSnapshotRequest {
    pub ticker_id: i32,
    pub snapshot_data: serde_json::Value,
    pub thesis_locked: bool,
    pub notes: Option<String>,
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
#[derive(Debug, Serialize)]
pub struct SnapshotSummary {
    pub id: i32,
    pub ticker_id: i32,
    pub thesis_locked: bool,
    pub notes: Option<String>,
    pub captured_at: chrono::DateTime<chrono::FixedOffset>,
}

impl From<analysis_snapshots::Model> for SnapshotSummary {
    fn from(m: analysis_snapshots::Model) -> Self {
        Self {
            id: m.id,
            ticker_id: m.ticker_id,
            thesis_locked: m.thesis_locked,
            notes: m.notes,
            captured_at: m.captured_at,
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

/// Creates a new analysis snapshot (append-only).
///
/// **POST** `/api/v1/snapshots`
#[debug_handler]
pub async fn create_snapshot(
    State(ctx): State<AppContext>,
    Json(req): Json<CreateSnapshotRequest>,
) -> Result<Response> {
    // Validate that the ticker exists
    tickers::Entity::find_by_id(req.ticker_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    let active = analysis_snapshots::ActiveModel {
        user_id: ActiveValue::set(1), // default single-user until Phase 3
        ticker_id: ActiveValue::set(req.ticker_id),
        snapshot_data: ActiveValue::set(req.snapshot_data),
        thesis_locked: ActiveValue::set(req.thesis_locked),
        chart_image: ActiveValue::set(None),
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
#[debug_handler]
pub async fn list_snapshots(
    State(ctx): State<AppContext>,
    Query(params): Query<SnapshotQueryParams>,
) -> Result<Response> {
    let mut query = analysis_snapshots::Entity::find()
        .filter(analysis_snapshots::Column::DeletedAt.is_null());

    if let Some(tid) = params.ticker_id {
        query = query.filter(analysis_snapshots::Column::TickerId.eq(tid));
    }
    if let Some(locked) = params.thesis_locked {
        query = query.filter(analysis_snapshots::Column::ThesisLocked.eq(locked));
    }

    let snapshots = query
        .order_by_desc(analysis_snapshots::Column::CapturedAt)
        .all(&ctx.db)
        .await?;

    let summaries: Vec<SnapshotSummary> = snapshots.into_iter().map(Into::into).collect();
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
        .add("/{id}", delete(delete_snapshot))
        .add("/{id}", put(update_snapshot))
        .add("/{id}", patch(update_snapshot))
}
