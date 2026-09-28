//! AI drafts — the `ai_drafts` table (migration v8, Story 8.2a; arch §Phase 4 A4).
//!
//! A draft is an AI client's proposal (a new study, a note, a cell or judgment value), inserted by
//! the MCP access surface (Story 8.3) and decided by the owner (Story 8.2b). [`Journal::list_drafts`]
//! / [`Journal::get_draft`] return rows, typed, for the export and the inbox / record views; the
//! owner's decisions are written by [`Journal::decide_draft`] and their undo/redo by
//! [`Journal::step_draft_decision`] — each ONE transaction with the study write and its FR51
//! snapshot (NFR-R2). There is no insert here on purpose — inserts are `McpAccess`'s (Story 8.3).
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
use crate::studies::{stored_study_is, study_status_in, write_study_with_snapshot};
use crate::util::{bump_logical_version, parse_uuid};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use steadyinvest_contract::{
    DRAFT_PAYLOAD_VERSION, DraftKind, DraftPayload, DraftStatus, Study, Timestamp,
};
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

/// The owner's decision on a pending draft (Story 8.2b) — only these two; `validated_undone` is
/// reached by an undo step ([`DraftStep`]), never decided directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftVerdict {
    /// The owner accepted the draft (as proposed, or edited before validation).
    Validated,
    /// The owner refused the draft; nothing else is written.
    Rejected,
}

/// An undo/redo step over a validated draft (Story 8.2b, FR32 / FR77).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftStep {
    /// `validated → validated_undone` (the study goes back to its pre-decision snapshot).
    Undo,
    /// `validated_undone → validated` (the decided study is written again).
    Redo,
}

/// The study side of a validation: the study as the decision read it (`expected_before`) and as it
/// is to be written (`after`), with the app's clock for its FR51 snapshot.
#[derive(Debug, Clone, Copy)]
pub struct StudyWrite<'a> {
    /// The study the decision was computed from — the stored one must still be it.
    pub expected_before: &'a Study,
    /// The study with the draft applied.
    pub after: &'a Study,
    /// The injected clock's now, for the FR51 snapshot row.
    pub now: &'a Timestamp,
}

/// One decision write (Story 8.2b Dev Notes §5). The facts obey the 8.2a CHECKs by construction of
/// the caller: `stale_at_decision` is `None` for note / study drafts, `edited_before_validation` is
/// `Some` only for a validation, and a rejection carries no study write.
#[derive(Debug, Clone, Copy)]
pub struct DraftDecisionWrite<'a> {
    pub draft_id: Uuid,
    pub verdict: DraftVerdict,
    pub decided_at: &'a Timestamp,
    pub stale_at_decision: Option<bool>,
    pub edited_before_validation: Option<bool>,
    /// `Some` for a validation that changes a study; `None` for a rejection.
    pub study: Option<StudyWrite<'a>>,
}

pub(crate) type DraftRow = (
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

pub(crate) fn row_tuple(r: &rusqlite::Row<'_>) -> rusqlite::Result<DraftRow> {
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

/// Why a stored payload is not a valid draft payload. [`PayloadProblem::Newer`] is data from a
/// newer build (named as such by the read and the import); every other case is corrupt / malformed.
/// The Display is the English technical detail of the typed persistence error.
#[derive(Debug)]
pub(crate) enum PayloadProblem {
    /// The payload's `version` is above [`DRAFT_PAYLOAD_VERSION`]: a newer build wrote it.
    Newer { version: u64 },
    /// The JSON does not parse as a [`DraftPayload`] (or carries no usable `version`).
    Unparsable(String),
    /// It parses but carries another version or the shape of another kind.
    Misfit { version: u32, kind: DraftKind },
}

impl std::fmt::Display for PayloadProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayloadProblem::Newer { version } => write!(
                f,
                "payload version {version} is newer than this build's {DRAFT_PAYLOAD_VERSION}"
            ),
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
/// payload rule shared by the read and the import. The `version` is read first, so a newer
/// build's payload (whose shape this build cannot know) is named newer, never corrupt.
pub(crate) fn check_payload(
    payload: &str,
    kind: DraftKind,
) -> std::result::Result<(), PayloadProblem> {
    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|e| PayloadProblem::Unparsable(e.to_string()))?;
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| PayloadProblem::Unparsable("no numeric version".to_string()))?;
    if version > u64::from(DRAFT_PAYLOAD_VERSION) {
        return Err(PayloadProblem::Newer { version });
    }
    let parsed: DraftPayload =
        serde_json::from_value(value).map_err(|e| PayloadProblem::Unparsable(e.to_string()))?;
    if !parsed.fits(kind) {
        return Err(PayloadProblem::Misfit {
            version: parsed.version,
            kind,
        });
    }
    Ok(())
}

/// A currency code as the app writes it (the 8.0 spec's `identifier_invalid` rule): three ASCII
/// upper-case letters (ISO 4217 form).
pub(crate) fn is_currency_code(code: &str) -> bool {
    code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase())
}

/// A timestamp as the app writes it: RFC3339 UTC, `AAAA-MM-JJTHH:MM:SS[.fraction]Z` — a shape
/// check (this crate has no date library; the app's clock is the only writer).
pub(crate) fn is_rfc3339_utc(text: &str) -> bool {
    let b = text.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    if b.len() < 20 || b[b.len() - 1] != b'Z' {
        return false;
    }
    let shape = digits(0..4)
        && b[4] == b'-'
        && digits(5..7)
        && b[7] == b'-'
        && digits(8..10)
        && b[10] == b'T'
        && digits(11..13)
        && b[13] == b':'
        && digits(14..16)
        && b[16] == b':'
        && digits(17..19);
    let rest = &b[19..b.len() - 1];
    let fraction_ok = rest.is_empty()
        || (rest[0] == b'.' && rest.len() > 1 && rest[1..].iter().all(u8::is_ascii_digit));
    shape && fraction_ok
}

pub(crate) fn record_from_row(row: DraftRow) -> Result<DraftRecord> {
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
    check_payload(&payload, kind).map_err(|e| match e {
        PayloadProblem::Newer { version } => Error::NewerRowSchema {
            row_schema_version: i64::try_from(version).unwrap_or(i64::MAX),
            supported: DRAFT_PAYLOAD_VERSION,
        },
        other => corrupt(format!("ai_drafts.payload of draft {id}: {other}")),
    })?;
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

    /// One draft by id, or `None` when the dossier holds no such draft — the same corruption rule
    /// as [`Self::list_drafts`] (a corrupt row is an error, never `None`).
    pub fn get_draft(&self, id: Uuid) -> Result<Option<DraftRecord>> {
        read_draft_in(&self.conn, id)
    }

    /// A study's lifecycle status (`"active"` / `"archived"`), or `None` when the study is gone —
    /// the decision rail refuses a draft of an archived study by name (Story 8.2b).
    pub fn study_status(&self, id: Uuid) -> Result<Option<String>> {
        study_status_in(&self.conn, id)
    }

    /// Write the owner's decision on a pending draft (Story 8.2b, arch A8, NFR-R2): in **one**
    /// transaction, re-check that the draft is still `pending` ([`Error::DraftNotPending`]) and, for
    /// a validation, that the stored study is still the one the decision was computed from
    /// ([`Error::StudyChangedSinceRead`]); then write the study (upsert + FR51 snapshot) and the
    /// draft's `status` / `decided_at` / decision facts, and bump the logical version **once** (the
    /// `ai_drafts` trigger fires on INSERT only). Any failure rolls the whole transaction back.
    pub fn decide_draft(&mut self, d: DraftDecisionWrite<'_>) -> Result<()> {
        self.check_writable()?;
        if let Some(w) = &d.study {
            self.check_study_identity(w.after)?;
        }
        let tx = self.conn.transaction()?;
        let (status, draft_study) = draft_status_in(&tx, d.draft_id)?;
        if let Some(w) = &d.study
            && Some(w.after.id) != draft_study
        {
            return Err(Error::DraftStudyMismatch {
                study_id: w.after.id,
            });
        }
        if status != DraftStatus::Pending {
            return Err(Error::DraftNotPending {
                status: status.as_str().to_string(),
            });
        }
        if let Some(w) = &d.study {
            if !stored_study_is(&tx, w.expected_before)? {
                return Err(Error::StudyChangedSinceRead);
            }
            let payload = serde_json::to_string(w.after)?;
            write_study_with_snapshot(&tx, w.after, &payload, w.now)?;
        }
        let to = match d.verdict {
            DraftVerdict::Validated => DraftStatus::Validated,
            DraftVerdict::Rejected => DraftStatus::Rejected,
        };
        let updated = tx.execute(
            "UPDATE ai_drafts
                 SET status = ?1, decided_at = ?2, stale_at_decision = ?3,
                     edited_before_validation = ?4
               WHERE id = ?5 AND status = 'pending'",
            rusqlite::params![
                to.as_str(),
                d.decided_at.0,
                d.stale_at_decision.map(i64::from),
                d.edited_before_validation.map(i64::from),
                d.draft_id.to_string()
            ],
        )?;
        if updated != 1 {
            return Err(Error::DraftNotPending {
                status: status.as_str().to_string(),
            });
        }
        bump_logical_version(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// Undo or redo a validated draft (Story 8.2b, FR32 / FR77): in **one** transaction, check the
    /// draft is in the step's `from` status ([`Error::DraftStatusMismatch`]), write `study` (upsert +
    /// FR51 snapshot) and move the draft to the step's `to` status with `decided_at = now` (the time
    /// of the latest transition — the validation time lives in the study's `AiOrigin`); the decision
    /// facts are kept as decided. One logical-version bump.
    pub fn step_draft_decision(
        &mut self,
        study: &Study,
        draft_id: Uuid,
        step: DraftStep,
        now: &Timestamp,
    ) -> Result<()> {
        self.check_writable()?;
        self.check_study_identity(study)?;
        let (from, to) = match step {
            DraftStep::Undo => (DraftStatus::Validated, DraftStatus::ValidatedUndone),
            DraftStep::Redo => (DraftStatus::ValidatedUndone, DraftStatus::Validated),
        };
        let tx = self.conn.transaction()?;
        let (found, draft_study) = draft_status_in(&tx, draft_id)?;
        if draft_study != Some(study.id) {
            return Err(Error::DraftStudyMismatch { study_id: study.id });
        }
        if found != from {
            return Err(Error::DraftStatusMismatch {
                expected: from.as_str().to_string(),
                found: found.as_str().to_string(),
            });
        }
        let payload = serde_json::to_string(study)?;
        write_study_with_snapshot(&tx, study, &payload, now)?;
        tx.execute(
            "UPDATE ai_drafts SET status = ?1, decided_at = ?2 WHERE id = ?3 AND status = ?4",
            rusqlite::params![to.as_str(), now.0, draft_id.to_string(), from.as_str()],
        )?;
        bump_logical_version(&tx)?;
        tx.commit()?;
        Ok(())
    }
}

/// The stored status and study of a draft inside a decision transaction ([`Error::DraftNotFound`]
/// when the dossier holds no such draft; an unknown spelling is corrupt, never guessed).
fn draft_status_in(
    tx: &rusqlite::Transaction<'_>,
    id: Uuid,
) -> Result<(DraftStatus, Option<Uuid>)> {
    let row: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT status, study_id FROM ai_drafts WHERE id = ?1",
            rusqlite::params![id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (status, study) = row.ok_or(Error::DraftNotFound { id })?;
    let status = status
        .parse()
        .map_err(|e| corrupt(format!("ai_drafts.status of draft {id}: {e}")))?;
    let study = study
        .as_deref()
        .map(|s| parse_uuid(s, "ai_drafts.study_id"))
        .transpose()?;
    Ok((status, study))
}

/// One draft by id on a given connection (the journal's, or the MCP access surface's — Story 8.4
/// G3 point lookup): a corrupt row is an error, never `None`.
pub(crate) fn read_draft_in(conn: &rusqlite::Connection, id: Uuid) -> Result<Option<DraftRecord>> {
    use rusqlite::OptionalExtension;
    let row = conn
        .query_row(
            &format!("SELECT {DRAFT_COLUMNS} FROM ai_drafts WHERE id = ?1"),
            rusqlite::params![id.to_string()],
            row_tuple,
        )
        .optional()?;
    row.map(record_from_row).transpose()
}
