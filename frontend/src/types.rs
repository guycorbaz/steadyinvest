//! Frontend-specific type definitions.
//!
//! Contains DTOs that mirror backend response shapes but are used only on the
//! client side (compiled to WASM).

use serde::{Deserialize, Serialize};
use naic_logic::AnalysisSnapshot;
use chrono::{DateTime, Utc};

/// Client-side representation of a locked analysis record from the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedAnalysisModel {
    /// Database row ID.
    pub id: i32,
    /// Foreign key to the ticker table.
    pub ticker_id: i32,
    /// Raw JSON containing the serialized [`AnalysisSnapshot`].
    pub snapshot_data: serde_json::Value,
    /// Analyst's thesis note captured at lock time.
    pub analyst_note: String,
    /// Timestamp when the analysis was locked.
    pub created_at: DateTime<Utc>,
}

impl LockedAnalysisModel {
    /// Deserializes the raw `snapshot_data` JSON into an [`AnalysisSnapshot`].
    ///
    /// # Panics
    ///
    /// Panics if `snapshot_data` contains invalid or corrupt JSON that cannot
    /// be deserialized into an [`AnalysisSnapshot`].
    pub fn snapshot(&self) -> AnalysisSnapshot {
        serde_json::from_value(self.snapshot_data.clone()).unwrap()
    }
}
