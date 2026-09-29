//! The owner's decisions on AI drafts, headless (Story 8.2b — arch §Phase 4 A6, A7, A8; owner
//! decisions D3, D5, O4, O5).
//!
//! Three pure parts and one rail:
//! - [`draft_freshness`] classifies a pending draft as fresh / stale / target gone, from the study
//!   and the draft's payload (the fingerprint of `contract::draftable`, computed with this build's
//!   `METHOD_VERSION`);
//! - [`owner_edit`] reads the owner's edit of a proposal under the owner's number format and the
//!   grid's units (millions for sales / pre-tax profit, #117) — never the AI text parser;
//! - [`decided_study`] builds the study with a draft applied — a cell through
//!   `Cell::validated_from_draft` (always `?`, D5), a judgment field with its "placed by AI" mark, a
//!   note appended with its AI origin — or the owner's edited value without any AI origin (an edit
//!   equal to the proposal is a plain validation);
//! - [`JournalState::decide_draft`] sequences the guards, reads the study afresh, builds the
//!   decision and hands it to `Journal::decide_draft`, which re-checks the draft and the study and
//!   writes both in ONE transaction; a validation is recorded on the undo stack WITH its draft id,
//!   so undo / redo move the draft's status with the study (`undo.rs`).
//!
//! There is no UI here: the inbox (8.5a/8.5b) opens the draft's study first — a decision is only
//! taken on the study the undo history belongs to.

use steadyinvest_contract::{
    AiOrigin, Cell, DraftField, DraftFieldKind, DraftKind, DraftPayload, DraftTarget, DraftUnit,
    DraftValue, ForecastLowOption, Money, Note, Study, Timestamp, draft_fingerprint, option_name,
};
use steadyinvest_persistence::{
    DraftDecisionWrite, DraftStudyValidation, DraftVerdict, Error as PersistError, StudyWrite,
};
use uuid::Uuid;

use crate::viewmodel::entry;
use crate::viewmodel::format::NumberFormat;

use super::notes::normalized_note_text;
use super::{
    JournalState, MSG_DECISION_ALREADY_DECIDED, MSG_DECISION_CHANGED, MSG_DECISION_DRAFT_GONE,
    MSG_DECISION_OTHER_STUDY, MSG_DECISION_READ_ONLY, MSG_DECISION_SAVE_FAILED,
    MSG_DECISION_SAVE_FAILED_CAUSE, MSG_DECISION_STUDY_ARCHIVED, MSG_DECISION_STUDY_CHANGED,
    MSG_DECISION_STUDY_GONE, MSG_DECISION_TARGET_GONE, MSG_DRAFT_STUDY_EXISTS,
    MSG_GONE_REASON_FIELD, MSG_GONE_REASON_STUDY, MSG_GONE_REASON_YEAR, MSG_NO_JOURNAL,
    MSG_NO_STUDY_OPEN, MSG_NOTE_EMPTY, MSG_READ_FAILED, MSG_VALUE_NOT_A_NUMBER,
    MSG_VALUE_NOT_AN_OPTION, persist_cause, typed_entry,
};

/// Why a pending draft's target is gone (it can then only be rejected).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoneReason {
    /// The draft's fiscal year is no longer a year of the study.
    YearRemoved(i32),
    /// The draft's study was deleted.
    StudyDeleted,
    /// The draft names no draftable field of its kind (a malformed import; MCP checks, 8.3). The
    /// key is kept for the log, never shown (G3 F4).
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
            GoneReason::FieldUnknown(_) => MSG_GONE_REASON_FIELD.to_string(),
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

/// The owner's edit of a proposal, already read by [`owner_edit`] (G3 E1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditedValue {
    /// A number in the field's STORED unit (a millions field already scaled back to absolute).
    Number(Money),
    /// A forecast-low option.
    Option(ForecastLowOption),
    /// A note text, normalized.
    Note(String),
}

/// The owner's decision on a draft (Story 8.2b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Accept the draft as proposed. `seen_fingerprint` is the fingerprint the owner saw when
    /// confirming a stale draft (`None` = the draft was shown fresh, i.e. its base fingerprint).
    Validate { seen_fingerprint: Option<String> },
    /// Accept after editing the proposal: saved as the owner's own entry, without AI origin —
    /// unless the edit equals the proposal, which is then a plain validation (G3 E2).
    ValidateEdited {
        seen_fingerprint: Option<String>,
        value: EditedValue,
    },
    /// Refuse the draft; nothing but the draft row is written.
    Reject,
}

/// Which draft a decision is about, as the inbox shows it: its id, its study (`None` for a draft
/// study) and its security — so a refusal can name the study even when the draft or the study is
/// gone (G3 E4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftRef {
    pub draft_id: Uuid,
    pub study_id: Option<Uuid>,
    pub ticker: String,
}

/// Read the owner's edit of a draft's proposal (G3 E1): a note text normalized (empty →
/// [`MSG_NOTE_EMPTY`]); an option by its name (else [`MSG_VALUE_NOT_AN_OPTION`]); a number under
/// the owner's `format` (ambiguous / not a number → the owner-entry refusals), in the grid's units —
/// sales and pre-tax profit are entered in millions and stored absolute (#117). A blank number is
/// refused: emptying a proposal is a rejection, not a validation.
pub fn owner_edit(
    kind: DraftKind,
    payload: &DraftPayload,
    text: &str,
    format: NumberFormat,
) -> Result<EditedValue, String> {
    if kind == DraftKind::Note {
        return normalized_note_text(text)
            .map(EditedValue::Note)
            .ok_or_else(|| MSG_NOTE_EMPTY.to_string());
    }
    let field = payload
        .target
        .as_ref()
        .and_then(DraftField::of_target)
        .map(|(f, _)| f)
        .ok_or_else(|| MSG_DECISION_SAVE_FAILED.to_string())?;
    if field.unit() == DraftUnit::Option {
        let t = text.trim();
        return field
            .options()
            .iter()
            .find(|o| option_name(**o) == t)
            .map(|o| EditedValue::Option(*o))
            .ok_or_else(|| MSG_VALUE_NOT_AN_OPTION.to_string());
    }
    let typed = typed_entry(text, format)?.ok_or_else(|| MSG_VALUE_NOT_A_NUMBER.to_string())?;
    Ok(EditedValue::Number(if field.unit() == DraftUnit::Amount {
        entry::entered_to_stored(typed, entry::FIELD_SALES)
    } else {
        typed
    }))
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

/// The fingerprint of a cell / judgment draft's target as the owner sees it NOW (Story 8.5b): taken
/// when the decision dialog opens, and passed back as `Decision::Validate { seen_fingerprint }` when
/// the owner confirms a stale draft — so a target that moves again between the look and the click
/// is refused (arch A7, O4). `None` for a note / study draft or a target that is gone.
pub fn seen_fingerprint(study: &Study, payload: &DraftPayload) -> Option<String> {
    payload
        .target
        .as_ref()
        .and_then(|target| current_fingerprint(study, target).ok())
}

/// Whether a cell / judgment draft's target differs from its base (G3 B3/E6): stale when the base is
/// absent (it cannot prove freshness), the target is gone, or today's fingerprint differs.
fn is_stale(study: &Study, payload: &DraftPayload, target: &DraftTarget) -> bool {
    match (
        current_fingerprint(study, target),
        payload.base_fingerprint.as_deref(),
    ) {
        (Ok(now), Some(base)) => now != base,
        _ => true,
    }
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
        Ok(_) if !is_stale(study, payload, target) => DraftFreshness::Fresh,
        Ok(_) => DraftFreshness::Stale,
    }
}

/// Why [`decided_study`] could not build the decided study.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildProblem {
    /// The target is gone.
    TargetGone(GoneReason),
    /// The target changed since the owner saw it. `confirmed`: the owner had confirmed a stale
    /// draft (the §3.3 « encore changé » refusal); else the draft was shown fresh (G3 F8).
    Changed { confirmed: bool },
    /// The stored proposal, or the edit, does not fit the draft's field (a malformed row, or an
    /// edit of another shape — internal).
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
            BuildProblem::Changed { confirmed: true } => MSG_DECISION_CHANGED.to_string(),
            BuildProblem::Changed { confirmed: false } => MSG_DECISION_STUDY_CHANGED.to_string(),
            BuildProblem::BadProposal => {
                tracing::warn!("draft {draft_id}: the proposal or the edit does not fit its field");
                MSG_DECISION_SAVE_FAILED.to_string()
            }
        }
    }
}

/// What the rail injects into [`decided_study`]: the AI origin a value validated as proposed
/// carries, the id of a note to append, the owner-entry provenance of a written cell, and the
/// clock's now.
#[derive(Debug, Clone)]
pub struct DecisionContext {
    pub origin: AiOrigin,
    pub new_note_id: Uuid,
    pub manual: steadyinvest_contract::Provenance,
    pub now: Timestamp,
}

/// A decided study: the study with the draft applied, whether the target had changed since the
/// draft was submitted (`None` for a note draft), and whether the owner's edit differed from the
/// proposal (then no AI origin is recorded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decided {
    pub study: Study,
    pub stale_at_decision: Option<bool>,
    pub edited: bool,
}

/// The study with a validated draft applied (pure). A rejection returns the study unchanged.
pub fn decided_study(
    before: &Study,
    kind: DraftKind,
    payload: &DraftPayload,
    decision: &Decision,
    ctx: DecisionContext,
) -> Result<Decided, BuildProblem> {
    let (seen, edit) = match decision {
        Decision::Validate { seen_fingerprint } => (seen_fingerprint.clone(), None),
        Decision::ValidateEdited {
            seen_fingerprint,
            value,
        } => (seen_fingerprint.clone(), Some(value)),
        Decision::Reject => {
            return Ok(Decided {
                study: before.clone(),
                stale_at_decision: None,
                edited: false,
            });
        }
    };
    let DecisionContext {
        origin,
        new_note_id,
        manual,
        now,
    } = ctx;
    let mut after = before.clone();
    if kind == DraftKind::Note {
        let proposed = payload
            .note_text
            .as_deref()
            .and_then(normalized_note_text)
            .ok_or(BuildProblem::BadProposal)?;
        let (text, edited) = match edit {
            None => (proposed, false),
            Some(EditedValue::Note(t)) => {
                let edited = *t != proposed;
                (t.clone(), edited)
            }
            Some(_) => return Err(BuildProblem::BadProposal),
        };
        after.notes.push(Note {
            id: new_note_id,
            text,
            created_at: now.clone(),
            updated_at: now,
            ai_origin: (!edited).then_some(origin),
        });
        return Ok(Decided {
            study: after,
            stale_at_decision: None,
            edited,
        });
    }
    let target = payload.target.as_ref().ok_or(BuildProblem::BadProposal)?;
    let current = current_fingerprint(before, target).map_err(BuildProblem::TargetGone)?;
    let confirmed = seen.is_some();
    let seen = seen.or_else(|| payload.base_fingerprint.clone());
    if seen.as_deref() != Some(current.as_str()) {
        return Err(BuildProblem::Changed { confirmed });
    }
    let stale = is_stale(before, payload, target);
    let (field, year) = DraftField::of_target(target).ok_or(BuildProblem::BadProposal)?;
    let proposed = field
        .parse_value(payload.proposed_value.as_deref().unwrap_or_default())
        .map_err(|_| BuildProblem::BadProposal)?;
    let value = match edit {
        None => proposed,
        Some(EditedValue::Number(m)) => DraftValue::Number(*m),
        Some(EditedValue::Option(o)) => DraftValue::Option(*o),
        Some(EditedValue::Note(_)) => return Err(BuildProblem::BadProposal),
    };
    // G3 E2: an edit equal to the proposal (value equality — `3` is `3.0`) is a plain validation.
    let edited = value != proposed;
    let origin = (!edited).then_some(origin);
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
    Ok(Decided {
        study: after,
        stale_at_decision: Some(stale),
        edited,
    })
}

/// A persistence failure of a decision write, named in French (the English detail is logged).
fn decision_save_error(error: PersistError) -> String {
    match error {
        PersistError::DraftNotPending { .. } => MSG_DECISION_ALREADY_DECIDED.to_string(),
        PersistError::StudyChangedSinceRead => MSG_DECISION_STUDY_CHANGED.to_string(),
        PersistError::DraftNotFound { .. } => MSG_DECISION_DRAFT_GONE.to_string(),
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
    /// Decide an AI draft (Story 8.2b, arch A8). The draft's study must be the study the undo
    /// history belongs to — the open study (the inbox opens a draft's study first, 8.5b); a draft
    /// **study** (no study yet) can only be rejected here — its validation creates a study (8.7).
    ///
    /// Guards, each a named French refusal with nothing written: read-only dossier · no journal ·
    /// draft unreadable / gone (its study gone → the study is named) · already decided · no study
    /// open / another study open · study gone / archived · target gone (validation) · changed since
    /// the owner saw it. A validation is then written with its study in one transaction and
    /// recorded on the undo stack with its draft id.
    pub fn decide_draft(&mut self, draft: &DraftRef, decision: Decision) -> Result<(), String> {
        if self.read_only.is_some() {
            return Err(MSG_DECISION_READ_ONLY.to_string());
        }
        let Some(journal) = self.journal.as_ref() else {
            return Err(MSG_NO_JOURNAL.to_string());
        };
        let ticker = draft.ticker.as_str();
        let study_gone = || MSG_DECISION_STUDY_GONE.replace("{ticker}", ticker);
        let record = match journal.get_draft(draft.draft_id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                // A draft leaves the dossier with its study (O7) — name the study if it is gone.
                return Err(match draft.study_id.map(|id| self.try_get_study(id)) {
                    Some(Ok(None)) => study_gone(),
                    _ => MSG_DECISION_DRAFT_GONE.to_string(),
                });
            }
            Err(error) => {
                tracing::warn!("draft read failed: {error}");
                return Err(MSG_READ_FAILED.to_string());
            }
        };
        if record.status != steadyinvest_contract::DraftStatus::Pending {
            return Err(MSG_DECISION_ALREADY_DECIDED.to_string());
        }
        if record.study_id != draft.study_id {
            // G3 F1/F2: the caller's reference disagrees with the stored draft — internal.
            tracing::warn!(
                "draft {}: the caller named study {:?}, the draft is about {:?}",
                draft.draft_id,
                draft.study_id,
                record.study_id
            );
            return Err(MSG_DECISION_SAVE_FAILED.to_string());
        }
        let payload: DraftPayload = serde_json::from_str(&record.payload).map_err(|error| {
            tracing::warn!("draft payload unreadable after a checked read: {error}");
            MSG_READ_FAILED.to_string()
        })?;
        let now = self.clock.now();
        let Some(study_id) = record.study_id else {
            // A draft study.
            if decision != Decision::Reject {
                // Story 8.7 owns the creation of a study from a draft study.
                tracing::warn!("a draft study's validation reached the 8.2b rail");
                return Err(MSG_DECISION_SAVE_FAILED.to_string());
            }
            return self.write_rejection(draft.draft_id, None, &now);
        };
        match self.history.owner() {
            None => return Err(MSG_NO_STUDY_OPEN.to_string()),
            Some(open) if open != study_id => {
                return Err(MSG_DECISION_OTHER_STUDY.replace("{ticker}", ticker));
            }
            Some(_) => {}
        }
        let journal = self.journal.as_ref().expect("checked above");
        match journal.study_status(study_id) {
            Ok(Some(status)) if status == "archived" => {
                return Err(MSG_DECISION_STUDY_ARCHIVED.replace("{ticker}", ticker));
            }
            Ok(Some(_)) => {}
            Ok(None) => return Err(study_gone()),
            Err(error) => {
                tracing::warn!("study status read failed: {error}");
                return Err(MSG_READ_FAILED.to_string());
            }
        }
        let before = match self.try_get_study(study_id) {
            Ok(Some(study)) => study,
            Ok(None) => return Err(study_gone()),
            Err(_) => return Err(MSG_READ_FAILED.to_string()),
        };
        if decision == Decision::Reject {
            let stale = payload
                .target
                .as_ref()
                .map(|target| is_stale(&before, &payload, target));
            return self.write_rejection(draft.draft_id, stale, &now);
        }
        let ctx = DecisionContext {
            origin: AiOrigin {
                draft_id: draft.draft_id,
                client: record.origin_client.clone(),
                model: record.origin_model.clone(),
                validated_at: now.clone(),
            },
            new_note_id: self.idgen.new_id(),
            manual: self.manual_provenance(),
            now: now.clone(),
        };
        let decided = decided_study(&before, record.kind, &payload, &decision, ctx)
            .map_err(|problem| problem.refusal(draft.draft_id))?;
        let result = {
            let journal = self.journal.as_mut().expect("checked above");
            journal.decide_draft(DraftDecisionWrite {
                draft_id: draft.draft_id,
                verdict: DraftVerdict::Validated,
                decided_at: &now,
                stale_at_decision: decided.stale_at_decision,
                edited_before_validation: Some(decided.edited),
                study: Some(StudyWrite {
                    expected_before: &before,
                    after: &decided.study,
                    now: &now,
                }),
            })
        };
        result.map_err(decision_save_error)?;
        self.history.record_draft(before, draft.draft_id);
        Ok(())
    }

    /// Write a rejection: only the draft row (its outcome and whether its target had changed).
    /// Validate a pending draft study (Story 8.7, arch A8, FR70): the owner confirmed the prefilled
    /// create form with `ticker` / `currency` / `company_name` (as the form holds them — possibly
    /// edited). Builds the new, empty study through the ordinary create path's builder and writes
    /// it with the draft's `validated` + `created_study_id` in ONE transaction (the duplicate check
    /// runs again inside it). Not on any undo stack: the owner reverses it by deleting the study
    /// (O7). No provider call (FR76). Returns the new study's id.
    pub fn validate_draft_study(
        &mut self,
        draft_id: Uuid,
        ticker: &str,
        currency: &str,
        company_name: &str,
    ) -> Result<Uuid, String> {
        if self.read_only.is_some() {
            return Err(MSG_DECISION_READ_ONLY.to_string());
        }
        let Some(journal) = self.journal.as_ref() else {
            return Err(MSG_NO_JOURNAL.to_string());
        };
        let record = match journal.get_draft(draft_id) {
            Ok(Some(record)) => record,
            Ok(None) => return Err(MSG_DECISION_DRAFT_GONE.to_string()),
            Err(error) => {
                tracing::warn!("draft read failed: {error}");
                return Err(MSG_READ_FAILED.to_string());
            }
        };
        if record.status != steadyinvest_contract::DraftStatus::Pending {
            return Err(MSG_DECISION_ALREADY_DECIDED.to_string());
        }
        if record.kind != DraftKind::Study {
            tracing::warn!(
                "draft {draft_id}: a draft-study validation of a {:?} draft",
                record.kind
            );
            return Err(MSG_DECISION_SAVE_FAILED.to_string());
        }
        let proposed_name = serde_json::from_str::<DraftPayload>(&record.payload)
            .ok()
            .and_then(|p| p.company_name)
            .unwrap_or_default();
        let study = self.new_study_from_form(ticker, currency, company_name)?;
        let edited = !study
            .security_ticker
            .eq_ignore_ascii_case(record.security_ticker.trim())
            || record
                .native_currency
                .as_deref()
                .is_none_or(|c| !study.native_currency.eq_ignore_ascii_case(c.trim()))
            || study.company_name.as_deref().unwrap_or("") != proposed_name.trim();
        let now = self.clock.now();
        let journal = self
            .journal
            .as_mut()
            .ok_or_else(|| MSG_NO_JOURNAL.to_string())?;
        match journal.validate_draft_study(DraftStudyValidation {
            draft_id,
            study: &study,
            edited,
            now: &now,
        }) {
            Ok(()) => Ok(study.id),
            Err(PersistError::DraftStudyExists { study_id, .. }) => {
                // Named in the EXISTING study's spelling, read back (never the error's text).
                match self.try_get_study(study_id) {
                    Ok(Some(existing)) => Err(MSG_DRAFT_STUDY_EXISTS
                        .replace("{ticker}", existing.security_ticker.trim())
                        .replace("{currency}", existing.native_currency.trim())),
                    _ => Err(MSG_DRAFT_STUDY_EXISTS
                        .replace("{ticker}", study.security_ticker.trim())
                        .replace("{currency}", study.native_currency.trim())),
                }
            }
            Err(error) => Err(decision_save_error(error)),
        }
    }

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

/// Everything the decision dialog needs about one pending draft (Story 8.5b), read afresh when the
/// dialog opens — never the inbox row's cached text.
#[derive(Debug, Clone)]
pub struct DialogDraft {
    /// The reference a decision is taken on (its study named even if the study goes).
    pub draft: DraftRef,
    pub record: steadyinvest_persistence::DraftRecord,
    pub payload: DraftPayload,
    /// The draft's study as read now (`None`: a draft study, or its study was deleted).
    pub study: Option<Study>,
    /// The draft's study is archived (its decisions are refused — the row is not activatable).
    pub archived: bool,
    pub freshness: DraftFreshness,
    /// The target cell carries the `✓` tag (Q5: the dialog's initial focus is then « Annuler »).
    pub target_validated: bool,
    /// The target's fingerprint now — what a stale confirmation passes back ([`seen_fingerprint`]).
    pub seen_fingerprint: Option<String>,
}

impl JournalState {
    /// The study the undo history belongs to — the only study a draft decision is taken on
    /// (8.2b); the decision dialog opens the draft's study first when it is another one (8.5b).
    pub fn decision_study(&self) -> Option<Uuid> {
        self.history.owner()
    }

    /// The refusals of a decision that need no open study (Story 8.5b G3): the dossier read-only,
    /// the draft gone or already decided, its study gone or archived, the target gone, or — for a
    /// validation — the target changed since the fingerprint the owner saw. Run BEFORE the dialog
    /// opens the draft's study, so a refused decision never switches the open study for nothing.
    /// The rail re-checks all of it in its transaction.
    pub fn precheck_decision(&self, draft: &DraftRef, decision: &Decision) -> Result<(), String> {
        if self.read_only.is_some() {
            return Err(MSG_DECISION_READ_ONLY.to_string());
        }
        let shown = self.draft_for_dialog(draft.draft_id)?;
        let ticker = draft.ticker.as_str();
        let Some(study_id) = shown.record.study_id else {
            return Ok(());
        };
        if shown.archived {
            return Err(MSG_DECISION_STUDY_ARCHIVED.replace("{ticker}", ticker));
        }
        let Some(before) = shown.study.as_ref() else {
            return Err(MSG_DECISION_STUDY_GONE.replace("{ticker}", ticker));
        };
        debug_assert_eq!(before.id, study_id);
        if *decision == Decision::Reject {
            return Ok(());
        }
        // The pure build decides target-gone / changed exactly as the rail will; its context is
        // a placeholder (nothing is written here).
        let ctx = DecisionContext {
            origin: AiOrigin {
                draft_id: draft.draft_id,
                client: String::new(),
                model: String::new(),
                validated_at: self.clock.now(),
            },
            new_note_id: Uuid::nil(),
            manual: self.manual_provenance(),
            now: self.clock.now(),
        };
        decided_study(before, shown.record.kind, &shown.payload, decision, ctx)
            .map(|_| ())
            .map_err(|problem| problem.refusal(draft.draft_id))
    }

    /// Whether a decided draft was recorded as edited before its validation (Story 8.5b: the
    /// outcome notice « … (modifiée avant validation) »). A read failure reads `false` — the plain
    /// outcome, never a wrong claim of an edit.
    pub fn draft_was_edited(&self, draft_id: Uuid) -> bool {
        self.journal
            .as_ref()
            .and_then(|j| j.get_draft(draft_id).ok().flatten())
            .is_some_and(|r| r.edited_before_validation == Some(true))
    }

    /// Read one pending draft for the decision dialog (Story 8.5b): the draft record, its payload,
    /// its study (and whether it is archived), its freshness, whether its target cell is `✓`, and the
    /// fingerprint the owner is about to see. Every failure is named — a draft gone or already
    /// decided, a read failure — never a guessed dialog.
    pub fn draft_for_dialog(&self, draft_id: Uuid) -> Result<DialogDraft, String> {
        let Some(journal) = self.journal.as_ref() else {
            return Err(MSG_NO_JOURNAL.to_string());
        };
        let record = match journal.get_draft(draft_id) {
            Ok(Some(record)) => record,
            Ok(None) => return Err(MSG_DECISION_DRAFT_GONE.to_string()),
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
        let (study, archived) = match record.study_id {
            None => (None, false),
            Some(id) => {
                let study = self.try_get_study(id)?;
                let archived = match journal.study_status(id) {
                    Ok(status) => status.as_deref() == Some("archived"),
                    Err(error) => {
                        tracing::warn!("study status read failed: {error}");
                        return Err(MSG_READ_FAILED.to_string());
                    }
                };
                (study, archived)
            }
        };
        let freshness = draft_freshness(study.as_ref(), record.kind, &payload);
        let target_validated = match (study.as_ref(), payload.target.as_ref()) {
            (Some(s), Some(target)) => DraftField::of_target(target)
                .filter(|(f, _)| f.kind() == DraftFieldKind::Cell)
                .and_then(|(f, year)| {
                    s.years
                        .iter()
                        .find(|y| Some(y.year) == year)
                        .and_then(|row| f.cell_in(row))
                        .map(|c| c.review == steadyinvest_contract::Review::Validated)
                })
                .unwrap_or(false),
            _ => false,
        };
        let seen = study.as_ref().and_then(|s| seen_fingerprint(s, &payload));
        let ticker = match study.as_ref() {
            Some(s) => s.security_ticker.clone(),
            None => {
                let t = record.security_ticker.trim().to_ascii_uppercase();
                if steadyinvest_persistence::is_ticker(&t) {
                    t
                } else {
                    "—".to_string()
                }
            }
        };
        Ok(DialogDraft {
            draft: DraftRef {
                draft_id,
                study_id: record.study_id,
                ticker,
            },
            record,
            payload,
            study,
            archived,
            freshness,
            target_validated,
            seen_fingerprint: seen,
        })
    }
}

/// A draft-inbox read that failed (Story 8.5a): the French message (logged, for tests) and its
/// named cause, for the « indisponible » band — never an empty-looking inbox (arch A9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxReadError {
    pub message: String,
    /// `None` when no cause can be named (the band then says « indisponible » without one).
    pub cause: Option<&'static str>,
}

fn inbox_read_error(error: PersistError) -> InboxReadError {
    let cause = persist_cause(&error);
    InboxReadError {
        message: super::read_failure(super::MSG_SUBJECT_DRAFTS, error),
        cause,
    }
}

/// What the inbox shows, read in one go: the PENDING drafts, today's study of each that has one — a
/// study absent from the map was deleted — and the studies archived today (G3).
#[derive(Debug, Clone)]
pub struct InboxData {
    pub drafts: Vec<steadyinvest_persistence::DraftRecord>,
    pub studies: std::collections::HashMap<Uuid, Study>,
    pub archived: std::collections::HashSet<Uuid>,
}

impl JournalState {
    /// `PRAGMA data_version` of the open dossier (Story 8.5a, arch A9): `Ok(None)` when no dossier
    /// is open (a true absence), `Err` on a read failure (logged, cause named).
    ///
    /// `log`: the poller asks every 2.5 s — it logs a failure once, when it starts (`log = true`),
    /// then quietly while it lasts.
    pub fn try_data_version(&self, log: bool) -> Result<Option<i64>, InboxReadError> {
        let Some(journal) = self.journal.as_ref() else {
            return Ok(None);
        };
        journal.data_version().map(Some).map_err(|error| {
            if log {
                inbox_read_error(error)
            } else {
                InboxReadError {
                    cause: persist_cause(&error),
                    message: String::new(),
                }
            }
        })
    }

    /// Read the inbox (Story 8.5a): the drafts and the studies their pending ones target, through
    /// the app's own journal — never `McpAccess`. `Ok(None)` when no dossier is open. ANY failure
    /// — the drafts or one study — fails the whole read (no partial list with guessed values).
    pub fn read_inbox(&self) -> Result<Option<InboxData>, InboxReadError> {
        let Some(journal) = self.journal.as_ref() else {
            return Ok(None);
        };
        // Pending only: the decided record is the Registre's (8.7), never read on every poll (G3).
        let drafts = journal.list_pending_drafts().map_err(inbox_read_error)?;
        self.with_studies(drafts).map(Some)
    }

    /// Read the drafts record (Story 8.7, FR77): EVERY draft of the dossier, pending included, and
    /// the studies they target or created — read when the « Registre » is shown, never on every
    /// poll. Same rules as [`Self::read_inbox`] (one failure fails the whole read).
    pub fn read_record(&self) -> Result<Option<InboxData>, InboxReadError> {
        let Some(journal) = self.journal.as_ref() else {
            return Ok(None);
        };
        let drafts = journal.list_drafts().map_err(inbox_read_error)?;
        self.with_studies(drafts).map(Some)
    }

    /// The studies `drafts` are about (`study_id` / `created_study_id`) and which are archived.
    fn with_studies(
        &self,
        drafts: Vec<steadyinvest_persistence::DraftRecord>,
    ) -> Result<InboxData, InboxReadError> {
        let journal = self.journal.as_ref().expect("checked by the callers");
        let mut studies = std::collections::HashMap::new();
        let mut archived = std::collections::HashSet::new();
        for id in drafts
            .iter()
            .flat_map(|d| [d.study_id, d.created_study_id])
            .flatten()
        {
            if studies.contains_key(&id) {
                continue;
            }
            if let Some(study) = journal.get_study(id).map_err(inbox_read_error)? {
                if journal
                    .study_status(id)
                    .map_err(inbox_read_error)?
                    .as_deref()
                    == Some("archived")
                {
                    archived.insert(id);
                }
                studies.insert(id, study);
            }
        }
        Ok(InboxData {
            drafts,
            studies,
            archived,
        })
    }
}
