//! In-memory undo/redo for the open study (Story 2.9, FR32): a stack of whole [`Study`] snapshots,
//! **not** a diff log — the journal is the source of truth and state is small, so a snapshot IS a
//! `Study` clone. [`UndoHistory`] holds the two stacks (capped, reset on open, never persisted);
//! the [`JournalState`] undo/redo rail writes the restored snapshot back through the guarded
//! `put_study` path, so a step is itself reversible and the history is never silently lost.

use steadyinvest_contract::Study;
use steadyinvest_persistence::Error as PersistError;
use uuid::Uuid;

use super::{JournalState, MSG_NO_JOURNAL, MSG_READ_ONLY_WRITE, MSG_SAVE_FAILED};

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
    undo: Vec<Study>,
    /// States displaced by an undo, available to redo (most recent on top).
    redo: Vec<Study>,
}

impl UndoHistory {
    /// Record the pre-mutation snapshot and invalidate the redo branch (a new edit forks history).
    pub(crate) fn record(&mut self, before: Study) {
        self.undo.push(before);
        if self.undo.len() > UNDO_CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn reset(&mut self) {
        self.undo.clear();
        self.redo.clear();
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

    /// Clear the undo/redo history — a different study was opened, so its edit history starts empty.
    pub fn reset_undo(&mut self) {
        self.history.reset();
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
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let popped = match dir {
            Direction::Undo => self.history.undo.pop(),
            Direction::Redo => self.history.redo.pop(),
        };
        let Some(restored) = popped else {
            return Ok(false); // nothing to step to
        };
        let push_back = |history: &mut UndoHistory, study: Study| match dir {
            Direction::Undo => history.undo.push(study),
            Direction::Redo => history.redo.push(study),
        };
        let Some(current) = self.get_study(study_id) else {
            push_back(&mut self.history, restored);
            return Err(MSG_SAVE_FAILED.to_string());
        };
        // Issue #34 (FR51): a step back/forward is a real state change — it lands in the durable
        // history honestly (the cadrage decision: no special case for undo in v1).
        let now = self.clock.now();
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study_with_history(&restored, &now)
        };
        match result {
            Ok(()) => {
                // The present state becomes reversible on the opposite stack.
                match dir {
                    Direction::Undo => self.history.redo.push(current),
                    Direction::Redo => self.history.undo.push(current),
                }
                Ok(true)
            }
            Err(PersistError::NewerJournalSchema { .. }) => {
                push_back(&mut self.history, restored);
                Err(MSG_READ_ONLY_WRITE.to_string())
            }
            Err(error) => {
                push_back(&mut self.history, restored);
                Err(format!("{MSG_SAVE_FAILED} {error}"))
            }
        }
    }
}
