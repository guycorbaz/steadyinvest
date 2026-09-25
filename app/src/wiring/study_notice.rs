//! The OPEN STUDY's notice slot (G1 J, #237) — `Studies.study-notice`, shown pinned under the
//! verdict bar of the study screen. Before it, every notice meant for an open study landed in
//! `Studies.notice`, which only the study LIST shows: a fetch result, the soft-lock notice, an undo
//! or drag refusal were all invisible while the study was open, and read stale on the list later.
//! The two slots are now separate: `Studies.notice` stays the list's (create/archive/export/import
//! outcomes, startup states), this one belongs to the study on screen and is emptied whenever a
//! study (or the demo) opens and when the dossier changes.
//!
//! The notice-slot rule (F4, docs/review-checklist.md §3) lives HERE for this slot: each notice
//! carries its SOURCE (the gesture family that wrote it). A success/info OUTCOME replaces only an
//! empty slot or a notice of its own source (the in-progress banner of the same fetch, the same
//! gesture family's previous word) — never a sibling's failure. A success CLEARS only its own
//! source's notice. A failure is always shown (it answers the gesture the user just made).

use std::cell::Cell;

use slint::{ComponentHandle, SharedString};

use crate::{MainWindow, Studies};

/// The gesture family that wrote the notice on show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    /// The provider fetch of the open study (its in-progress banner and its result).
    Fetch,
    /// An edit of the open study: a cell, a review tag, a judgment, a rationale, undo/redo, the
    /// soft-lock, the unlock, the forecast-low option, the traceability read.
    Edit,
    /// What the study says of itself when it is rendered or opened (a normalize failure, the
    /// examen-rapide « étude créée » outcome).
    Render,
}

thread_local! {
    // The UI is single-threaded (every callback runs on the event loop) — the `dialog` precedent.
    static SHOWN: Cell<Option<Source>> = const { Cell::new(None) };
}

/// May an OUTCOME from `source` take the slot, given what it shows (`shown`, `None` when empty)?
/// Pure — the F4 decision, unit-tested.
fn outcome_may_replace(shown: Option<Source>, source: Source) -> bool {
    shown.is_none() || shown == Some(source)
}

/// Does a success from `source` empty the slot? Only when the slot shows `source`'s own notice.
fn success_clears(shown: Option<Source>, source: Source) -> bool {
    shown == Some(source)
}

fn write(ui: &MainWindow, source: Option<Source>, text: &str) {
    SHOWN.with(|s| s.set(source));
    ui.global::<Studies>()
        .set_study_notice(SharedString::from(text));
}

/// A failure (or an in-progress banner) answering the gesture just made — always shown.
pub(crate) fn fail(ui: &MainWindow, source: Source, text: &str) {
    write(ui, Some(source), text);
}

/// A success/info outcome — shown under the F4 rule (never over a sibling's failure).
pub(crate) fn outcome(ui: &MainWindow, source: Source, text: &str) {
    if outcome_may_replace(SHOWN.with(Cell::get), source) {
        write(ui, Some(source), text);
    }
}

/// A success of `source`: its own earlier notice (a refusal it just overcame) goes; a sibling's stays.
pub(crate) fn clear(ui: &MainWindow, source: Source) {
    if success_clears(SHOWN.with(Cell::get), source) {
        write(ui, None, "");
    }
}

/// Empty the slot whatever it shows — a study (or the demo) opens, the dossier changes.
pub(crate) fn reset(ui: &MainWindow) {
    write(ui, None, "");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_outcome_takes_an_empty_slot_or_its_own_source_never_a_sibling() {
        assert!(outcome_may_replace(None, Source::Fetch));
        // The fetch result replaces the fetch's own in-progress banner.
        assert!(outcome_may_replace(Some(Source::Fetch), Source::Fetch));
        // …but not an edit refusal raised while the fetch ran (F4).
        assert!(!outcome_may_replace(Some(Source::Edit), Source::Fetch));
        assert!(!outcome_may_replace(Some(Source::Fetch), Source::Edit));
        assert!(!outcome_may_replace(Some(Source::Render), Source::Edit));
    }

    #[test]
    fn a_success_clears_only_its_own_source() {
        assert!(success_clears(Some(Source::Edit), Source::Edit));
        // A cell commit that succeeds never erases the fetch failure, nor a normalize failure.
        assert!(!success_clears(Some(Source::Fetch), Source::Edit));
        assert!(!success_clears(Some(Source::Render), Source::Edit));
        assert!(!success_clears(None, Source::Edit));
    }
}
