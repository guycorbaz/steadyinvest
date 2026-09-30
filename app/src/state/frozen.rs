//! Story 8.8 — « Valider l'étude »: freeze the study's full verdict (FR68, arch A13). An ordinary
//! owner edit: one upsert + its FR51 snapshot, on the undo stack.

use steadyinvest_contract::Timestamp;
use uuid::Uuid;

use super::{
    JournalState, MSG_FREEZE_NOT_FULL, MSG_NO_JOURNAL, MSG_READ_FAILED, MSG_SAVE_FAILED,
    MSG_STUDY_GONE,
};
use crate::viewmodel::engine;

impl JournalState {
    /// Freeze the study's verdict — only a FULL one (`report::form::freeze`, the compile-time
    /// gate). Replaces an earlier frozen verdict (the caller asked the replace confirmation; the
    /// previous stays in the FR51 history). Returns the freeze time. Refusals: read-only, no
    /// journal, the study gone or unreadable, the verdict not full (its open inputs named), a save
    /// failure.
    pub fn freeze_verdict(&mut self, study_id: Uuid) -> Result<Timestamp, String> {
        self.refuse_if_read_only()?;
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let study = match self.try_get_study(study_id) {
            Ok(Some(study)) => study,
            Ok(None) => return Err(MSG_STUDY_GONE.to_string()),
            Err(_) => return Err(MSG_READ_FAILED.to_string()),
        };
        let now = self.clock.now();
        let frozen = match steadyinvest_report::form::freeze(&study, &now) {
            Ok(frozen) => frozen,
            Err(steadyinvest_report::form::NotFrozen::NotFull) => {
                let list = steadyinvest_report::form::build_snapshot(&study)
                    .ok()
                    .and_then(|snapshot| engine::open_inputs(&snapshot))
                    .unwrap_or_default();
                return Err(MSG_FREEZE_NOT_FULL.replace("{list}", &list));
            }
            Err(steadyinvest_report::form::NotFrozen::Normalize(error)) => {
                tracing::warn!("freeze: the study does not normalize: {error}");
                return Err(MSG_SAVE_FAILED.to_string());
            }
        };
        let at = frozen.frozen_at.clone();
        self.mutate_study(study_id, move |s| s.frozen_verdict = Some(frozen))?;
        Ok(at)
    }
}
