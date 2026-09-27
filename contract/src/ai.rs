//! AI-origin types (Epic 8, Phase 4 — arch §Phase 4 A6).
//!
//! A validated AI draft is an owner entry; what it keeps of its origin is an [`AiOrigin`]. The
//! origin of a *pending* draft (`DraftOrigin`) arrives with Story 8.2b.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::provenance::Timestamp;

/// The origin of a value, judgment field or note that the owner validated from an AI draft: which
/// draft, which AI client and model proposed it, and when the owner validated it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiOrigin {
    /// The `ai_drafts` row the value was validated from.
    pub draft_id: Uuid,
    /// The AI client that submitted the draft (e.g. "claude-code").
    pub client: String,
    /// The model that produced the draft.
    pub model: String,
    /// When the owner validated the draft (RFC3339 UTC).
    pub validated_at: Timestamp,
}
