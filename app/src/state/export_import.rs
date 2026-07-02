//! Portable export/import (Stories 5.2/5.3 — FR59/FR60/NFR-R5): one study or the whole journal as
//! a JSON envelope — the serialized data contract + `schema_version` + integrity hash (NOT a raw
//! `.db`; that unit is the Story-5.4 backup). Import verifies integrity + version **before** any
//! write, preserves entity ids (a re-import updates in place, never duplicates), rebinds studies to
//! the current journal, and applies atomically (all-or-nothing); every rejection maps to a neutral
//! notice and writes nothing.

use steadyinvest_contract::ImportError;
use steadyinvest_persistence::{Error as PersistError, ImportSummary};
use uuid::Uuid;

use super::{
    JournalState, MSG_EXPORT_MISSING, MSG_IMPORT_INTEGRITY, MSG_IMPORT_MALFORMED,
    MSG_IMPORT_VERSION, MSG_NO_JOURNAL, MSG_READ_ONLY_WRITE, MSG_SAVE_FAILED,
};

impl JournalState {
    /// Export one study to its portable envelope JSON (Story 5.2, FR59) — the serialized data
    /// contract + `schema_version` + integrity hash (NOT a raw `.db`). A pure read; the caller writes
    /// the string to a user-chosen file. Guarded: no journal / missing id → a neutral notice.
    pub fn export_study(&self, id: Uuid) -> Result<String, String> {
        let study = self.get_study(id).ok_or(MSG_EXPORT_MISSING.to_string())?;
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
                return Err(MSG_READ_ONLY_WRITE.to_string())
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
        Ok(summary)
    }
}
