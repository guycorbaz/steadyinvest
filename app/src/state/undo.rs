//! In-memory undo/redo for the open study (Story 2.9, FR32): a stack of whole [`Study`] snapshots,
//! **not** a diff log — the journal is the source of truth and state is small, so a snapshot IS a
//! `Study` clone. [`UndoHistory`] holds the two stacks (capped, reset on open, never persisted);
//! the [`JournalState`] undo/redo rail writes the restored snapshot back through the guarded
//! `put_study` path, so a step is itself reversible and the history is never silently lost.
//!
//! **Draft decisions (Story 8.2b, arch A8).** A step may carry the id of the AI draft whose
//! validation it recorded: undoing it writes the prior study AND moves the draft to
//! `validated_undone` in one transaction; redoing it writes the decided study and moves the draft
//! back to `validated`. The history also knows which study it belongs to (its **owner**, set when a
//! study is opened) — a decision is only taken on that study, so its undo lands in the right stack.

use steadyinvest_contract::Study;
use steadyinvest_persistence::DraftStep;
use uuid::Uuid;

use super::{
    JournalState, MSG_NO_JOURNAL, MSG_SAVE_FAILED, MSG_UNDO_DRAFT_STEP_DROPPED, save_error,
};

/// The maximum number of undo steps kept in memory (oldest dropped past this). `Study` clones are
/// small but not free; a long session does not grow the history unboundedly (Story 2.9).
const UNDO_CAP: usize = 100;

/// Which way [`JournalState::step`] moves through the history.
#[derive(Clone, Copy)]
enum Direction {
    Undo,
    Redo,
}

/// In-memory undo/redo history for the open study (Story 2.9) — a stack of whole [`Study`] snapshots,
/// **NOT a diff log**. This realizes the architecture's "snapshot stack, simple clones because state
/// is small" directly over the persisted `Study` blob: the app keeps no separate in-memory domain
/// state (the journal is the source of truth), so a snapshot IS a `Study` clone. Per open study,
/// reset on open, never persisted across reopen.
#[derive(Default)]
pub struct UndoHistory {
    /// States as they were BEFORE each mutation (most recent on top).
    undo: Vec<UndoStep>,
    /// States displaced by an undo, available to redo (most recent on top).
    redo: Vec<UndoStep>,
    /// The study this history belongs to — the open study (Story 8.2b); `None` for the demo, after
    /// a dossier switch, and before any study is opened.
    owner: Option<Uuid>,
}

/// One undo/redo entry: a whole-study snapshot, and — when the step recorded an AI draft's
/// validation — that draft's id, so stepping over it moves the draft's status with the study.
struct UndoStep {
    study: Study,
    draft: Option<Uuid>,
}

impl UndoHistory {
    /// Record the pre-mutation snapshot and invalidate the redo branch (a new edit forks history).
    pub(crate) fn record(&mut self, before: Study) {
        self.push_step(UndoStep {
            study: before,
            draft: None,
        });
    }

    /// Record the pre-decision snapshot of an AI draft's validation (Story 8.2b): undoing this step
    /// moves the draft to `validated_undone`. Invalidates redo like any new edit — a draft already
    /// undone stays `validated_undone`.
    pub(crate) fn record_draft(&mut self, before: Study, draft_id: Uuid) {
        self.push_step(UndoStep {
            study: before,
            draft: Some(draft_id),
        });
    }

    /// Push a step — unless the history has an owner study and the step belongs to another one
    /// (Story 8.2b G3 B1: a late fetch result for a study that is no longer open writes that study
    /// through the ordinary rails; its snapshot must never land in the open study's history, where
    /// an undo would write it back as the open study's state). Such a step is dropped and logged;
    /// the redo branch is left as it is (the open study was not edited).
    fn push_step(&mut self, step: UndoStep) {
        if let Some(owner) = self.owner
            && step.study.id != owner
        {
            tracing::warn!(
                "an undo step of study {} was not recorded in the history of the open study {owner}",
                step.study.id
            );
            return;
        }
        self.undo.push(step);
        if self.undo.len() > UNDO_CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn reset(&mut self, owner: Option<Uuid>) {
        self.undo.clear();
        self.redo.clear();
        self.owner = owner;
    }

    /// The study this history belongs to (see [`UndoHistory::owner`]).
    pub(crate) fn owner(&self) -> Option<Uuid> {
        self.owner
    }

    fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

impl JournalState {
    // ── Undo/redo (Story 2.9) ──

    /// Clear the undo/redo history with no owner study (the demo, a dossier switch, a deletion).
    pub fn reset_undo(&mut self) {
        self.history.reset(None);
    }

    /// Clear the undo/redo history for a newly opened study, which becomes its owner — the only
    /// study a draft decision may be taken on (Story 8.2b, arch A8).
    pub fn reset_undo_for(&mut self, study_id: Uuid) {
        self.history.reset(Some(study_id));
    }

    /// Whether an undo / redo step is available (the UI disables its control when not).
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// The number of recorded undo steps — test-only, to prove an idempotent mutation records no
    /// phantom step (Story 3.3 AC1: a no-op refresh must not push undo state).
    #[cfg(test)]
    pub fn undo_depth(&self) -> usize {
        self.history.undo.len()
    }

    /// Step the open study **back** to the snapshot before the last mutation (FR32). Returns
    /// `Ok(true)` when a step was taken (the caller re-reads + re-renders), `Ok(false)` when the
    /// undo stack is empty. The restore is a real, guarded `put_study` of the whole prior `Study`.
    pub fn undo(&mut self, study_id: Uuid) -> Result<bool, String> {
        self.step(study_id, Direction::Undo)
    }

    /// Step the open study **forward** to a snapshot displaced by a prior undo (no-op if the redo
    /// stack is empty).
    pub fn redo(&mut self, study_id: Uuid) -> Result<bool, String> {
        self.step(study_id, Direction::Redo)
    }

    /// The shared undo/redo engine: pop the target snapshot, write it back, and move the present
    /// state onto the opposite stack so the step is itself reversible. On a write failure the popped
    /// snapshot is pushed back (the history is never silently lost) and a neutral notice surfaces.
    fn step(&mut self, study_id: Uuid, dir: Direction) -> Result<bool, String> {
        self.refuse_if_read_only()?;
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let popped = match dir {
            Direction::Undo => self.history.undo.pop(),
            Direction::Redo => self.history.redo.pop(),
        };
        let Some(UndoStep {
            study: restored,
            draft,
        }) = popped
        else {
            return Ok(false); // nothing to step to
        };
        let push_back = |history: &mut UndoHistory, study: Study| {
            let step = UndoStep { study, draft };
            match dir {
                Direction::Undo => history.undo.push(step),
                Direction::Redo => history.redo.push(step),
            }
        };
        if restored.id != study_id {
            // G3 F5: a step is only ever written back into its own study.
            tracing::warn!(
                "an undo step of study {} was asked for study {study_id}",
                restored.id
            );
            push_back(&mut self.history, restored);
            return Err(MSG_SAVE_FAILED.to_string());
        }
        let Some(current) = self.get_study(study_id) else {
            push_back(&mut self.history, restored);
            return Err(MSG_SAVE_FAILED.to_string());
        };
        // Issue #34 (FR51): a step back/forward is a real state change — it lands in the durable
        // history honestly (the cadrage decision: no special case for undo in v1). A step over a
        // draft validation (Story 8.2b) moves the draft's status in the SAME transaction.
        let now = self.clock.now();
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            match draft {
                None => journal.put_study_with_history(&restored, &now),
                Some(draft_id) => {
                    let step = match dir {
                        Direction::Undo => DraftStep::Undo,
                        Direction::Redo => DraftStep::Redo,
                    };
                    journal.step_draft_decision(&restored, draft_id, step, &now)
                }
            }
        };
        match result {
            Ok(()) => {
                // The present state becomes reversible on the opposite stack — carrying the same
                // draft id, so redo re-validates the draft the undo set aside.
                let step = UndoStep {
                    study: current,
                    draft,
                };
                match dir {
                    Direction::Undo => self.history.redo.push(step),
                    Direction::Redo => self.history.undo.push(step),
                }
                Ok(true)
            }
            // G3 B2/E7: a draft step whose draft is no longer in the state the step expects (decided
            // or removed elsewhere) can never succeed — pushing it back would wedge the history on
            // it. It is dropped and named; nothing was written.
            Err(
                error @ (steadyinvest_persistence::Error::DraftStatusMismatch { .. }
                | steadyinvest_persistence::Error::DraftNotFound { .. }),
            ) => {
                tracing::warn!("an undo step over a draft was dropped: {error}");
                Err(MSG_UNDO_DRAFT_STEP_DROPPED.to_string())
            }
            Err(error) => {
                push_back(&mut self.history, restored);
                Err(save_error(error))
            }
        }
    }
}
