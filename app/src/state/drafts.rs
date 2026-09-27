//! The owner's decisions on AI drafts, headless (Story 8.2b — arch §Phase 4 A6, A7, A8; owner
//! decisions D3, D5, O4, O5).
//!
//! Two pure parts and one rail:
//! - [`draft_freshness`] classifies a pending draft as fresh / stale / target gone, from the study
//!   and the draft's payload (the fingerprint of `contract::draftable`, computed with this build's
//!   `METHOD_VERSION`);
//! - [`decided_study`] builds the study with a draft applied — a cell through
//!   `Cell::validated_from_draft` (always `?`, D5), a judgment field with its "placed by AI" mark, a
//!   note appended with its AI origin — or the owner's edited value without any AI origin;
//! - [`JournalState::decide_draft`] sequences the guards, reads the study afresh, builds the
//!   decision and hands it to `Journal::decide_draft`, which re-checks the draft and the study and
//!   writes both in ONE transaction; a validation is recorded on the undo stack WITH its draft id,
//!   so undo / redo move the draft's status with the study (`undo.rs`).
//!
//! There is no UI here: the inbox (8.5a/8.5b) opens the draft's study first — a decision is only
//! taken on the study the undo history belongs to.

use steadyinvest_contract::{
    AiOrigin, Cell, DraftField, DraftFieldKind, DraftKind, DraftPayload, DraftTarget, DraftValue,
    Note, Study, Timestamp, draft_fingerprint,
};
use steadyinvest_persistence::{
    DraftDecisionWrite, DraftVerdict, Error as PersistError, StudyWrite,
};
use uuid::Uuid;

use crate::viewmodel::entry;

use super::notes::normalized_note_text;
use super::{
    JournalState, MSG_DECISION_ALREADY_DECIDED, MSG_DECISION_CHANGED, MSG_DECISION_READ_ONLY,
    MSG_DECISION_SAVE_FAILED, MSG_DECISION_SAVE_FAILED_CAUSE, MSG_DECISION_STUDY_ARCHIVED,
    MSG_DECISION_STUDY_GONE, MSG_DECISION_TARGET_GONE, MSG_GONE_REASON_FIELD,
    MSG_GONE_REASON_STUDY, MSG_GONE_REASON_YEAR, MSG_NO_JOURNAL, MSG_NO_STUDY_OPEN, MSG_NOTE_EMPTY,
    MSG_READ_FAILED, MSG_STUDY_GONE, MSG_VALUE_NOT_A_NUMBER, persist_cause,
};

/// Why a pending draft's target is gone (it can then only be rejected).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoneReason {
    /// The draft's fiscal year is no longer a year of the study.
    YearRemoved(i32),
    /// The draft's study was deleted.
    StudyDeleted,
    /// The draft names no draftable field of its kind (a malformed import; MCP checks, 8.3).
    FieldUnknown(String),
}

impl GoneReason {
    /// The French reason fragment of [`MSG_DECISION_TARGET_GONE`].
    pub fn text(&self) -> String {
        match self {
            GoneReason::YearRemoved(year) => {
                MSG_GONE_REASON_YEAR.replace("{year}", &year.to_string())
            }
            GoneReason::StudyDeleted => MSG_GONE_REASON_STUDY.to_string(),
            GoneReason::FieldUnknown(field) => MSG_GONE_REASON_FIELD.replace("{field}", field),
        }
    }
}

/// The state of a pending draft against today's study (arch A7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftFreshness {
    /// The target is as it was when the draft was submitted.
    Fresh,
    /// The target changed since submission (owner edit, refresh, method change) — still
    /// validatable after an explicit confirmation (O4).
    Stale,
    /// The target no longer exists — rejection only.
    TargetGone(GoneReason),
}

/// The owner's decision on a draft (Story 8.2b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Accept the draft as proposed. `seen_fingerprint` is the fingerprint the owner saw when
    /// confirming a stale draft (`None` = the draft was shown fresh, i.e. its base fingerprint).
    Validate { seen_fingerprint: Option<String> },
    /// Accept after editing the proposed value (or note text): saved as the owner's own entry,
    /// without AI origin.
    ValidateEdited {
        seen_fingerprint: Option<String>,
        value: String,
    },
    /// Refuse the draft; nothing but the draft row is written.
    Reject,
}

/// Today's fingerprint of a cell / judgment draft's target, or the reason it is gone.
fn current_fingerprint(study: &Study, target: &DraftTarget) -> Result<String, GoneReason> {
    let Some((_, year)) = DraftField::of_target(target) else {
        let field = match target {
            DraftTarget::Cell { field, .. } | DraftTarget::Judgment { field } => field.clone(),
        };
        return Err(GoneReason::FieldUnknown(field));
    };
    draft_fingerprint(study, target, steadyinvest_core::METHOD_VERSION)
        .ok_or_else(|| GoneReason::YearRemoved(year.unwrap_or_default()))
}

/// Classify a pending draft (pure — arch A7). `study` is the draft's study as read today (`None`
/// when it was deleted). Study drafts are always fresh (no target); note drafts are fresh unless
/// their study is gone; a cell / judgment draft is stale when today's fingerprint differs from the
/// one it was submitted against — an absent base fingerprint cannot prove freshness, so it reads
/// stale.
pub fn draft_freshness(
    study: Option<&Study>,
    kind: DraftKind,
    payload: &DraftPayload,
) -> DraftFreshness {
    if kind == DraftKind::Study {
        return DraftFreshness::Fresh;
    }
    let Some(study) = study else {
        return DraftFreshness::TargetGone(GoneReason::StudyDeleted);
    };
    let Some(target) = payload.target.as_ref() else {
        return DraftFreshness::Fresh; // a note draft
    };
    match current_fingerprint(study, target) {
        Err(why) => DraftFreshness::TargetGone(why),
        Ok(now) if payload.base_fingerprint.as_deref() == Some(now.as_str()) => {
            DraftFreshness::Fresh
        }
        Ok(_) => DraftFreshness::Stale,
    }
}

/// Why [`decided_study`] could not build the decided study.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildProblem {
    /// The target is gone.
    TargetGone(GoneReason),
    /// The target changed since the fingerprint the owner saw.
    Changed,
    /// The owner's edited value does not read as a value for the field.
    NotAValue,
    /// The edited note text is empty.
    EmptyNote,
    /// The stored proposal does not read as a value of its field (a malformed row).
    BadProposal,
}

impl BuildProblem {
    /// The French refusal of a decision that could not be built (the detail of a malformed row is
    /// logged, never shown).
    pub fn refusal(self, draft_id: Uuid) -> String {
        match self {
            BuildProblem::TargetGone(gone) => {
                MSG_DECISION_TARGET_GONE.replace("{reason}", &gone.text())
            }
            BuildProblem::Changed => MSG_DECISION_CHANGED.to_string(),
            BuildProblem::NotAValue => MSG_VALUE_NOT_A_NUMBER.to_string(),
            BuildProblem::EmptyNote => MSG_NOTE_EMPTY.to_string(),
            BuildProblem::BadProposal => {
                tracing::warn!("draft {draft_id}: the stored proposal does not read as a value");
                MSG_DECISION_SAVE_FAILED.to_string()
            }
        }
    }
}

/// What the rail injects into [`decided_study`]: the AI origin (`Some` for a validation as
/// proposed, `None` for an edited one), the id of a note to append, the owner-entry provenance of
/// a written cell, and the clock's now.
#[derive(Debug, Clone)]
pub struct DecisionContext {
    pub origin: Option<AiOrigin>,
    pub new_note_id: Uuid,
    pub manual: steadyinvest_contract::Provenance,
    pub now: Timestamp,
}

/// The study with a validated draft applied, and whether the decision was taken on a stale target
/// (pure). Returns the decided study and `stale_at_decision` (`None` for a note draft).
pub fn decided_study(
    before: &Study,
    kind: DraftKind,
    payload: &DraftPayload,
    decision: &Decision,
    ctx: DecisionContext,
) -> Result<(Study, Option<bool>), BuildProblem> {
    let DecisionContext {
        origin,
        new_note_id,
        manual,
        now,
    } = ctx;
    let now = &now;
    let (seen, edited) = match decision {
        Decision::Validate { seen_fingerprint } => (seen_fingerprint.clone(), None),
        Decision::ValidateEdited {
            seen_fingerprint,
            value,
        } => (seen_fingerprint.clone(), Some(value.as_str())),
        Decision::Reject => return Ok((before.clone(), None)),
    };
    let mut after = before.clone();
    if kind == DraftKind::Note {
        let text = match edited {
            Some(v) => normalized_note_text(v).ok_or(BuildProblem::EmptyNote)?,
            None => payload
                .note_text
                .as_deref()
                .and_then(normalized_note_text)
                .ok_or(BuildProblem::BadProposal)?,
        };
        after.notes.push(Note {
            id: new_note_id,
            text,
            created_at: now.clone(),
            updated_at: now.clone(),
            ai_origin: origin,
        });
        return Ok((after, None));
    }
    let target = payload.target.as_ref().ok_or(BuildProblem::BadProposal)?;
    let current = current_fingerprint(before, target).map_err(BuildProblem::TargetGone)?;
    let seen = seen.or_else(|| payload.base_fingerprint.clone());
    if seen.as_deref() != Some(current.as_str()) {
        return Err(BuildProblem::Changed);
    }
    let stale = payload.base_fingerprint.as_deref() != Some(current.as_str());
    let (field, year) = DraftField::of_target(target).ok_or(BuildProblem::BadProposal)?;
    let value = match edited {
        Some(v) => field.parse_value(v).map_err(|_| BuildProblem::NotAValue)?,
        None => field
            .parse_value(payload.proposed_value.as_deref().unwrap_or_default())
            .map_err(|_| BuildProblem::BadProposal)?,
    };
    match field.kind() {
        DraftFieldKind::Cell => {
            let DraftValue::Number(money) = value else {
                return Err(BuildProblem::BadProposal);
            };
            let row = after
                .years
                .iter_mut()
                .find(|y| Some(y.year) == year)
                .ok_or(BuildProblem::TargetGone(GoneReason::YearRemoved(
                    year.unwrap_or_default(),
                )))?;
            let base: Cell = field
                .cell_in(row)
                .cloned()
                .unwrap_or_else(|| entry::tofill_cell(manual.clone()));
            let provenance = steadyinvest_contract::Provenance {
                ai_origin: origin,
                ..manual
            };
            field.put_cell(row, base.validated_from_draft(Some(money), provenance));
        }
        DraftFieldKind::Judgment => {
            if !field.set_judgment(&mut after.judgment, value) {
                return Err(BuildProblem::BadProposal);
            }
            if let Some(slot) = field.ai_slot_mut(&mut after.judgment.ai_placed) {
                *slot = origin;
            }
        }
    }
    Ok((after, Some(stale)))
}

/// A persistence failure of a decision write, named in French (the English detail is logged).
fn decision_save_error(error: PersistError) -> String {
    match error {
        PersistError::DraftNotPending { .. } => MSG_DECISION_ALREADY_DECIDED.to_string(),
        PersistError::StudyChangedSinceRead => MSG_DECISION_CHANGED.to_string(),
        PersistError::DraftNotFound { .. } => MSG_STUDY_GONE.to_string(),
        other => {
            tracing::warn!("draft decision write failed: {other}");
            match persist_cause(&other) {
                Some(cause) => MSG_DECISION_SAVE_FAILED_CAUSE.replace("{cause}", cause),
                None => MSG_DECISION_SAVE_FAILED.to_string(),
            }
        }
    }
}

impl JournalState {
    /// Decide an AI draft (Story 8.2b, arch A8). `study_id` must be the study the undo history
    /// belongs to — the open study (the inbox opens a draft's study first, 8.5b); a draft **study**
    /// (no study yet) can only be rejected here — its validation creates a study (Story 8.7).
    ///
    /// Guards, each a named French refusal with nothing written: read-only dossier · no journal ·
    /// draft unreadable / gone · already decided · not the open study · study gone / archived ·
    /// target gone (validation) · changed since the owner's confirmation. A validation is then
    /// written with its study in one transaction and recorded on the undo stack with its draft id.
    pub fn decide_draft(
        &mut self,
        study_id: Uuid,
        draft_id: Uuid,
        decision: Decision,
    ) -> Result<(), String> {
        if self.read_only.is_some() {
            return Err(MSG_DECISION_READ_ONLY.to_string());
        }
        let Some(journal) = self.journal.as_ref() else {
            return Err(MSG_NO_JOURNAL.to_string());
        };
        let record = match journal.get_draft(draft_id) {
            Ok(Some(record)) => record,
            // A draft only leaves the dossier with its study (the O7 cascade).
            Ok(None) => return Err(MSG_STUDY_GONE.to_string()),
            Err(error) => {
                tracing::warn!("draft read failed: {error}");
                return Err(MSG_READ_FAILED.to_string());
            }
        };
        if record.status != steadyinvest_contract::DraftStatus::Pending {
            return Err(MSG_DECISION_ALREADY_DECIDED.to_string());
        }
        let payload: DraftPayload = serde_json::from_str(&record.payload).map_err(|error| {
            tracing::warn!("draft payload unreadable after a checked read: {error}");
            MSG_READ_FAILED.to_string()
        })?;
        let now = self.clock.now();
        if record.kind == DraftKind::Study {
            if decision != Decision::Reject {
                // Story 8.7 owns the creation of a study from a draft study.
                tracing::warn!("a draft study's validation reached the 8.2b rail");
                return Err(MSG_DECISION_SAVE_FAILED.to_string());
            }
            return self.write_rejection(draft_id, None, &now);
        }
        if record.study_id != Some(study_id) || self.history.owner() != Some(study_id) {
            return Err(MSG_NO_STUDY_OPEN.to_string());
        }
        let ticker = &record.security_ticker;
        let journal = self.journal.as_ref().expect("checked above");
        match journal.study_status(study_id) {
            Ok(Some(status)) if status == "archived" => {
                return Err(MSG_DECISION_STUDY_ARCHIVED.replace("{ticker}", ticker));
            }
            Ok(Some(_)) => {}
            Ok(None) => return Err(MSG_DECISION_STUDY_GONE.replace("{ticker}", ticker)),
            Err(error) => {
                tracing::warn!("study status read failed: {error}");
                return Err(MSG_READ_FAILED.to_string());
            }
        }
        let before = match self.try_get_study(study_id) {
            Ok(Some(study)) => study,
            Ok(None) => return Err(MSG_DECISION_STUDY_GONE.replace("{ticker}", ticker)),
            Err(_) => return Err(MSG_READ_FAILED.to_string()),
        };
        if decision == Decision::Reject {
            let stale = payload.target.as_ref().map(|target| {
                current_fingerprint(&before, target).ok() != payload.base_fingerprint
            });
            return self.write_rejection(draft_id, stale, &now);
        }
        let origin = match &decision {
            Decision::Validate { .. } => Some(AiOrigin {
                draft_id,
                client: record.origin_client.clone(),
                model: record.origin_model.clone(),
                validated_at: now.clone(),
            }),
            _ => None,
        };
        let edited = matches!(decision, Decision::ValidateEdited { .. });
        let note_id = self.idgen.new_id();
        let manual = self.manual_provenance();
        let ctx = DecisionContext {
            origin,
            new_note_id: note_id,
            manual,
            now: now.clone(),
        };
        let (after, stale) = decided_study(&before, record.kind, &payload, &decision, ctx)
            .map_err(|problem| problem.refusal(draft_id))?;
        let result = {
            let journal = self.journal.as_mut().expect("checked above");
            journal.decide_draft(DraftDecisionWrite {
                draft_id,
                verdict: DraftVerdict::Validated,
                decided_at: &now,
                stale_at_decision: stale,
                edited_before_validation: Some(edited),
                study: Some(StudyWrite {
                    expected_before: &before,
                    after: &after,
                    now: &now,
                }),
            })
        };
        result.map_err(decision_save_error)?;
        self.history.record_draft(before, draft_id);
        Ok(())
    }

    /// Write a rejection: only the draft row (its outcome and whether its target had changed).
    fn write_rejection(
        &mut self,
        draft_id: Uuid,
        stale_at_decision: Option<bool>,
        now: &Timestamp,
    ) -> Result<(), String> {
        let journal = self
            .journal
            .as_mut()
            .ok_or_else(|| MSG_NO_JOURNAL.to_string())?;
        journal
            .decide_draft(DraftDecisionWrite {
                draft_id,
                verdict: DraftVerdict::Rejected,
                decided_at: now,
                stale_at_decision,
                edited_before_validation: None,
                study: None,
            })
            .map_err(decision_save_error)
    }
}
