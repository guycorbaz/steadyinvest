//! The study LIST's notice slot (G1 final review, #237) — `Studies.notice`, shown on the list
//! only (the open study has its own, [`crate::wiring::study_notice`]). It holds the outcomes of
//! the list's gestures (export, import, archive / unarchive / delete), a fetch result for a study
//! that is not open, and the startup states.
//!
//! The notice-slot rule (F4, docs/review-checklist.md §3) lives HERE for this slot: each notice
//! carries its SOURCE and its KIND. A FAILURE answering the gesture just made always shows; so
//! does an IN-PROGRESS banner. An OUTCOME takes the slot only when it is empty, holds an
//! in-progress banner or another outcome, or holds its own source's notice — never a sibling's
//! failure. A success CLEARS only its own source's notice.
//!
//! Every writer of this slot goes through here (G1 P: the examination's « study created » and the
//! startup state moved in). Two clears stay outside — creating a study and opening the demo
//! empty the slot outright. A notice on show that this module did not write has no known kind,
//! so it is treated as a failure: an outcome never covers it. That check compares the slot with
//! the text this module wrote last — it only tells « written here » from « written elsewhere »,
//! never a kind from a wording.

use std::cell::RefCell;

use slint::{ComponentHandle, SharedString};

use crate::{MainWindow, Studies};

/// The gesture family that wrote the notice on show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    /// A study exported (JSON envelope or PDF).
    Export,
    /// A study imported from a file.
    Import,
    /// A study archived, unarchived or deleted.
    StudyAction,
    /// A provider fetch whose study is not on screen.
    Fetch,
    /// The quick examination's « Créer l'étude » when the new study is not the one on screen.
    QuickScreen,
    /// The startup state (read-only dossier, unreadable configured file) — a standing fact,
    /// written as a failure so no outcome covers it.
    Startup,
}

/// What the notice on show is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Failure,
    Progress,
    Outcome,
}

/// The notice this module wrote last: its tag and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Written {
    source: Source,
    kind: Kind,
    text: String,
}

thread_local! {
    // The UI is single-threaded (every callback runs on the event loop) — the `dialog` precedent.
    static WRITTEN: RefCell<Option<Written>> = const { RefCell::new(None) };
}

/// May an outcome from `source` take the slot now showing `shown`? Pure — the F4 decision,
/// unit-tested. `written` is what this module wrote last.
fn may_place(shown: &str, written: Option<&Written>, source: Source) -> bool {
    if shown.is_empty() {
        return true;
    }
    match written {
        // The notice on show is ours: its tag decides.
        Some(w) if w.text == shown => w.source == source || w.kind != Kind::Failure,
        // Written elsewhere, kind unknown: kept, as a failure would be.
        _ => false,
    }
}

/// Does a success from `source` empty the slot? Only when it shows `source`'s own notice.
fn clears(shown: &str, written: Option<&Written>, source: Source) -> bool {
    matches!(written, Some(w) if w.text == shown && w.source == source)
}

fn write(ui: &MainWindow, source: Source, kind: Kind, text: &str) {
    WRITTEN.with(|w| {
        *w.borrow_mut() = Some(Written {
            source,
            kind,
            text: text.to_string(),
        })
    });
    ui.global::<Studies>().set_notice(SharedString::from(text));
}

fn shown(ui: &MainWindow) -> String {
    ui.global::<Studies>().get_notice().to_string()
}

/// A success/info outcome — shown under the F4 rule (never over a sibling's failure).
pub(crate) fn show(ui: &MainWindow, source: Source, text: &str) {
    let shown = shown(ui);
    if WRITTEN.with(|w| may_place(&shown, w.borrow().as_ref(), source)) {
        write(ui, source, Kind::Outcome, text);
    }
}

/// A failure answering the gesture just made — always shown.
pub(crate) fn fail(ui: &MainWindow, source: Source, text: &str) {
    write(ui, source, Kind::Failure, text);
}

/// The in-progress banner of a gesture just started — always shown; any outcome may replace it.
#[allow(dead_code)] // the F4 API of this slot, for its writers not moved here yet
pub(crate) fn progress(ui: &MainWindow, source: Source, text: &str) {
    write(ui, source, Kind::Progress, text);
}

/// A success of `source`: its own earlier notice goes; a sibling's stays.
#[allow(dead_code)] // the F4 API of this slot, for its writers not moved here yet
pub(crate) fn clear(ui: &MainWindow, source: Source) {
    let shown = shown(ui);
    if WRITTEN.with(|w| clears(&shown, w.borrow().as_ref(), source)) {
        WRITTEN.with(|w| *w.borrow_mut() = None);
        ui.global::<Studies>().set_notice(SharedString::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(source: Source, kind: Kind, text: &str) -> Written {
        Written {
            source,
            kind,
            text: text.into(),
        }
    }

    #[test]
    fn an_outcome_never_covers_a_siblings_failure() {
        // An empty slot takes any outcome.
        assert!(may_place("", None, Source::Export));
        // Another source's outcome or in-progress banner may be replaced…
        let archived = w(Source::StudyAction, Kind::Outcome, "archivée");
        assert!(may_place("archivée", Some(&archived), Source::Export));
        let fetching = w(Source::Fetch, Kind::Progress, "en cours");
        assert!(may_place("en cours", Some(&fetching), Source::Import));
        // …never its failure; its own source's failure, yes.
        let failed = w(Source::Fetch, Kind::Failure, "échec");
        assert!(!may_place("échec", Some(&failed), Source::Export));
        assert!(!may_place("échec", Some(&failed), Source::StudyAction));
        assert!(may_place("échec", Some(&failed), Source::Fetch));
        // G1 P: the startup state, now written here as a failure, is never covered by an
        // examination's outcome (nor any sibling's).
        let startup = w(Source::Startup, Kind::Failure, "lecture seule");
        assert!(!may_place(
            "lecture seule",
            Some(&startup),
            Source::QuickScreen
        ));
        assert!(!may_place("lecture seule", Some(&startup), Source::Export));
        // A notice written elsewhere is kept, whatever our last write was.
        assert!(!may_place("dossier en lecture seule", None, Source::Export));
        assert!(!may_place(
            "dossier en lecture seule",
            Some(&archived),
            Source::Export
        ));
    }

    #[test]
    fn a_success_clears_only_its_own_source() {
        let failed = w(Source::Fetch, Kind::Failure, "échec");
        assert!(clears("échec", Some(&failed), Source::Fetch));
        assert!(!clears("échec", Some(&failed), Source::Export));
        // Not ours any more (rewritten elsewhere): left alone.
        assert!(!clears("autre", Some(&failed), Source::Fetch));
        assert!(!clears("", None, Source::Fetch));
    }
}
