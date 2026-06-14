//! Study-state slice (Story 2.2): open/create the journal, list studies, create a new study, and
//! reopen one with full state. This is the *open/load/save* slice only — the architecture tree's
//! full `state.rs` (immutable StudyState snapshot, undo stack, content-addressed verdict) is Story
//! 2.9 (undo) / 2.6 (verdict); a documented partial is fine here.
//!
//! **No calculation lives here** (Cardinal Rule) and **no network** — only `steadyinvest-persistence`
//! and `steadyinvest-contract` are touched. Time and identity come *only* through the injected
//! [`Clock`] / [`IdGen`] (ADD15): this module never calls `Uuid::new_v4` or a wall clock itself.
//!
//! Failure modes degrade, never crash: a newer-schema file opens read-only (writes are refused with
//! a neutral notice), a corrupt/foreign configured file is set aside in favour of the default
//! journal (also with a notice), and the app stays usable throughout.

use std::path::{Path, PathBuf};

use steadyinvest_contract::{
    Cell, Coverage, ForecastLowOption, Judgment, Money, Provenance, Review, Source, Study,
    Timestamp,
};
use steadyinvest_persistence::{Error as PersistError, Journal, StudySummary};
use uuid::Uuid;

use steadyinvest_core::verdict::StudySnapshot;

use crate::clock::{Clock, IdGen};
use crate::viewmodel::{engine, entry};

// ── User-facing neutral notices (FR13). French (the UI source language); the crate-local posture
//    gate scans `USER_FACING_MESSAGES` for banned verbs, exactly like the `@tr()` literals. The
//    dynamic cause spliced into some of them comes from `persistence::Error`, already posture-gated
//    in that crate. ──

/// The ticker field was blank.
pub const MSG_BLANK_TICKER: &str = "Le symbole est vide ; aucune étude n'a été enregistrée.";
/// The native-currency field was blank.
pub const MSG_BLANK_CURRENCY: &str = "La devise est vide ; aucune étude n'a été enregistrée.";
/// A write was attempted on a read-only (newer-schema) journal.
pub const MSG_READ_ONLY_WRITE: &str =
    "Journal en lecture seule (schéma plus récent) ; l'écriture n'a pas eu lieu.";
/// No journal is open at all (even the default could not be prepared).
pub const MSG_NO_JOURNAL: &str = "Aucun journal n'est ouvert ; l'écriture n'a pas eu lieu.";
/// Startup: the open journal is read-only because its file was written by a newer schema.
pub const MSG_STARTUP_READ_ONLY: &str =
    "Le journal a été écrit par un schéma plus récent ; il est ouvert en lecture seule.";
/// Startup: the configured journal file was unreadable, so the default journal is in use instead.
pub const MSG_CONFIGURED_UNREADABLE: &str =
    "Le journal configuré est illisible ; le journal par défaut est utilisé.";
/// Startup: no journal directory is available from the OS (the default path cannot be computed).
pub const MSG_NO_DATA_DIR: &str =
    "Aucun emplacement de journal n'est disponible ; les études ne sont pas enregistrées.";
/// A save failed for a reason other than the read-only / identity guards (cause appended).
pub const MSG_SAVE_FAILED: &str = "L'enregistrement a échoué.";
/// The system clipboard could not be read for a paste-a-column (Story 2.4).
pub const MSG_CLIPBOARD_UNAVAILABLE: &str =
    "Le presse-papiers est indisponible ; aucune colonne n'a été collée.";
/// A pasted column had more lines than the grid has years; the surplus lines were dropped.
pub const MSG_PASTE_CLIPPED: &str =
    "Certaines lignes dépassaient la grille et n'ont pas été collées.";
/// A direct value edit was attempted on a validated (soft-locked) cell (Story 2.5). The sign-off
/// must be cleared first — the cell is never silently blanked by a stray keystroke.
pub const MSG_SOFT_LOCKED: &str = "Cellule validée ; retirez la validation avant de la modifier.";
/// The "unlock all" confirmation copy (Story 2.5) — fact-stating, posture-gated. `{n}` is replaced
/// with the count of validated cells the chosen scope would flip back to to-review.
pub const MSG_UNLOCK_CONFIRM: &str = "Cette action retire la validation de {n} cellule(s).";
/// The neutral notice after an "unlock all" completes — `{n}` is the count actually flipped.
pub const MSG_UNLOCK_DONE: &str = "Validation retirée de {n} cellule(s).";
/// The engine could not normalize the study's data (a structural input error — duplicate year /
/// invalid split): the verdict is suspended, never computed from broken inputs (Story 2.6).
pub const MSG_NORMALIZE_FAILED: &str =
    "Les données ne peuvent pas être préparées ; le calcul est suspendu.";

/// The confirmation prompt for an "unlock all" of `count` cells (a `{n}`-substitution of
/// [`MSG_UNLOCK_CONFIRM`] so the scanned const and the runtime string stay one source).
pub fn unlock_confirm_message(count: usize) -> String {
    MSG_UNLOCK_CONFIRM.replace("{n}", &count.to_string())
}

/// The completion notice for an "unlock all" that flipped `count` cells.
pub fn unlock_done_message(count: usize) -> String {
    MSG_UNLOCK_DONE.replace("{n}", &count.to_string())
}

/// Every static user-facing message above — exposed so the crate-local posture gate (FR13) scans
/// them for banned verbs alongside the `@tr()` literals. Test-only (the gate's sole consumer);
/// the individual `MSG_*` consts are the runtime surfaces. Keep in sync with the consts.
#[cfg(test)]
pub const USER_FACING_MESSAGES: &[&str] = &[
    MSG_BLANK_TICKER,
    MSG_BLANK_CURRENCY,
    MSG_READ_ONLY_WRITE,
    MSG_NO_JOURNAL,
    MSG_STARTUP_READ_ONLY,
    MSG_CONFIGURED_UNREADABLE,
    MSG_NO_DATA_DIR,
    MSG_SAVE_FAILED,
    MSG_CLIPBOARD_UNAVAILABLE,
    MSG_PASTE_CLIPPED,
    MSG_SOFT_LOCKED,
    MSG_UNLOCK_CONFIRM,
    MSG_UNLOCK_DONE,
    MSG_NORMALIZE_FAILED,
];

/// The scope of a bulk "unlock all" (Story 2.5): the whole study, a single year column, or a single
/// metric (one §3 column / §2 row) across all years. Each flips every `✓` it covers back to `?`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnlockScope {
    /// Every validated cell in the study.
    Study,
    /// Every validated cell in one materialized year (by index into the year window).
    Year(usize),
    /// Every validated cell of one field (a §3 column letter / §2 row key) across all years.
    Metric(String),
}

impl UnlockScope {
    /// Whether this scope covers the `(year_index, field)` cell address.
    fn covers(&self, year_index: usize, field: &str) -> bool {
        match self {
            UnlockScope::Study => true,
            UnlockScope::Year(y) => *y == year_index,
            UnlockScope::Metric(f) => f == field,
        }
    }
}

/// Where a default journal lives when the user has none yet: the OS **data** dir (NOT the config
/// dir, NOT beside `config.json`, NOT inside the journal) — outside any sync-watched tree (the
/// Synology-Drive SQLite-corruption risk, project memory). The location picker / sync-safety switch
/// is Story 5-5; this is the safe default only.
pub fn default_journal_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "steadyinvest")
        .map(|dirs| dirs.data_dir().join("journal.db"))
}

/// The all-`None` judgment a freshly-created study starts with (every optional `None`, plus the
/// default forecast-low option). 2.2 creates a study with no judgment inputs yet — those are 2.6.
fn empty_judgment() -> Judgment {
    Judgment {
        estimated_high_eps: None,
        estimated_low_eps: None,
        projected_sales_growth_pct: None,
        projected_eps_growth_pct: None,
        judged_avg_high_pe: None,
        judged_avg_low_pe: None,
        forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
        recent_severe_low: None,
        current_price: None,
        present_full_year_dividend: None,
    }
}

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
    fn record(&mut self, before: Study) {
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

/// The live journal session plus the injected time/identity sources. Owns the open [`Journal`] for
/// the lifetime of the window so the create/open callbacks can reach it.
pub struct JournalState {
    /// `None` only when not even the default journal could be opened/created — the app stays
    /// usable (read-only, study creation refused with [`MSG_NO_JOURNAL`]).
    journal: Option<Journal>,
    /// The resolved on-disk path (to persist into app-config), when a journal is open.
    path: Option<PathBuf>,
    /// True when the open journal is read-only (newer-schema file): writes are refused up front.
    read_only: bool,
    clock: Box<dyn Clock>,
    idgen: Box<dyn IdGen>,
    /// Undo/redo history for the currently-open study (Story 2.9). Reset on open.
    history: UndoHistory,
}

impl JournalState {
    /// Open the last-used journal (`configured`, from app-config) or, failing that, open/create the
    /// default journal in the OS data dir. Returns the state plus an optional neutral startup notice
    /// to surface in a banner. Never panics; a failure leaves a usable (journal-less) state.
    pub fn open_or_create(
        configured: Option<&Path>,
        clock: Box<dyn Clock>,
        idgen: Box<dyn IdGen>,
    ) -> (Self, Option<String>) {
        // 1) A configured journal that exists on disk → open it.
        if let Some(path) = configured {
            if path.exists() {
                match Journal::open(path) {
                    Ok(journal) => {
                        let read_only = journal.is_read_only();
                        return (
                            Self {
                                journal: Some(journal),
                                path: Some(path.to_path_buf()),
                                read_only,
                                clock,
                                idgen,
                                history: UndoHistory::default(),
                            },
                            read_only.then(|| MSG_STARTUP_READ_ONLY.to_string()),
                        );
                    }
                    Err(error) => {
                        // The configured pick is corrupt/foreign/damaged — never write our schema
                        // into it (open already refused without writing). Fall back to the default
                        // journal so the app stays usable, and surface the cause.
                        tracing::warn!("configured journal {} unreadable: {error}", path.display());
                        let (state, _) = Self::open_or_create_default(clock, idgen);
                        return (state, Some(MSG_CONFIGURED_UNREADABLE.to_string()));
                    }
                }
            }
        }
        // 2) No usable configured path → the default journal.
        Self::open_or_create_default(clock, idgen)
    }

    /// Open the default journal if its file already exists, else create it (parent dirs included),
    /// stamping identity + creation time from the injected sources.
    fn open_or_create_default(
        clock: Box<dyn Clock>,
        idgen: Box<dyn IdGen>,
    ) -> (Self, Option<String>) {
        let Some(path) = default_journal_path() else {
            return (
                Self {
                    journal: None,
                    path: None,
                    read_only: false,
                    clock,
                    idgen,
                    history: UndoHistory::default(),
                },
                Some(MSG_NO_DATA_DIR.to_string()),
            );
        };

        let result = if path.exists() {
            Journal::open(&path)
        } else {
            if let Some(parent) = path.parent() {
                if let Err(error) = std::fs::create_dir_all(parent) {
                    tracing::warn!("data dir {} not created: {error}", parent.display());
                }
            }
            Journal::create(&path, idgen.new_id(), &clock.now())
        };

        match result {
            Ok(journal) => {
                let read_only = journal.is_read_only();
                (
                    Self {
                        journal: Some(journal),
                        path: Some(path),
                        read_only,
                        clock,
                        idgen,
                        history: UndoHistory::default(),
                    },
                    read_only.then(|| MSG_STARTUP_READ_ONLY.to_string()),
                )
            }
            Err(error) => {
                tracing::warn!("default journal {} unavailable: {error}", path.display());
                (
                    Self {
                        journal: None,
                        path: None,
                        read_only: false,
                        clock,
                        idgen,
                        history: UndoHistory::default(),
                    },
                    Some(format!("{MSG_SAVE_FAILED} {error}")),
                )
            }
        }
    }

    /// The resolved on-disk path of the open journal, for persisting into app-config.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// True when the open journal is read-only (newer-schema file).
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

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
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study(&restored)
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

    /// Build the manual [`Provenance`] for an edit (Story 2.4): `source = Manual`, `timestamp` from
    /// the injected [`Clock`] (ADD15 — never a scattered wall clock). For a manually-entered **leaf**
    /// input there is no app-side per-cell version counter and no upstream dependency digest, so v1
    /// uses defensible sentinels — `logical_version = 1` and `hash_of_dependencies = "manual"` —
    /// recorded in the 2.4 interpretations issue; both earn real meaning in Epic 3 reconciliation.
    /// (`Provenance` performs no validation on these strings — `contract` module doc.)
    fn manual_provenance(&self) -> Provenance {
        Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: self.clock.now(),
            hash_of_dependencies: "manual".to_string(),
        }
    }

    /// Manually set/clear a cell's value (FR16): parse-side `value` (already a [`Money`] or `None`)
    /// is routed through the one mutation rail [`contract::Cell::edited`] with a manual provenance,
    /// then the whole [`Study`] is upserted via [`Journal::put_study`] (bumps `logical_version`,
    /// appends the FR51 time-series). A `None` value clears the cell to a [`Coverage::ToFill`] gap —
    /// **never `0`**. Reuses the read-only / no-journal / save-failure guards (no silent `.ok()`).
    pub fn edit_cell(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
        value: Option<Money>,
    ) -> Result<(), String> {
        // Soft-lock (Story 2.5): a validated (`✓`) cell refuses a DIRECT typed edit — the sign-off is
        // load-bearing and must be cleared deliberately first (clear-✓ → `?`), never undone by a
        // stray keystroke. This is the Rust-side backstop behind the read-only TextInput; the UI also
        // guards visually, but the rail is the testable, authoritative refusal. (Bulk `paste_column`
        // keeps the `Cell::edited` auto-demote backstop instead — recorded interpretation: typing is
        // blocked (a), paste is allowed-with-demote (b).)
        if self.current_review(study_id, year_index, field) == Some(Review::Validated) {
            return Err(MSG_SOFT_LOCKED.to_string());
        }
        self.mutate_cell(study_id, year_index, field, |base, provenance| {
            base.edited(value, provenance)
        })
    }

    /// The current review tag of a cell, or `None` if the study/year/cell is absent (a never-touched
    /// optional column reads as no cell). Used by the soft-lock guard before a direct value edit.
    fn current_review(&self, study_id: Uuid, year_index: usize, field: &str) -> Option<Review> {
        let study = self.get_study(study_id)?;
        let year = study.years.get(year_index)?;
        entry::get_cell(year, field).map(|cell| cell.review)
    }

    /// Set a cell's **review tag only** (Story 2.5, FR20): a review-only change that preserves the
    /// value, coverage, source, freshness and provenance verbatim — it does NOT route through
    /// [`Cell::edited`] (which would re-stamp source/freshness from a fresh provenance and re-derive
    /// coverage). The cycle `none → ? → ✓ → none` and the deliberate clear-✓ (`✓ → ?`) both land
    /// here; the UI computes the target. Setting a tag on a never-touched optional column materializes
    /// a to-fill gap carrying the tag — the value stays `None`, **never `0`**. Persisted via
    /// [`Journal::put_study`]; reuses the read-only / no-journal / save-failure guards.
    ///
    /// **Interpretation (recorded):** a review-only edit changes ONLY the `review` field — the cell's
    /// provenance (and its origin timestamp) is preserved verbatim, never re-stamped. The sign-off
    /// act's timing is captured by the study-level `logical_version` bump (FR51), not a per-cell
    /// provenance overwrite, so a value's source/fetch time is never lost to a review toggle.
    pub fn set_review(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
        review: Review,
    ) -> Result<(), String> {
        self.mutate_cell(study_id, year_index, field, move |base, _provenance| Cell {
            review,
            ..base
        })
    }

    /// Bulk "unlock all" (Story 2.5, FR20): flip every `Review::Validated → ToReview` within `scope`
    /// (study / year / metric), leaving `None`/`ToReview` cells untouched, in **one** persisted
    /// upsert (one `logical_version` bump). Returns the count of cells actually flipped (surfaced in a
    /// neutral notice). A review-only flip — values/coverage are never touched. Reuses the read-only /
    /// no-journal / save-failure guards.
    pub fn unlock_all(&mut self, study_id: Uuid, scope: &UnlockScope) -> Result<usize, String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let before = study.clone(); // pre-mutation snapshot for undo (Story 2.9)
        let mut flipped = 0usize;
        for (year_index, year) in study.years.iter_mut().enumerate() {
            for field in entry::ALL_FIELDS {
                if !scope.covers(year_index, field) {
                    continue;
                }
                let Some(cell) = entry::get_cell(year, field) else {
                    continue; // an absent optional column carries no sign-off
                };
                if cell.review != Review::Validated {
                    continue;
                }
                let demoted = Cell {
                    review: Review::ToReview,
                    ..cell
                };
                entry::set_cell(year, field, demoted).map_err(|()| MSG_SAVE_FAILED.to_string())?;
                flipped += 1;
            }
        }
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study(&study)
        };
        match result {
            Ok(()) => {
                // Only an unlock that actually flipped a ✓ is an undoable change.
                if flipped > 0 {
                    self.history.record(before);
                }
                Ok(flipped)
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Count the validated (`✓`) cells the given `scope` would flip — for the confirmation prompt
    /// (the bulk change is never silent). A read-only/pure query; never mutates or persists.
    pub fn count_validated(&self, study_id: Uuid, scope: &UnlockScope) -> usize {
        let Some(study) = self.get_study(study_id) else {
            return 0;
        };
        let mut count = 0usize;
        for (year_index, year) in study.years.iter().enumerate() {
            for field in entry::ALL_FIELDS {
                if !scope.covers(year_index, field) {
                    continue;
                }
                if entry::get_cell(year, field).map(|c| c.review) == Some(Review::Validated) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Mark a cell **not-available-accepted** (a deliberate, permanent gap — FR19), or clear that
    /// back to a to-fill gap (`accepted = false`). The value is cleared to `None` either way; only
    /// the coverage differs, so this is N/A vs to-fill vs 0 kept distinct. Persisted through the
    /// same upsert rail as [`Self::edit_cell`].
    pub fn set_not_available(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
        accepted: bool,
    ) -> Result<(), String> {
        // Soft-lock (Story 2.5): the not-available gesture is a value/coverage mutation, and AC 2
        // names it among the edits a `✓` cell must refuse — routed through `Cell::edited(None, …)` it
        // would both blank the value AND demote `✓ → ?` (a divergent edit), silently undoing the
        // sign-off. The UI swallows Ctrl+Space on a locked cell; this is the authoritative, testable
        // Rust backstop, symmetric with `edit_cell`, so the sign-off can never be lost by this path.
        if self.current_review(study_id, year_index, field) == Some(Review::Validated) {
            return Err(MSG_SOFT_LOCKED.to_string());
        }
        self.mutate_cell(study_id, year_index, field, move |base, provenance| {
            // Reuse the edit rail for value/source/freshness/review, then override coverage only —
            // `NotAvailableAccepted` is a coverage-only gesture, not reachable through `edited`.
            let cleared = base.edited(None, provenance);
            Cell {
                coverage: if accepted {
                    Coverage::NotAvailableAccepted
                } else {
                    Coverage::ToFill
                },
                ..cleared
            }
        })
    }

    /// The shared validate→materialize→mutate→persist path for a single cell. `make` builds the new
    /// cell from the current one (cloned, snapshot semantics) and a fresh manual provenance. The
    /// year-grid skeleton is materialized **on first edit** (not before — an untouched study is
    /// never written as all-empty rows).
    fn mutate_cell(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
        make: impl FnOnce(Cell, Provenance) -> Cell,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        // Re-read the authoritative study, mutate one cell, write it back (the 2.2/2.3 rail).
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        // The pre-mutation snapshot for undo (Story 2.9) — captured as read, before any materialize.
        let before = study.clone();
        if study.years.is_empty() {
            study.years =
                entry::materialize_year_window(&study.created_at, &self.manual_provenance());
        }
        let year = study
            .years
            .get_mut(year_index)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let base = entry::get_cell(year, field)
            .unwrap_or_else(|| entry::tofill_cell(self.manual_provenance()));
        let new_cell = make(base, self.manual_provenance());
        entry::set_cell(year, field, new_cell).map_err(|()| MSG_SAVE_FAILED.to_string())?;

        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study(&study)
        };
        match result {
            Ok(()) => {
                // Only a REAL change is undoable — a no-op edit (same value re-typed, same option
                // re-selected, same review tag) must not push a phantom step or clear redo (review P4).
                if before != study {
                    self.history.record(before);
                }
                Ok(())
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Paste a parsed column into consecutive years of the **same field**, downward from
    /// `start_year` (FR16). Each value is already a locale-parsed [`Money`] / `None`
    /// ([`entry::parse_pasted_column`]); a `None` line leaves its cell an empty to-fill gap (**never
    /// `0`**). Lines past the last year are dropped. One upsert for the whole column (one
    /// `logical_version` bump). Returns the number of cells actually filled (the caller compares it
    /// with the column length to surface a neutral "some lines dropped" notice).
    pub fn paste_column(
        &mut self,
        study_id: Uuid,
        start_year: usize,
        field: &str,
        values: &[Option<Money>],
    ) -> Result<usize, String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let before = study.clone(); // pre-mutation snapshot for undo (Story 2.9)
        if study.years.is_empty() {
            study.years =
                entry::materialize_year_window(&study.created_at, &self.manual_provenance());
        }
        let mut filled = 0usize;
        for (offset, value) in values.iter().enumerate() {
            let Some(year) = study.years.get_mut(start_year + offset) else {
                break; // ran past the grid bottom — drop the surplus line
            };
            let base = entry::get_cell(year, field)
                .unwrap_or_else(|| entry::tofill_cell(self.manual_provenance()));
            let new_cell = base.edited(*value, self.manual_provenance());
            entry::set_cell(year, field, new_cell).map_err(|()| MSG_SAVE_FAILED.to_string())?;
            filled += 1;
        }
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study(&study)
        };
        match result {
            Ok(()) => {
                // Only a real change is undoable (review P4).
                if before != study {
                    self.history.record(before);
                }
                Ok(filled)
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Set (or clear) one **numeric judgment field** (Story 2.6, FR6/FR31): re-read the study, set
    /// the one `Judgment` field on the mutation rail, then `put_study` — reusing the read-only /
    /// no-journal / save-failure guards (no silent `.ok()`). A cleared field is `None`, **never `0`**
    /// (the project's most-repeated rail). The `field` key is the Slint callback's wire identifier
    /// ([`judgment_field`] maps it to the struct field).
    pub fn set_judgment_field(
        &mut self,
        study_id: Uuid,
        field: &str,
        value: Option<Money>,
    ) -> Result<(), String> {
        self.mutate_judgment(study_id, |judgment| {
            apply_judgment_field(judgment, field, value)
        })
    }

    /// Select the §4 forecast-low option (Story 2.6) — a judgment edit through the same rail.
    pub fn set_forecast_low_option(
        &mut self,
        study_id: Uuid,
        option: ForecastLowOption,
    ) -> Result<(), String> {
        self.mutate_judgment(study_id, |judgment| {
            judgment.forecast_low_option = option;
            true
        })
    }

    /// Set (or clear) the study-level **decision rationale** (Story 2.10, FR49): the user's free-text
    /// "why I judged this way" note. Trims the incoming text and stores `Some(trimmed)`, or `None`
    /// when it is empty/whitespace-only — the project's "absence ≠ empty value" rail (never the
    /// empty-but-present `Some("")` surprise). Routed through [`Self::mutate_study`] so it is atomic
    /// (one `put_study`, `logical_version` bumped), guarded (read-only / no-journal / save-failure →
    /// a neutral notice, never a silent `.ok()`), and undoable (recorded only on a real change). The
    /// rationale is the user's own words and is therefore **never** posture-scanned — only the
    /// system-supplied label/placeholder are (FR13).
    pub fn set_rationale(&mut self, study_id: Uuid, text: Option<String>) -> Result<(), String> {
        let normalized = text
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        self.mutate_study(study_id, move |study| {
            study.rationale = normalized;
        })
    }

    /// The shared re-read → mutate-whole-study → persist path for a study-level field that is neither
    /// a review-tagged [`Cell`] nor a [`Judgment`] input (Story 2.10: the decision rationale). Mirrors
    /// [`Self::mutate_judgment`] but hands `apply` the whole [`Study`]. Records an undo snapshot only
    /// on a real change (`before != study`, the Story-2.9 guard) so a no-op re-save pushes no phantom
    /// step. Reuses the read-only / no-journal / save-failure guards.
    fn mutate_study(
        &mut self,
        study_id: Uuid,
        apply: impl FnOnce(&mut Study),
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let before = study.clone(); // pre-mutation snapshot for undo (Story 2.9)
        apply(&mut study);
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study(&study)
        };
        match result {
            Ok(()) => {
                // Only a REAL change is undoable — re-saving the same rationale must not push a
                // phantom step or clear redo (review P4).
                if before != study {
                    self.history.record(before);
                }
                Ok(())
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// The shared re-read → mutate-judgment → persist path. `apply` returns `false` for an unknown
    /// field key (a neutral save-failure notice; never a panic). Mirrors [`Self::mutate_cell`] but
    /// for the bare `Judgment` snapshot (judgment inputs are not review-tagged `Cell`s).
    fn mutate_judgment(
        &mut self,
        study_id: Uuid,
        apply: impl FnOnce(&mut Judgment) -> bool,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let before = study.clone(); // pre-mutation snapshot for undo (Story 2.9)
        if !apply(&mut study.judgment) {
            return Err(MSG_SAVE_FAILED.to_string());
        }
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study(&study)
        };
        match result {
            Ok(()) => {
                // Only a REAL change is undoable — a no-op edit (same value re-typed, same option
                // re-selected, same review tag) must not push a phantom step or clear redo (review P4).
                if before != study {
                    self.history.record(before);
                }
                Ok(())
            }
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

/// Set one numeric judgment field by its Slint wire key (Story 2.6). Returns `false` for an unknown
/// key (the caller surfaces a neutral notice; never a panic). A `None` value clears the field —
/// **never `0`**. The `forecast_low_option` selector has its own rail ([`JournalState::
/// set_forecast_low_option`]) — it is not routed here.
pub(crate) fn apply_judgment_field(
    judgment: &mut Judgment,
    field: &str,
    value: Option<Money>,
) -> bool {
    match field {
        "sales_growth" => judgment.projected_sales_growth_pct = value,
        "eps_growth" => judgment.projected_eps_growth_pct = value,
        "est_high_eps" => judgment.estimated_high_eps = value,
        "est_low_eps" => judgment.estimated_low_eps = value,
        "high_pe" => judgment.judged_avg_high_pe = value,
        "low_pe" => judgment.judged_avg_low_pe = value,
        "recent_severe_low" => judgment.recent_severe_low = value,
        "current_price" => judgment.current_price = value,
        "dividend" => judgment.present_full_year_dividend = value,
        _ => return false,
    }
    true
}

/// A neutral RFC3339 timestamp rendered for the dashboard list: the date portion only (the time of
/// day is not meaningful in the v1 list). A non-RFC3339 string passes through unchanged — this is a
/// display transform, it never repairs a value.
pub fn created_at_date(ts: &Timestamp) -> String {
    ts.0.split('T').next().unwrap_or(&ts.0).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{FixedClock, FixedIdGen};
    use tempfile::TempDir;

    fn fixed(id: u128, ts: &str) -> (Box<dyn Clock>, Box<dyn IdGen>) {
        (
            Box::new(FixedClock(Timestamp(ts.to_string()))),
            Box::new(FixedIdGen(Uuid::from_u128(id))),
        )
    }

    // ── Story 2.9 — undo/redo history ──

    /// Open a fresh temp journal + state (creating the file on first use), with injected clock/id.
    fn undo_state(dir: &TempDir, seed: u128, ts: &str) -> JournalState {
        let path = dir.path().join("journal.db");
        if !path.exists() {
            drop(
                Journal::create(
                    &path,
                    Uuid::from_u128(0xC0FFEE),
                    &Timestamp("2026-06-14T00:00:00Z".to_string()),
                )
                .unwrap(),
            );
        }
        let (clock, idgen) = fixed(seed, ts);
        let (state, _) = JournalState::open_or_create(Some(&path), clock, idgen);
        state
    }

    fn und_money(v: i64) -> Money {
        Money::from(rust_decimal::Decimal::new(v, 0))
    }

    #[test]
    fn undo_redo_steps_back_and_forward_and_a_new_edit_clears_redo() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x1D, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        assert!(
            !state.can_undo() && !state.can_redo(),
            "a fresh study has empty history"
        );

        // Edit one §3 cell (field "a" = high price) on year 0.
        state.edit_cell(id, 0, "a", Some(und_money(100))).unwrap();
        assert!(state.can_undo(), "an edit is undoable");
        assert!(!state.can_redo());
        assert!(state.get_study(id).unwrap().years[0]
            .high_price
            .value
            .is_some());

        // Undo → the pre-edit (fresh, no-value) state returns.
        assert_eq!(state.undo(id), Ok(true));
        assert!(state.can_redo());
        let undone = state.get_study(id).unwrap();
        assert!(
            undone.years.is_empty() || undone.years[0].high_price.value.is_none(),
            "undo restores the pre-edit state (no value)"
        );

        // Redo → the value comes back.
        assert_eq!(state.redo(id), Ok(true));
        assert!(state.get_study(id).unwrap().years[0]
            .high_price
            .value
            .is_some());

        // A NEW edit after an undo forks history → the redo branch is cleared.
        assert_eq!(state.undo(id), Ok(true));
        assert!(state.can_redo());
        state.edit_cell(id, 0, "b", Some(und_money(50))).unwrap();
        assert!(
            !state.can_redo(),
            "a new edit after an undo clears the redo branch"
        );
        assert!(state.can_undo());
    }

    #[test]
    fn undo_restores_a_judgment_edit() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x2D, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        state
            .set_judgment_field(id, "est_high_eps", Some(und_money(9)))
            .unwrap();
        assert!(state
            .get_study(id)
            .unwrap()
            .judgment
            .estimated_high_eps
            .is_some());
        assert_eq!(state.undo(id), Ok(true));
        assert!(
            state
                .get_study(id)
                .unwrap()
                .judgment
                .estimated_high_eps
                .is_none(),
            "undo restores the prior (unset) judgment — FR32, never destroys a saved input"
        );
    }

    #[test]
    fn undo_redo_on_empty_history_are_noops() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x3D, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        assert_eq!(state.undo(id), Ok(false), "nothing to undo");
        assert_eq!(state.redo(id), Ok(false), "nothing to redo");
        assert!(!state.can_undo() && !state.can_redo());
    }

    #[test]
    fn reset_undo_clears_history() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x4D, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        state
            .set_judgment_field(id, "est_high_eps", Some(und_money(9)))
            .unwrap();
        assert!(state.can_undo());
        state.reset_undo(); // a different study is opened
        assert!(
            !state.can_undo() && !state.can_redo(),
            "opening a study starts from an empty history"
        );
    }

    // ── Story 2.10 — decision rationale: set → reopen restores; clear → None; trim; undo restores ──

    #[test]
    fn rationale_round_trips_through_reopen_and_clears_to_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xC0FFEE),
                &Timestamp("2026-06-14T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut state = open_state(&path);
        let id = state.create_study("NESN", "CHF").unwrap();

        // Set a rationale → it is restored on reopen (a fresh JournalState on the same journal, FR49).
        state
            .set_rationale(id, Some("Marge en hausse, dette faible".to_string()))
            .unwrap();
        assert_eq!(
            open_state(&path).get_study(id).unwrap().rationale.as_deref(),
            Some("Marge en hausse, dette faible"),
            "a saved rationale survives reopen (FR49)"
        );

        // Whitespace-only clears to None (absence ≠ empty value) — never Some("").
        state.set_rationale(id, Some("   ".to_string())).unwrap();
        assert_eq!(
            open_state(&path).get_study(id).unwrap().rationale,
            None,
            "an empty/whitespace rationale stores None, never Some(\"\")"
        );

        // A bare `None` clears it too.
        state
            .set_rationale(id, Some("re-rempli".to_string()))
            .unwrap();
        state.set_rationale(id, None).unwrap();
        assert_eq!(open_state(&path).get_study(id).unwrap().rationale, None);
    }

    #[test]
    fn rationale_is_trimmed_before_storage() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xA),
                &Timestamp("2026-06-14T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut state = open_state(&path);
        let id = state.create_study("NESN", "CHF").unwrap();
        state
            .set_rationale(id, Some("  garde le texte  ".to_string()))
            .unwrap();
        assert_eq!(
            state.get_study(id).unwrap().rationale.as_deref(),
            Some("garde le texte"),
            "surrounding whitespace is trimmed before storage"
        );
    }

    #[test]
    fn undo_restores_the_prior_rationale() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x6A, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();

        state
            .set_rationale(id, Some("première raison".to_string()))
            .unwrap();
        state
            .set_rationale(id, Some("raison révisée".to_string()))
            .unwrap();
        assert_eq!(
            state.get_study(id).unwrap().rationale.as_deref(),
            Some("raison révisée")
        );

        // Undo restores the prior rationale (FR32 — a rationale edit is "any edit", never destroyed).
        assert_eq!(state.undo(id), Ok(true));
        assert_eq!(
            state.get_study(id).unwrap().rationale.as_deref(),
            Some("première raison"),
            "undo restores the prior rationale, never destroys it"
        );
    }

    #[test]
    fn re_saving_the_same_rationale_records_no_phantom_undo_step() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x6B, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        state.set_rationale(id, Some("inchangé".to_string())).unwrap();
        state.reset_undo();
        // Re-saving the identical rationale (after trimming) is a no-op → no undo step recorded (P4).
        state
            .set_rationale(id, Some("  inchangé  ".to_string()))
            .unwrap();
        assert!(
            !state.can_undo(),
            "re-saving the same rationale records no phantom undo step (review P4)"
        );
    }

    #[test]
    fn undo_restores_a_review_tag_without_destroying_the_value() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x5E, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        state.edit_cell(id, 0, "a", Some(und_money(100))).unwrap();
        state.set_review(id, 0, "a", Review::Validated).unwrap();
        assert_eq!(
            state.get_study(id).unwrap().years[0].high_price.review,
            Review::Validated
        );
        assert_eq!(state.undo(id), Ok(true)); // undo the review change only
        let undone = state.get_study(id).unwrap();
        assert_eq!(
            undone.years[0].high_price.review,
            Review::None,
            "undo restores the prior review tag"
        );
        assert!(
            undone.years[0].high_price.value.is_some(),
            "undoing the review tag never destroys the value"
        );
    }

    #[test]
    fn create_then_list_then_reopen_restores_full_state() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (clock, idgen) = fixed(0x5D, "2026-06-13T09:00:00Z");
        // Pre-create a journal at a known path so `open_or_create` opens it (rather than falling
        // through to the OS data dir, which is unavailable / undesirable under test).
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xC0FFEE),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );

        let (mut state, notice) = JournalState::open_or_create(Some(&path), clock, idgen);
        assert!(notice.is_none(), "clean open has no notice");
        assert!(!state.is_read_only());
        assert_eq!(state.path(), Some(path.as_path()));
        assert_eq!(state.list_studies().len(), 0, "no studies yet");

        let id = state
            .create_study("  NESN ", " chf ")
            .expect("a valid study is created");
        let rows = state.list_studies();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].security_ticker, "NESN",
            "ticker trimmed, case preserved"
        );
        assert_eq!(rows[0].status, "active");

        let back = state.get_study(id).expect("the study reopens");
        assert_eq!(back.security_ticker, "NESN");
        assert_eq!(
            back.native_currency, "CHF",
            "currency trimmed + upper-cased"
        );
        assert!(back.years.is_empty(), "a fresh study has no years");
        assert_eq!(back.created_at.0, "2026-06-13T09:00:00Z", "injected clock");
        assert_eq!(id, Uuid::from_u128(0x5D), "injected id");
    }

    #[test]
    fn blank_ticker_is_refused_with_a_neutral_message_and_writes_nothing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xA),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let (clock, idgen) = fixed(0x1, "2026-06-13T09:00:00Z");
        let (mut state, _) = JournalState::open_or_create(Some(&path), clock, idgen);

        assert_eq!(
            state.create_study("   ", "CHF"),
            Err(MSG_BLANK_TICKER.into())
        );
        assert_eq!(
            state.create_study("NESN", "  "),
            Err(MSG_BLANK_CURRENCY.into())
        );
        assert_eq!(state.list_studies().len(), 0, "no study was written");
    }

    #[test]
    fn missing_configured_file_falls_through_to_a_created_default_or_none() {
        // A configured path that does NOT exist must not be opened as an empty journal; the code
        // falls through to the default. In a sandbox the data dir may be unavailable — either a
        // created default (Some path) or a clean no-journal state is acceptable, never a panic.
        let (clock, idgen) = fixed(0x2, "2026-06-13T09:00:00Z");
        let missing = PathBuf::from("/nonexistent/steadyinvest/journal.db");
        let (state, _notice) = JournalState::open_or_create(Some(&missing), clock, idgen);
        assert_ne!(
            state.path(),
            Some(missing.as_path()),
            "a missing configured file is never adopted as-is"
        );
    }

    #[test]
    fn created_at_date_takes_the_date_portion() {
        assert_eq!(
            created_at_date(&Timestamp("2026-06-13T09:00:00Z".to_string())),
            "2026-06-13"
        );
        assert_eq!(created_at_date(&Timestamp("weird".to_string())), "weird");
    }

    // ── Story 2.4: manual entry → `Cell::edited` → `put_study` → reopen round-trip ──

    fn open_state(path: &Path) -> JournalState {
        let (clock, idgen) = fixed(0x5D, "2026-06-13T09:00:00Z");
        let (state, _) = JournalState::open_or_create(Some(path), clock, idgen);
        state
    }

    fn money(s: &str) -> Money {
        Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
    }

    #[test]
    fn manual_edit_stamps_source_manual_present_and_survives_reopen() {
        use steadyinvest_contract::{Freshness, Source};
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xC0FFEE),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut state = open_state(&path);
        let id = state.create_study("NESN", "CHF").unwrap();

        // A fresh study has no years; the first edit materializes the window then sets the cell.
        state
            .edit_cell(id, 0, entry::FIELD_HIGH, Some(money("120.5")))
            .expect("edit persists");

        // Reopen from disk: the entered value, its manual source/freshness and Present coverage survive.
        let back = open_state(&path).get_study(id).expect("study reopens");
        assert_eq!(
            back.years.len(),
            entry::YEAR_WINDOW,
            "the window was materialized"
        );
        let cell = &back.years[0].high_price;
        assert_eq!(cell.value, Some(money("120.5")));
        assert_eq!(
            cell.source,
            Source::Manual,
            "a manual edit is stamped source=manual"
        );
        assert_eq!(
            cell.freshness,
            Freshness::Current,
            "a fresh edit is current"
        );
        assert_eq!(cell.coverage, Coverage::Present);
        assert_eq!(cell.provenance.source, Source::Manual);
        assert_eq!(
            cell.provenance.timestamp.0, "2026-06-13T09:00:00Z",
            "the provenance timestamp comes from the injected clock"
        );
    }

    #[test]
    fn clearing_a_cell_reopens_a_to_fill_gap_never_zero() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xA),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut state = open_state(&path);
        let id = state.create_study("NESN", "CHF").unwrap();

        state
            .edit_cell(id, 1, entry::FIELD_EPS, Some(money("4.2")))
            .unwrap();
        state.edit_cell(id, 1, entry::FIELD_EPS, None).unwrap(); // clear it

        let cell = open_state(&path).get_study(id).unwrap().years[1]
            .eps
            .clone();
        assert_eq!(cell.value, None, "a cleared cell holds no value — never 0");
        assert_eq!(
            cell.coverage,
            Coverage::ToFill,
            "a cleared cell is a to-fill gap"
        );
    }

    #[test]
    fn not_available_is_a_distinct_quiet_gap_that_survives_reopen() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xB),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut state = open_state(&path);
        let id = state.create_study("NESN", "CHF").unwrap();

        // An optional column (dividend) marked not-available: distinct from to-fill and from 0.
        state
            .set_not_available(id, 2, entry::FIELD_DIVIDEND, true)
            .unwrap();
        let cell = open_state(&path).get_study(id).unwrap().years[2]
            .dividend_per_share
            .clone()
            .expect("the cell now exists");
        assert_eq!(cell.value, None);
        assert_eq!(cell.coverage, Coverage::NotAvailableAccepted);

        // Clearing it back returns a to-fill gap.
        state
            .set_not_available(id, 2, entry::FIELD_DIVIDEND, false)
            .unwrap();
        let back = open_state(&path).get_study(id).unwrap().years[2]
            .dividend_per_share
            .clone()
            .unwrap();
        assert_eq!(back.coverage, Coverage::ToFill);
    }

    // ── Story 2.5: tri-state review tag set/clear → persist → reopen; soft-lock; bulk unlock ──

    /// Open a journal at `path`, create a study, and fill A/C on year 0 so there is a present cell to
    /// review. Returns the state (still open) and the study id.
    fn study_with_entry(path: &Path) -> (JournalState, Uuid) {
        drop(
            Journal::create(
                path,
                Uuid::from_u128(0xC0FFEE),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut state = open_state(path);
        let id = state.create_study("NESN", "CHF").unwrap();
        state
            .edit_cell(id, 0, entry::FIELD_HIGH, Some(money("120.5")))
            .unwrap();
        (state, id)
    }

    #[test]
    fn set_review_survives_reopen_and_leaves_value_and_coverage_untouched() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        // none → ? → ✓, each persisted; the value and coverage never move (review-only edits).
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::ToReview)
            .unwrap();
        let cell = open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .clone();
        assert_eq!(cell.review, Review::ToReview);
        assert_eq!(cell.value, Some(money("120.5")), "value untouched by ?");
        assert_eq!(cell.coverage, Coverage::Present, "coverage untouched by ?");

        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
        let cell = open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .clone();
        assert_eq!(cell.review, Review::Validated, "✓ survives reopen");
        assert_eq!(cell.value, Some(money("120.5")));
        assert_eq!(cell.coverage, Coverage::Present);

        // ✓ → none clears the tag; still a review-only change.
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::None)
            .unwrap();
        let cell = open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .clone();
        assert_eq!(cell.review, Review::None);
        assert_eq!(
            cell.value,
            Some(money("120.5")),
            "clearing ✓ keeps the value"
        );
    }

    #[test]
    fn reviewing_a_to_fill_gap_keeps_the_value_none_never_zero() {
        // Setting a tag on a never-entered optional column materializes a to-fill cell carrying the
        // tag — the value stays None (the project's most-repeated rail: unknown is never 0).
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        state
            .set_review(id, 2, entry::FIELD_DIVIDEND, Review::ToReview)
            .unwrap();
        let cell = open_state(&path).get_study(id).unwrap().years[2]
            .dividend_per_share
            .clone()
            .expect("the cell now exists");
        assert_eq!(cell.review, Review::ToReview);
        assert_eq!(cell.value, None, "a reviewed gap holds no value — never 0");
        assert_eq!(cell.coverage, Coverage::ToFill);
    }

    #[test]
    fn a_validated_cell_is_soft_locked_until_the_tag_is_cleared() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();

        // A direct typed edit on the ✓ cell is refused with the neutral soft-lock notice, and the
        // on-disk value is unchanged (never silently blanked or overwritten).
        assert_eq!(
            state.edit_cell(id, 0, entry::FIELD_HIGH, Some(money("999"))),
            Err(MSG_SOFT_LOCKED.to_string())
        );
        let cell = open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .clone();
        assert_eq!(
            cell.value,
            Some(money("120.5")),
            "the refused edit wrote nothing"
        );
        assert_eq!(cell.review, Review::Validated, "the sign-off is intact");

        // The deliberate clear-✓ → ? releases the lock (recheck status preserved, not blanked).
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::ToReview)
            .unwrap();
        // Now the cell edits normally again.
        state
            .edit_cell(id, 0, entry::FIELD_HIGH, Some(money("130")))
            .expect("a ? cell edits normally");
        let cell = open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .clone();
        assert_eq!(cell.value, Some(money("130")));
        assert_eq!(cell.review, Review::ToReview, "editing a ? cell keeps ?");
    }

    #[test]
    fn set_not_available_is_refused_on_a_validated_cell_so_the_sign_off_is_never_blanked() {
        // The not-available gesture (Ctrl+Space) is a value/coverage mutation; on a `✓` cell it would
        // otherwise route through `Cell::edited(None, …)`, blanking the value AND demoting `✓ → ?`.
        // AC 2 forbids that — the soft-lock backstop must refuse it just like a typed edit does.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
        assert_eq!(
            state.set_not_available(id, 0, entry::FIELD_HIGH, true),
            Err(MSG_SOFT_LOCKED.to_string()),
            "not-available on a ✓ cell is refused"
        );
        // The on-disk cell is untouched: value kept, sign-off intact, still a present cell.
        let cell = open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .clone();
        assert_eq!(cell.value, Some(money("120.5")), "value untouched");
        assert_eq!(cell.review, Review::Validated, "sign-off intact");
        assert_eq!(cell.coverage, Coverage::Present, "coverage untouched");
    }

    #[test]
    fn unlock_all_flips_only_validated_cells_at_each_scope_and_persists() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        // Build a mixed field of tags across years/fields:
        //   (y0, A) ✓   (y1, A) ✓   (y0, C) ?   (y2, B) ✓
        state
            .edit_cell(id, 1, entry::FIELD_HIGH, Some(money("1")))
            .unwrap();
        state
            .edit_cell(id, 0, entry::FIELD_EPS, Some(money("2")))
            .unwrap();
        state
            .edit_cell(id, 2, entry::FIELD_LOW, Some(money("3")))
            .unwrap();
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
        state
            .set_review(id, 1, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
        state
            .set_review(id, 0, entry::FIELD_EPS, Review::ToReview)
            .unwrap();
        state
            .set_review(id, 2, entry::FIELD_LOW, Review::Validated)
            .unwrap();

        // ── Per-metric scope (field A): flips (y0,A) and (y1,A) only; (y2,B) ✓ is left.
        assert_eq!(
            state.count_validated(id, &UnlockScope::Metric(entry::FIELD_HIGH.to_string())),
            2
        );
        let flipped = state
            .unlock_all(id, &UnlockScope::Metric(entry::FIELD_HIGH.to_string()))
            .unwrap();
        assert_eq!(flipped, 2, "two A cells flipped");
        let back = open_state(&path).get_study(id).unwrap();
        assert_eq!(back.years[0].high_price.review, Review::ToReview);
        assert_eq!(back.years[1].high_price.review, Review::ToReview);
        assert_eq!(
            back.years[0].eps.review,
            Review::ToReview,
            "the ? cell is untouched"
        );
        assert_eq!(
            back.years[2].low_price.review,
            Review::Validated,
            "a different metric keeps its ✓"
        );

        // ── Per-year scope (year 2): flips (y2,B) only.
        let flipped = state.unlock_all(id, &UnlockScope::Year(2)).unwrap();
        assert_eq!(flipped, 1);
        assert_eq!(
            open_state(&path).get_study(id).unwrap().years[2]
                .low_price
                .review,
            Review::ToReview
        );

        // ── Study scope on an already-cleared study: nothing left to flip.
        assert_eq!(state.unlock_all(id, &UnlockScope::Study).unwrap(), 0);
    }

    #[test]
    fn unlock_all_study_scope_flips_every_validated_cell_in_one_upsert() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);
        state
            .edit_cell(id, 3, entry::FIELD_EPS, Some(money("9")))
            .unwrap();
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
        state
            .set_review(id, 3, entry::FIELD_EPS, Review::Validated)
            .unwrap();

        assert_eq!(state.count_validated(id, &UnlockScope::Study), 2);
        let flipped = state.unlock_all(id, &UnlockScope::Study).unwrap();
        assert_eq!(flipped, 2);
        let back = open_state(&path).get_study(id).unwrap();
        assert_eq!(back.years[0].high_price.review, Review::ToReview);
        assert_eq!(back.years[3].eps.review, Review::ToReview);
        assert_eq!(
            back.years[0].high_price.value,
            Some(money("120.5")),
            "values untouched"
        );
    }

    #[test]
    fn unlock_messages_substitute_the_count() {
        assert_eq!(
            unlock_confirm_message(3),
            "Cette action retire la validation de 3 cellule(s)."
        );
        assert_eq!(
            unlock_done_message(1),
            "Validation retirée de 1 cellule(s)."
        );
    }

    // ── Story 2.6: numeric judgment editing → persist → reopen; snapshot_for engine wiring ──

    #[test]
    fn judgment_fields_round_trip_and_clear_to_none_never_zero() {
        use steadyinvest_contract::ForecastLowOption;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        // Set each numeric judgment field; the option selector; then reopen and verify each survives.
        for (field, value) in [
            ("sales_growth", "12.5"),
            ("eps_growth", "10"),
            ("est_high_eps", "8.4"),
            ("est_low_eps", "3.1"),
            ("high_pe", "22"),
            ("low_pe", "11"),
            ("recent_severe_low", "44.5"),
            ("current_price", "60"),
            ("dividend", "2.25"),
        ] {
            state
                .set_judgment_field(id, field, Some(money(value)))
                .unwrap();
        }
        state
            .set_forecast_low_option(id, ForecastLowOption::RecentSevereLow)
            .unwrap();

        let j = open_state(&path).get_study(id).unwrap().judgment;
        assert_eq!(j.projected_sales_growth_pct, Some(money("12.5")));
        assert_eq!(j.estimated_high_eps, Some(money("8.4")));
        assert_eq!(j.judged_avg_high_pe, Some(money("22")));
        assert_eq!(j.current_price, Some(money("60")));
        assert_eq!(j.present_full_year_dividend, Some(money("2.25")));
        assert_eq!(j.forecast_low_option, ForecastLowOption::RecentSevereLow);

        // Clearing a field stores None — never 0.
        state.set_judgment_field(id, "current_price", None).unwrap();
        let j = open_state(&path).get_study(id).unwrap().judgment;
        assert_eq!(
            j.current_price, None,
            "a cleared judgment field is None, never 0"
        );
    }

    #[test]
    fn snapshot_for_runs_the_engine_and_matches_build_snapshot() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);
        // Fill the load-bearing cells + judgment so the snapshot is computable.
        for y in 0..entry::YEAR_WINDOW {
            for (field, v) in [
                (entry::FIELD_HIGH, "100"),
                (entry::FIELD_LOW, "50"),
                (entry::FIELD_EPS, "5"),
            ] {
                state.edit_cell(id, y, field, Some(money(v))).unwrap();
            }
        }
        for (field, v) in [
            ("est_high_eps", "8"),
            ("est_low_eps", "3"),
            ("high_pe", "20"),
            ("low_pe", "10"),
            ("current_price", "60"),
        ] {
            state.set_judgment_field(id, field, Some(money(v))).unwrap();
        }

        let snap = state.snapshot_for(id).expect("snapshot computes");
        // No drift: the state-level snapshot equals the pure adapter snapshot on the same study.
        let study = state.get_study(id).unwrap();
        let direct = crate::viewmodel::engine::build_snapshot(&study).unwrap();
        assert_eq!(snap.outputs(), direct.outputs());
        assert_eq!(snap.verdict(), direct.verdict());
    }

    #[test]
    fn an_edit_on_a_read_only_journal_is_refused_and_writes_nothing() {
        // A study created in a writable journal, then reopened read-only: the edit is refused with the
        // neutral notice and the on-disk value is unchanged.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xC),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let id = {
            let mut state = open_state(&path);
            state.create_study("NESN", "CHF").unwrap()
        };
        // Force a read-only state by constructing one whose `read_only` flag is set.
        let mut state = open_state(&path);
        state.read_only = true;
        assert_eq!(
            state.edit_cell(id, 0, entry::FIELD_HIGH, Some(money("1"))),
            Err(MSG_READ_ONLY_WRITE.to_string())
        );
        // Nothing was written: the cell is still absent/empty.
        let back = open_state(&path).get_study(id).unwrap();
        assert!(back.years.is_empty(), "a refused edit materialized nothing");
    }
}
