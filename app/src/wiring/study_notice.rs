//! The OPEN STUDY's notice slot (G1 J, #237) — `Studies.study-notice`, shown pinned under the
//! verdict bar of the study screen. Before it, every notice meant for an open study landed in
//! `Studies.notice`, which only the study LIST shows: a fetch result, the soft-lock notice, an undo
//! or drag refusal were all invisible while the study was open, and read stale on the list later.
//! The two slots are now separate: `Studies.notice` stays the list's (create/archive/export/import
//! outcomes, startup states), this one belongs to the study on screen and is emptied whenever a
//! study (or the demo) opens and when the dossier changes.
//!
//! The notice-slot rule (F4, docs/review-checklist.md §3) lives HERE for this slot: each notice
//! carries its SOURCE (the gesture family that wrote it) and its KIND (failure / in-progress /
//! outcome). A FAILURE answering the gesture just made always shows; so does an IN-PROGRESS banner
//! (the user just started it). An OUTCOME (and the render-time normalize state) takes the slot only
//! when it is empty, holds the in-progress banner or another outcome, or holds its own source's
//! notice — never a sibling's failure. A success CLEARS only its own source's notice.

use std::cell::Cell;

use slint::{ComponentHandle, SharedString};

use crate::{MainWindow, Studies};

/// The gesture family that wrote the notice on show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    /// The provider fetch of the open study (its in-progress banner and its result), including the
    /// examen-rapide « étude créée » outcome (provider data written into the new study).
    Fetch,
    /// An edit of the open study: a cell, a review tag, a judgment, a rationale, undo/redo, the
    /// soft-lock, the unlock, the forecast-low option, the traceability read.
    Edit,
    /// What the study says of itself when it is rendered (the normalize failure).
    Render,
}

/// What the notice on show is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Failure,
    Progress,
    Outcome,
}

type Shown = Option<(Source, Kind)>;

thread_local! {
    // The UI is single-threaded (every callback runs on the event loop) — the `dialog` precedent.
    static SHOWN: Cell<Shown> = const { Cell::new(None) };
}

/// May a notice from `source` that must not cover a sibling's failure (an outcome, the render
/// state) take the slot? Pure — the F4 decision, unit-tested.
fn may_place(shown: Shown, source: Source) -> bool {
    match shown {
        None => true,
        Some((s, _)) if s == source => true,
        Some((_, kind)) => kind != Kind::Failure,
    }
}

/// Does a success from `source` empty the slot? Only when it shows `source`'s own notice (and,
/// with `outcome_only`, only an outcome of it — never its failure nor its in-progress banner).
fn clears(shown: Shown, source: Source, outcome_only: bool) -> bool {
    match shown {
        Some((s, kind)) if s == source => !outcome_only || kind == Kind::Outcome,
        _ => false,
    }
}

fn write(ui: &MainWindow, shown: Shown, text: &str) {
    SHOWN.with(|s| s.set(shown));
    ui.global::<Studies>()
        .set_study_notice(SharedString::from(text));
}

/// A failure answering the gesture just made — always shown.
pub(crate) fn fail(ui: &MainWindow, source: Source, text: &str) {
    write(ui, Some((source, Kind::Failure)), text);
}

/// The in-progress banner of a gesture just started — always shown; any outcome may replace it.
pub(crate) fn progress(ui: &MainWindow, source: Source, text: &str) {
    write(ui, Some((source, Kind::Progress)), text);
}

/// A success/info outcome — shown under the F4 rule (never over a sibling's failure).
pub(crate) fn outcome(ui: &MainWindow, source: Source, text: &str) {
    if may_place(SHOWN.with(Cell::get), source) {
        write(ui, Some((source, Kind::Outcome)), text);
    }
}

/// A render-time failure STATE (the normalize failure, rewritten on every render that fails): it
/// never erases another source's failure (a fetch failure, an edit refusal) — F4.
pub(crate) fn standing(ui: &MainWindow, source: Source, text: &str) {
    if may_place(SHOWN.with(Cell::get), source) {
        write(ui, Some((source, Kind::Failure)), text);
    }
}

/// A success of `source`: its own earlier notice (a refusal it just overcame) goes; a sibling's stays.
pub(crate) fn clear(ui: &MainWindow, source: Source) {
    if clears(SHOWN.with(Cell::get), source, false) {
        write(ui, None, "");
    }
}

/// `source`'s OUTCOME goes (it may no longer hold — an undo reverted what it reported); its
/// failure and its in-progress banner stay.
pub(crate) fn clear_outcome(ui: &MainWindow, source: Source) {
    if clears(SHOWN.with(Cell::get), source, true) {
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
    fn an_outcome_never_covers_a_siblings_failure() {
        assert!(may_place(None, Source::Fetch));
        // The fetch result replaces the fetch's own in-progress banner, and its own failure.
        assert!(may_place(
            Some((Source::Fetch, Kind::Progress)),
            Source::Fetch
        ));
        assert!(may_place(
            Some((Source::Fetch, Kind::Failure)),
            Source::Fetch
        ));
        // An edit outcome replaces the fetch's in-progress banner (F4) and an older outcome…
        assert!(may_place(
            Some((Source::Fetch, Kind::Progress)),
            Source::Edit
        ));
        assert!(may_place(
            Some((Source::Fetch, Kind::Outcome)),
            Source::Edit
        ));
        // …but never a sibling's failure — nor does the render state.
        assert!(!may_place(
            Some((Source::Edit, Kind::Failure)),
            Source::Fetch
        ));
        assert!(!may_place(
            Some((Source::Fetch, Kind::Failure)),
            Source::Edit
        ));
        assert!(!may_place(
            Some((Source::Fetch, Kind::Failure)),
            Source::Render
        ));
        assert!(!may_place(
            Some((Source::Edit, Kind::Failure)),
            Source::Render
        ));
        assert!(may_place(
            Some((Source::Render, Kind::Failure)),
            Source::Render
        ));
    }

    #[test]
    fn a_success_clears_only_its_own_source() {
        assert!(clears(
            Some((Source::Edit, Kind::Failure)),
            Source::Edit,
            false
        ));
        // A cell commit that succeeds never erases the fetch failure, nor a normalize failure.
        assert!(!clears(
            Some((Source::Fetch, Kind::Failure)),
            Source::Edit,
            false
        ));
        assert!(!clears(
            Some((Source::Render, Kind::Failure)),
            Source::Edit,
            false
        ));
        assert!(!clears(None, Source::Edit, false));
    }

    #[test]
    fn an_undo_clears_the_fetch_outcome_never_its_failure() {
        assert!(clears(
            Some((Source::Fetch, Kind::Outcome)),
            Source::Fetch,
            true
        ));
        assert!(!clears(
            Some((Source::Fetch, Kind::Failure)),
            Source::Fetch,
            true
        ));
        assert!(!clears(
            Some((Source::Fetch, Kind::Progress)),
            Source::Fetch,
            true
        ));
        assert!(!clears(
            Some((Source::Edit, Kind::Outcome)),
            Source::Fetch,
            true
        ));
    }
}
