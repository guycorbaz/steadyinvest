//! System health and monitoring controller.
//!
//! Provides endpoints for checking API provider health, listing audit logs,
//! and exporting audit data as CSV. All routes live under `/api/v1/system`.

use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use crate::models::audit_logs;
use crate::services::provider_health;

/// API response DTO for audit log records.
#[derive(Serialize, Deserialize)]
pub struct AuditResponse {
    pub id: i32,
    pub ticker: String,
    pub exchange: String,
    pub field_name: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub event_type: String,
    pub source: String,
    pub timestamp: String,
}

impl From<audit_logs::Model> for AuditResponse {
    fn from(m: audit_logs::Model) -> Self {
        Self {
            id: m.id,
            ticker: m.ticker,
            exchange: m.exchange,
            field_name: m.field_name,
            old_value: m.old_value,
            new_value: m.new_value,
            event_type: m.event_type,
            source: m.source,
            timestamp: m.created_at.to_string(),
        }
    }
}

/// System health and monitoring controller.
/// 
/// This controller provides endpoints for monitoring API health, 
/// listing audit logs, and exporting data for administrative review.
/// 
/// All endpoints are restricted to local subnets via security middleware.
pub async fn health(
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let health_data = provider_health::check_providers(&ctx.db).await;
    format::json(health_data)
}

/// List data integrity audit logs.
/// 
/// Returns a list of the 100 most recent anomalies and manual overrides.
pub async fn list_audit_logs(
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let logs = audit_logs::Model::find_recent(&ctx.db, 100).await?;
    let response: Vec<AuditResponse> = logs.into_iter().map(AuditResponse::from).collect();
    format::json(response)
}

/// Export audit logs as CSV.
/// 
/// Generates a standardized CSV export of the 5000 most recent audit events.
pub async fn export_audit_logs(
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let logs = audit_logs::Model::find_recent(&ctx.db, 5000).await?;
    
    let mut wtr = csv::Writer::from_writer(Vec::new());
    wtr.write_record(&["ID", "Ticker", "Exchange", "Field", "Old", "New", "Event", "Source", "Timestamp"])
        .map_err(|e| Error::BadRequest(e.to_string()))?;
        
    for log in logs {
        wtr.write_record(&[
            log.id.to_string(),
            log.ticker,
            log.exchange,
            log.field_name,
            log.old_value.unwrap_or_default(),
            log.new_value.unwrap_or_default(),
            log.event_type,
            log.source,
            log.created_at.to_string(),
        ]).map_err(|e| Error::BadRequest(e.to_string()))?;
    }
    
    let csv_data = wtr.into_inner().map_err(|e| Error::BadRequest(e.to_string()))? ;
    
    Response::builder()
        .header("Content-Type", "text/csv")
        .header("Content-Disposition", "attachment; filename=\"audit_logs.csv\"")
        .body(axum::body::Body::from(csv_data))
        .map_err(|e| Error::BadRequest(e.to_string()))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/v1/system")
        .add("/health", get(health))
        .add("/audit-logs", get(list_audit_logs))
        .add("/audit-logs/export", get(export_audit_logs))
}
