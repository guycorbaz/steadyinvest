//! Analysis persistence controller (thesis locking, export).
//!
//! Endpoints for saving analysis snapshots, listing past analyses per ticker,
//! and exporting locked analyses as PDF reports.

use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use crate::models::{_entities::analysis_snapshots, tickers};
use naic_logic::AnalysisSnapshot;
use sea_orm::QueryOrder;

/// Request body for locking (saving) an analysis snapshot.
#[derive(Debug, Deserialize, Serialize)]
pub struct LockRequest {
    /// Ticker symbol the analysis belongs to.
    pub ticker: String,
    /// The complete analysis state to persist.
    pub snapshot: AnalysisSnapshot,
    /// Analyst's thesis note (required, must not be blank).
    pub analyst_note: String,
}

/// Locks (saves) an analysis snapshot for a ticker.
///
/// **POST** `/api/analyses/lock`
///
/// # Errors
///
/// Returns an error if the analyst note is blank or the ticker is not found.
#[debug_handler]
pub async fn lock_analysis(
    State(ctx): State<AppContext>,
    Json(req): Json<LockRequest>,
) -> Result<Response> {
    if req.analyst_note.trim().is_empty() {
        return Err(Error::string("Analyst note is required (AC 2)"));
    }

    let ticker_symbol = req.ticker.to_uppercase();

    // Find the ticker
    let ticker = tickers::Entity::find()
        .filter(tickers::Column::Ticker.eq(&ticker_symbol))
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    let active = analysis_snapshots::ActiveModel {
        user_id: ActiveValue::set(1), // default single-user until Phase 3 auth
        ticker_id: ActiveValue::set(ticker.id),
        snapshot_data: ActiveValue::set(serde_json::to_value(req.snapshot).map_err(|e| Error::string(&e.to_string()))?),
        thesis_locked: ActiveValue::set(true),
        chart_image: ActiveValue::set(None), // Story 7.4 adds chart image capture
        notes: ActiveValue::set(Some(req.analyst_note)),
        captured_at: ActiveValue::set(chrono::Utc::now().into()),
        ..Default::default()
    };

    let model = active.insert(&ctx.db).await?;

    format::json(model)
}

/// Lists all locked analyses for a ticker, newest first.
///
/// **GET** `/api/analyses/{ticker}`
///
/// # Errors
///
/// Returns `404 Not Found` if the ticker does not exist in the database.
#[debug_handler]
pub async fn get_analyses(
    State(ctx): State<AppContext>,
    Path(ticker_symbol): Path<String>,
) -> Result<Response> {
    let ticker_symbol = ticker_symbol.to_uppercase();

    let ticker = tickers::Entity::find()
        .filter(tickers::Column::Ticker.eq(&ticker_symbol))
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    let analyses = analysis_snapshots::Entity::find()
        .filter(analysis_snapshots::Column::TickerId.eq(ticker.id))
        .filter(analysis_snapshots::Column::ThesisLocked.eq(true))
        .order_by_desc(analysis_snapshots::Column::CapturedAt)
        .all(&ctx.db)
        .await?;

    format::json(analyses)
}

/// Exports a locked analysis as a PDF report.
///
/// **GET** `/api/analyses/export/{id}`
///
/// Generates the PDF synchronously via [`spawn_blocking`](tokio::task::spawn_blocking)
/// and returns it with `Content-Type: application/pdf`.
///
/// # Errors
///
/// Returns `404 Not Found` if the analysis or its parent ticker is missing.
/// Returns an error if snapshot JSON deserialization or PDF generation fails.
#[debug_handler]
pub async fn export_analysis(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let (snapshot_row, ticker) = analysis_snapshots::Entity::find_by_id(id)
        .find_also_related(tickers::Entity)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    let ticker = ticker.ok_or_else(|| Error::NotFound)?;

    // Only locked analyses may be exported (immutability contract)
    if !snapshot_row.thesis_locked {
        return Err(Error::NotFound);
    }

    let snapshot: AnalysisSnapshot = serde_json::from_value(snapshot_row.snapshot_data)
        .map_err(|e| Error::string(&e.to_string()))?;

    let ticker_for_pdf = ticker.ticker.clone();
    let note_for_pdf = snapshot_row.notes.clone().unwrap_or_default();
    let captured_at = snapshot_row.captured_at;

    let pdf_bytes = tokio::task::spawn_blocking(move || {
        crate::services::reporting::ReportingService::generate_ssg_report(
            &ticker_for_pdf,
            captured_at,
            &note_for_pdf,
            &snapshot,
        )
    }).await.map_err(|e| Error::string(&format!("Blocking task error: {}", e)))?
      .map_err(|e| Error::string(&e.to_string()))?;

    Response::builder()
        .header("Content-Type", "application/pdf")
        .header("Content-Disposition", format!("attachment; filename=\"ssg_report_{}.pdf\"", ticker.ticker))
        .body(pdf_bytes.into())
        .map_err(|e| Error::string(&e.to_string()))
}

/// Registers analysis routes under `/api/analyses`.
pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/analyses")
        .add("/lock", post(lock_analysis))
        .add("/export/{id}", get(export_analysis))
        .add("/{ticker}", get(get_analyses))
}
