use serde::{Deserialize, Serialize};
use naic_logic::AnalysisSnapshot;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedAnalysisModel {
    pub id: i32,
    pub ticker_id: i32,
    pub snapshot_data: serde_json::Value,
    pub analyst_note: String,
    pub created_at: DateTime<Utc>,
}

impl LockedAnalysisModel {
    pub fn snapshot(&self) -> AnalysisSnapshot {
        serde_json::from_value(self.snapshot_data.clone()).unwrap()
    }
}
