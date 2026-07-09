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
    JournalState, MSG_BLANK_CURRENCY, MSG_BLANK_TICKER, MSG_NO_JOURNAL, MSG_NORMALIZE_FAILED,
    MSG_READ_ONLY_WRITE, MSG_SAVE_FAILED, empty_judgment,
};

impl JournalState {
    /// The deterministic `created_at, id`-ordered study summaries (empty on no-journal / read error).
    /// Absence-blind — a consumer that STATES absence to the user must use
    /// [`Self::try_list_studies`] instead (issue #95).
    pub fn list_studies(&self) -> Vec<StudySummary> {
        self.try_list_studies().unwrap_or_default()
    }

    /// Fallible listing (issue #95): `Ok(rows)` — possibly empty, a true absence — or `Err` on a
    /// read FAILURE, so a consumer can say « indisponible » instead of the factually wrong
    /// « aucune étude ». No journal open → `Ok(empty)` (a true absence, not a failure).
    pub fn try_list_studies(&self) -> Result<Vec<StudySummary>, String> {
        let Some(journal) = self.journal.as_ref() else {
            return Ok(Vec::new());
        };
        journal.list_studies().map_err(|error| {
            tracing::warn!("list_studies failed: {error}");
            error.to_string()
        })
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
        // Issue #34 (FR51): the creation IS the timeline's first entry — the durable history
        // starts at the all-`None` state, same transaction as the row itself.
        match journal.put_study_with_history(&study, &study.created_at) {
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
    /// read error (logged). Absence-blind — a consumer that STATES absence to the user must use
    /// [`Self::try_get_study`] instead (issue #95).
    pub fn get_study(&self, id: Uuid) -> Option<Study> {
        self.try_get_study(id).ok().flatten()
    }

    /// The study's FR51 snapshot summaries, oldest first (issue #34, PR 2) — fallible from day
    /// one (the #95 discipline): `Err` is a read FAILURE the panel states as « indisponible »,
    /// never an empty-looking timeline. No journal open → `Ok(empty)`.
    pub fn try_list_study_history(
        &self,
        study_id: Uuid,
    ) -> Result<Vec<steadyinvest_persistence::JudgmentSnapshotSummary>, String> {
        let Some(journal) = self.journal.as_ref() else {
            return Ok(Vec::new());
        };
        journal.list_judgment_snapshots(study_id).map_err(|error| {
            tracing::warn!("list_judgment_snapshots({study_id}) failed: {error}");
            error.to_string()
        })
    }

    /// One FR51 snapshot's full state (issue #34, PR 2) — same tri-state contract as
    /// [`Self::try_get_study`].
    pub fn try_get_history_snapshot(&self, id: Uuid) -> Result<Option<Study>, String> {
        let Some(journal) = self.journal.as_ref() else {
            return Ok(None);
        };
        journal.get_judgment_snapshot(id).map_err(|error| {
            tracing::warn!("get_judgment_snapshot({id}) failed: {error}");
            error.to_string()
        })
    }

    /// Fallible reopen (issue #95): `Ok(Some)` found, `Ok(None)` truly absent (also when no
    /// journal is open), `Err` on a read FAILURE (logged) — so a consumer can say
    /// « indisponible » instead of the factually wrong « n'existe pas ».
    pub fn try_get_study(&self, id: Uuid) -> Result<Option<Study>, String> {
        let Some(journal) = self.journal.as_ref() else {
            return Ok(None);
        };
        journal.get_study(id).map_err(|error| {
            tracing::warn!("get_study({id}) failed: {error}");
            error.to_string()
        })
    }
}
