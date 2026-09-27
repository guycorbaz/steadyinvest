//! AI drafts — the `ai_drafts` table (migration v8, Story 8.2a; arch §Phase 4 A4).
//!
//! A draft is an AI client's proposal (a new study, a note, a cell or judgment value), inserted by
//! the MCP access surface (Story 8.3) and decided by the owner (Story 8.2b). This module is the
//! **read side** of 8.2a: [`Journal::list_drafts`] returns every row, typed, for the export and for
//! the later inbox / record views. There is no writer here on purpose — inserts are `McpAccess`'s,
//! decisions are `decide_draft`'s.
//!
//! Corruption is never skipped (review checklist §1): an unreadable id, an unknown `kind`/`status`,
//! a boolean outside 0/1 or a payload that does not parse or does not fit its kind is a
//! [`Error::CorruptPayload`] naming the column — a draft is absent or right, never silently wrong.
//!
//! **No computation reads this table** (FR72 — a pending draft changes no output): `core` and
//! `report` do not depend on this crate, and the engine view models never call [`Journal::list_drafts`]
//! (asserted by `tests/drafts.rs`).

use crate::error::{Error, Result};
use crate::journal::Journal;
use crate::util::parse_uuid;
use serde::{Deserialize, Serialize};
use steadyinvest_contract::{DraftKind, DraftPayload, DraftStatus, Timestamp};
use uuid::Uuid;

/// One `ai_drafts` row, one field per column (Story 8.2a). `payload` is the **raw** stored JSON —
/// exported byte-faithfully like a history snapshot's payload. Serde so the whole-journal export
/// carries it (the #78 additive array); no `deny_unknown_fields` (the per-entity #78 rule).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftRecord {
    pub id: Uuid,
    pub kind: DraftKind,
    /// The study a note / cell / judgment draft targets; `None` for a draft study.
    pub study_id: Option<Uuid>,
    pub security_ticker: String,
    /// The proposed currency of a draft study; `None` for every other kind.
    pub native_currency: Option<String>,
    pub status: DraftStatus,
    pub created_at: Timestamp,
    pub decided_at: Option<Timestamp>,
    pub comment: String,
    pub origin_client: String,
    pub origin_model: String,
    pub stale_at_decision: Option<bool>,
    pub edited_before_validation: Option<bool>,
    /// The study a validated draft study became (O7 cascade, history view).
    pub created_study_id: Option<Uuid>,
    /// The versioned [`DraftPayload`] JSON, as stored.
    pub payload: String,
}

type DraftRow = (
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    Option<i64>,
    Option<i64>,
    Option<String>,
    String,
);

/// The `SELECT` column list, in schema order — shared by every reader.
pub(crate) const DRAFT_COLUMNS: &str = "id, kind, study_id, security_ticker, native_currency, \
     status, created_at, decided_at, comment, origin_client, origin_model, stale_at_decision, \
     edited_before_validation, created_study_id, payload";

fn row_tuple(r: &rusqlite::Row<'_>) -> rusqlite::Result<DraftRow> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
        r.get(9)?,
        r.get(10)?,
        r.get(11)?,
        r.get(12)?,
        r.get(13)?,
        r.get(14)?,
    ))
}

fn corrupt(detail: String) -> Error {
    Error::CorruptPayload { detail }
}

fn parse_bool(value: Option<i64>, column: &str, id: &str) -> Result<Option<bool>> {
    match value {
        None => Ok(None),
        Some(0) => Ok(Some(false)),
        Some(1) => Ok(Some(true)),
        Some(other) => Err(corrupt(format!(
            "ai_drafts.{column} of draft {id} is {other}, not 0 or 1"
        ))),
    }
}

/// Why a stored payload is not a valid draft payload — the technical detail the read and the
/// import put into their typed errors ([`Error::CorruptPayload`] / [`Error::ImportMalformed`]).
#[derive(Debug)]
pub(crate) enum PayloadProblem {
    /// The JSON does not parse as a [`DraftPayload`].
    Unparsable(serde_json::Error),
    /// It parses but carries another version or the shape of another kind.
    Misfit { version: u32, kind: DraftKind },
}

impl std::fmt::Display for PayloadProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayloadProblem::Unparsable(e) => write!(f, "payload does not parse: {e}"),
            PayloadProblem::Misfit { version, kind } => write!(
                f,
                "payload (version {version}) does not fit a {} draft",
                kind.as_str()
            ),
        }
    }
}

/// Check that a stored payload parses, carries this build's version and fits its kind — the one
/// payload rule shared by the read and the import.
pub(crate) fn check_payload(
    payload: &str,
    kind: DraftKind,
) -> std::result::Result<(), PayloadProblem> {
    let parsed: DraftPayload = serde_json::from_str(payload).map_err(PayloadProblem::Unparsable)?;
    if !parsed.fits(kind) {
        return Err(PayloadProblem::Misfit {
            version: parsed.version,
            kind,
        });
    }
    Ok(())
}

fn record_from_row(row: DraftRow) -> Result<DraftRecord> {
    let (
        id,
        kind,
        study_id,
        security_ticker,
        native_currency,
        status,
        created_at,
        decided_at,
        comment,
        origin_client,
        origin_model,
        stale_at_decision,
        edited_before_validation,
        created_study_id,
        payload,
    ) = row;
    let kind: DraftKind = kind
        .parse()
        .map_err(|e| corrupt(format!("ai_drafts.kind of draft {id}: {e}")))?;
    let status: DraftStatus = status
        .parse()
        .map_err(|e| corrupt(format!("ai_drafts.status of draft {id}: {e}")))?;
    check_payload(&payload, kind)
        .map_err(|e| corrupt(format!("ai_drafts.payload of draft {id}: {e}")))?;
    Ok(DraftRecord {
        id: parse_uuid(&id, "ai_drafts.id")?,
        kind,
        study_id: study_id
            .as_deref()
            .map(|s| parse_uuid(s, "ai_drafts.study_id"))
            .transpose()?,
        security_ticker,
        native_currency,
        status,
        created_at: Timestamp(created_at),
        decided_at: decided_at.map(Timestamp),
        comment,
        origin_client,
        origin_model,
        stale_at_decision: parse_bool(stale_at_decision, "stale_at_decision", &id)?,
        edited_before_validation: parse_bool(
            edited_before_validation,
            "edited_before_validation",
            &id,
        )?,
        created_study_id: created_study_id
            .as_deref()
            .map(|s| parse_uuid(s, "ai_drafts.created_study_id"))
            .transpose()?,
        payload,
    })
}

impl Journal {
    /// Every draft in the dossier, ordered by `(created_at, id)` — deterministic, so the export's
    /// canonical serialization (and its hash) is stable. A corrupt row fails the whole read
    /// ([`Error::CorruptPayload`] naming the column) — never skipped.
    pub fn list_drafts(&self) -> Result<Vec<DraftRecord>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {DRAFT_COLUMNS} FROM ai_drafts ORDER BY created_at, id"
        ))?;
        let rows = stmt.query_map([], row_tuple)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(record_from_row(row?)?);
        }
        Ok(out)
    }
}
