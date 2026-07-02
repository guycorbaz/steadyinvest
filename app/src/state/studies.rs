//! Study lifecycle (Stories 2.2/2.6/2.12 — FR1/FR2/FR54/FR55): create a study (all-`None`
//! judgment, id/timestamp from the injected sources), list the deterministic summaries, reopen one
//! with full state, archive/unarchive (reversible hide) and delete (irreversible, atomic with its
//! FR51 time-series) — plus [`JournalState::snapshot_for`], THE engine call site that builds one
//! coherent [`StudySnapshot`] per frame (an incoherent frame is structurally impossible).

use steadyinvest_contract::Study;
use steadyinvest_core::verdict::StudySnapshot;
use steadyinvest_persistence::{Error as PersistError, StudySummary};
use uuid::Uuid;

use crate::viewmodel::engine;

use super::{
    empty_judgment, JournalState, MSG_BLANK_CURRENCY, MSG_BLANK_TICKER, MSG_NORMALIZE_FAILED,
    MSG_NO_JOURNAL, MSG_READ_ONLY_WRITE, MSG_SAVE_FAILED,
};

impl JournalState {
    /// The deterministic `created_at, id`-ordered study summaries (empty on no-journal / read error).
    pub fn list_studies(&self) -> Vec<StudySummary> {
        let Some(journal) = self.journal.as_ref() else {
            return Vec::new();
        };
        match journal.list_studies() {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!("list_studies failed: {error}");
                Vec::new()
            }
        }
    }

    /// Archive a study (Story 2.12, FR54): flip `status` to `"archived"` so it leaves the default
    /// dashboard view. Reversible via [`Self::unarchive_study`]. Guarded (read-only / no-journal /
    /// save-failure → a neutral notice, never a silent `.ok()`). Not part of the per-open-study undo
    /// stack — it's a dashboard-lifecycle action, reversed by un-archiving, not by Ctrl+Z.
    pub fn archive_study(&mut self, study_id: Uuid) -> Result<(), String> {
        self.set_study_status(study_id, "archived")
    }

    /// Un-archive a study (Story 2.12): flip `status` back to `"active"`. The inverse of
    /// [`Self::archive_study`]; same guards.
    pub fn unarchive_study(&mut self, study_id: Uuid) -> Result<(), String> {
        self.set_study_status(study_id, "active")
    }

    /// The shared status-change rail (read-only / no-journal / save-failure guards → persist).
    pub(crate) fn set_study_status(&mut self, study_id: Uuid, status: &str) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let Some(journal) = self.journal.as_mut() else {
            return Err(MSG_NO_JOURNAL.to_string());
        };
        match journal.set_study_status(study_id, status) {
            Ok(()) => Ok(()),
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Permanently delete a study and its FR51 `judgments` time-series rows in one transaction
    /// (Story 2.12, FR55) — atomic, FK-safe, no orphan, other studies untouched. **Irreversible**
    /// (distinct from archive's reversible hide); not undoable. Clears the in-memory undo history so
    /// a later Ctrl+Z can't resurrect a pointer to a deleted study. Guarded (read-only / no-journal /
    /// save-failure → a neutral notice, never a silent `.ok()`).
    pub fn delete_study(&mut self, study_id: Uuid) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let Some(journal) = self.journal.as_mut() else {
            return Err(MSG_NO_JOURNAL.to_string());
        };
        match journal.delete_study(study_id) {
            Ok(()) => {
                // The deleted study must not linger in any undo/redo snapshot stack.
                self.reset_undo();
                Ok(())
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Validate inputs and create a study (FR1): id/journal_id/created_at from the injected
    /// sources, all-`None` judgment, `Study::new` schema-stamped, written with `put_study` (which
    /// bumps `logical_version`). Returns the new id on success, or a neutral notice for a banner.
    pub fn create_study(&mut self, ticker: &str, currency: &str) -> Result<Uuid, String> {
        let ticker = ticker.trim();
        let currency = currency.trim();
        if ticker.is_empty() {
            return Err(MSG_BLANK_TICKER.to_string());
        }
        if currency.is_empty() {
            return Err(MSG_BLANK_CURRENCY.to_string());
        }
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let Some(journal) = self.journal.as_mut() else {
            return Err(MSG_NO_JOURNAL.to_string());
        };

        let study = Study::new(
            self.idgen.new_id(),
            journal.id(),
            ticker,
            currency.to_uppercase(),
            empty_judgment(),
            self.clock.now(),
        );
        let id = study.id;
        match journal.put_study(&study) {
            Ok(()) => Ok(id),
            // The newer-schema guard can also fire here (defense in depth); name it neutrally.
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Compute the coherent [`StudySnapshot`] for a study (Story 2.6 — THE engine-call site): re-read
    /// the authoritative study, run the `contract` → `core` mapping, `normalize` (a [`NormalizeError`]
    /// surfaces as the neutral [`MSG_NORMALIZE_FAILED`], never `unwrap`/`.ok()`), then
    /// `StudySnapshot::new` ONCE — so the outputs and the verdict are always one coherent frame
    /// (architecture: "an incoherent frame is structurally impossible"). `Err` for an absent study /
    /// a normalize failure.
    pub fn snapshot_for(&self, study_id: Uuid) -> Result<StudySnapshot, String> {
        let study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        engine::build_snapshot(&study).map_err(|error| {
            tracing::warn!("normalize failed for study {study_id}: {error}");
            MSG_NORMALIZE_FAILED.to_string()
        })
    }

    /// Reopen a study by id with its **full** persisted state (FR2). `None` when absent or on a
    /// read error (logged).
    pub fn get_study(&self, id: Uuid) -> Option<Study> {
        let journal = self.journal.as_ref()?;
        match journal.get_study(id) {
            Ok(found) => found,
            Err(error) => {
                tracing::warn!("get_study({id}) failed: {error}");
                None
            }
        }
    }
}
