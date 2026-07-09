//! Portable export/import (Stories 5.2/5.3 — FR59/FR60/NFR-R5): one study or the whole journal as
//! a JSON envelope — the serialized data contract + `schema_version` + integrity hash (NOT a raw
//! `.db`; that unit is the Story-5.4 backup). Import verifies integrity + version **before** any
//! write, preserves entity ids (a re-import updates in place, never duplicates), rebinds studies to
//! the current journal, and applies atomically (all-or-nothing); every rejection maps to a neutral
//! notice and writes nothing.

use steadyinvest_contract::ImportError;
use steadyinvest_persistence::{Error as PersistError, ImportSummary, inspect_journal_envelope};
use uuid::Uuid;

use super::{
    JournalState, MSG_EXPORT_MISSING, MSG_EXPORT_UNREADABLE, MSG_IMPORT_INTEGRITY,
    MSG_IMPORT_MALFORMED, MSG_IMPORT_VERSION, MSG_NO_JOURNAL, MSG_READ_ONLY_WRITE, MSG_SAVE_FAILED,
};

/// The outcome of [`JournalState::request_import_journal`] (issue #65): applied straight away, or
/// parked behind a confirm because the envelope is an OLDER snapshot of the SAME journal — a
/// version regression the merge would silently apply (shared entities snap back to their old
/// state) without arbitration.
#[derive(Debug)]
pub enum ImportRequest {
    /// No regression — the import was applied; here is its summary.
    Applied(ImportSummary),
    /// Same journal, older version — parked; the UI surfaces the pair and asks for a confirm.
    NeedsConfirm { source: u64, current: u64 },
}

impl JournalState {
    /// Export one study to its portable envelope JSON (Story 5.2, FR59) — the serialized data
    /// contract + `schema_version` + integrity hash (NOT a raw `.db`). A pure read; the caller writes
    /// the string to a user-chosen file. Guarded: no journal / missing id → a neutral notice.
    ///
    /// Issue #63 — reads via `journal.get_study` directly (not the state-level [`Self::get_study`],
    /// which flattens a read error to `None`) so a **present but unreadable** row (a newer
    /// `schema_version`, or a payload that fails to parse) is told apart from a truly **absent**
    /// id: the dashboard lists the row either way (it reads only indexed columns), so "introuvable"
    /// on a present row was a contradictory message.
    pub fn export_study(&self, id: Uuid) -> Result<String, String> {
        let journal = self.journal.as_ref().ok_or(MSG_NO_JOURNAL.to_string())?;
        let study = journal
            .get_study(id)
            .map_err(|_| MSG_EXPORT_UNREADABLE.to_string())?
            .ok_or(MSG_EXPORT_MISSING.to_string())?;
        Ok(steadyinvest_contract::to_export_json(&study))
    }

    /// Import a study from its portable envelope JSON (Story 5.2, FR59/NFR-R5): verify integrity +
    /// `schema_version`, then persist. The study's **own id is preserved** (a re-import of the same
    /// study updates in place, never duplicates); its `journal_id` is **rebound to this journal** so a
    /// study seeded/shared from another journal joins the current one (identity = the study id).
    /// Returns `(id, overwrote_existing)` — an import onto a pre-existing id **updates** it, which the
    /// caller surfaces distinctly (AC3); an overwrite onto an **archived** study also reactivates it,
    /// so an imported study is never silently left hidden. Each [`ImportError`] maps to a neutral
    /// notice; nothing is written on a rejection. Guarded.
    pub fn import_study(&mut self, json: &str) -> Result<(Uuid, bool), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let mut study = steadyinvest_contract::from_export_json(json).map_err(|e| match e {
            ImportError::Integrity => MSG_IMPORT_INTEGRITY.to_string(),
            ImportError::Version { .. } => MSG_IMPORT_VERSION.to_string(),
            ImportError::Malformed(_) => MSG_IMPORT_MALFORMED.to_string(),
        })?;
        let id = study.id;
        // Detect a pre-existing study with this id (and whether it is currently archived/hidden) so
        // the import is surfaced as an UPDATE, not a silent clobber (AC3 review finding).
        let existing_archived = self
            .list_studies()
            .into_iter()
            .find(|s| s.id == id)
            .map(|s| (true, s.status == "archived"));
        let overwrote = existing_archived.is_some();
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        study.journal_id = journal.id();
        match journal.put_study(&study) {
            Ok(()) => {}
            Err(PersistError::NewerJournalSchema { .. }) => {
                return Err(MSG_READ_ONLY_WRITE.to_string());
            }
            Err(error) => return Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
        // `put_study`'s upsert does not touch `status`; an imported study must be visible, so an
        // overwrite onto an archived id is reactivated.
        if existing_archived == Some((true, true)) {
            self.set_study_status(id, "active")?;
        }
        Ok((id, overwrote))
    }

    /// Export the **whole journal** to its portable envelope JSON (Story 5.3, FR60) — the serialized
    /// data contract for every entity + `schema_version` + `(journal_id, logical_version)` + integrity
    /// hash (NOT a raw `.db`). A pure read; the caller writes the string to a user-chosen file.
    /// Guarded: no journal → a neutral notice.
    pub fn export_journal(&self) -> Result<String, String> {
        let journal = self.journal.as_ref().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .export_journal()
            .map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))
    }

    /// Import a **whole journal** from its portable envelope JSON (Story 5.3, FR60/NFR-R5): verify
    /// integrity + `schema_version`, then apply **every entity atomically** (all-or-nothing, never
    /// partially). Entities are upserted by id (a re-import updates in place); studies are rebound to
    /// this journal. Returns an [`ImportSummary`] of what was applied; the caller surfaces the counts.
    /// Each rejection maps to a neutral notice (reusing the single-study integrity/version/malformed
    /// copy); nothing is written on a rejection. Guarded (read-only / no journal).
    ///
    /// Issue #65: after the merge, every holding's materialized aggregate is re-derived from its
    /// (now possibly merged) ledger — the envelope's aggregate reflects only its OWN rows, while
    /// local rows survive the upsert. Prefer [`Self::request_import_journal`], which additionally
    /// arbitrates a same-journal version REGRESSION behind a confirm; this direct rail is the
    /// shared apply tail.
    pub fn import_journal(&mut self, text: &str) -> Result<ImportSummary, String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        let summary = journal.import_journal(text).map_err(|error| match error {
            PersistError::ImportIntegrity => MSG_IMPORT_INTEGRITY.to_string(),
            PersistError::ImportVersion { .. } => MSG_IMPORT_VERSION.to_string(),
            PersistError::ImportMalformed { .. } => MSG_IMPORT_MALFORMED.to_string(),
            PersistError::NewerJournalSchema { .. } => MSG_READ_ONLY_WRITE.to_string(),
            other => format!("{MSG_SAVE_FAILED} {other}"),
        })?;
        // Issue #65 (the 6.3 sharpening): make the aggregates truthful against the merged ledger
        // NOW — not at the next ledger mutation, which would silently snap them back.
        self.rederive_position_aggregates();
        Ok(summary)
    }

    /// Request a whole-journal import (issue #65): the arbitration gate over
    /// [`Self::import_journal`]. Peeks the envelope's `(journal_id, logical_version)` (full
    /// verification — the peek refuses exactly what the import would); an envelope of the SAME
    /// journal at an OLDER version is a regression (shared entities would snap back to their old
    /// state) — it is **parked**, never applied silently, and the caller surfaces the version
    /// pair for an explicit [`Self::confirm_import_journal`]. Anything else (same journal at the
    /// same/newer version, a foreign journal — the FR60 seed case, no version axis to compare)
    /// applies straight away. Guarded (read-only / no journal).
    pub fn request_import_journal(&mut self, text: &str) -> Result<ImportRequest, String> {
        self.pending_import = None;
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let (source_id, source_version) = inspect_journal_envelope(text).map_err(|e| match e {
            ImportError::Integrity => MSG_IMPORT_INTEGRITY.to_string(),
            ImportError::Version { .. } => MSG_IMPORT_VERSION.to_string(),
            ImportError::Malformed(_) => MSG_IMPORT_MALFORMED.to_string(),
        })?;
        let journal = self.journal.as_ref().ok_or(MSG_NO_JOURNAL.to_string())?;
        let current = journal
            .logical_version()
            .map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))?;
        if source_id == journal.id() && source_version < current {
            self.pending_import = Some(text.to_string());
            return Ok(ImportRequest::NeedsConfirm {
                source: source_version,
                current,
            });
        }
        self.import_journal(text).map(ImportRequest::Applied)
    }

    /// Apply the parked older-version import (issue #65) — the explicit confirm. The parked
    /// envelope TEXT is applied as-is (it lives in memory, so no TOCTOU re-read applies); the
    /// same verify-then-atomic-merge rail and the aggregate re-derivation run unchanged. A
    /// neutral no-op error when nothing is parked.
    pub fn confirm_import_journal(&mut self) -> Result<ImportSummary, String> {
        let text = self
            .pending_import
            .take()
            .ok_or(MSG_IMPORT_MALFORMED.to_string())?;
        self.import_journal(&text)
    }

    /// Discard a parked import (issue #65) — no write.
    pub fn cancel_import_journal(&mut self) {
        self.pending_import = None;
    }
}
