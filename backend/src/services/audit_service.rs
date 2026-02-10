//! Audit trail service.
//!
//! Provides a unified interface for recording data-integrity anomalies and
//! manual override events to the audit log. All audit records are immutable
//! once written.

use loco_rs::prelude::*;
use crate::models::audit_logs;

/// Service for recording and managing system audit events.
/// 
/// This service handles all data integrity alerts and manual user overrides,
/// ensuring a persistent audit trail for financial analysis.
pub struct AuditService;

impl AuditService {
    /// Logs a generalized audit event to the database.
    pub async fn log_event(
        db: &DatabaseConnection,
        ticker: &str,
        exchange: &str,
        field: &str,
        old: Option<String>,
        new: Option<String>,
        event_type: &str,
        source: &str,
    ) -> Result<audit_logs::Model, DbErr> {
        audit_logs::Model::create(
            db,
            ticker,
            exchange,
            field,
            old,
            new,
            event_type,
            source,
        ).await
    }

    /// Records a system-detected anomaly in financial data.
    pub async fn log_anomaly(
        db: &DatabaseConnection,
        ticker: &str,
        exchange: &str,
        field: &str,
        message: &str,
    ) -> Result<audit_logs::Model, DbErr> {
        Self::log_event(
            db,
            ticker,
            exchange,
            field,
            None,
            Some(message.to_string()),
            "Anomaly",
            "System",
        ).await
    }

    /// Records a manual data override performed by a user.
    pub async fn log_override(
        db: &DatabaseConnection,
        ticker: &str,
        exchange: &str,
        field: &str,
        old: Option<String>,
        new: Option<String>,
    ) -> Result<audit_logs::Model, DbErr> {
        Self::log_event(
            db,
            ticker,
            exchange,
            field,
            old,
            new,
            "Override",
            "User",
        ).await
    }
}
