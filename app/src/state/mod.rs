//! Journal-session state — the app-layer rail between the Slint callbacks and
//! `steadyinvest-persistence`. Owns the open [`Journal`] for the lifetime of the window and exposes
//! every journal-backed action as a guarded method on [`JournalState`].
//!
//! **No calculation lives here** (Cardinal Rule) and **no network** — only `steadyinvest-persistence`
//! and `steadyinvest-contract` are touched. Time and identity come *only* through the injected
//! [`Clock`] / [`IdGen`] (ADD15): this module never calls `Uuid::new_v4` or a wall clock itself.
//! Failure modes degrade, never crash: a newer-schema file opens read-only (writes are refused with
//! a neutral notice), a corrupt/foreign configured file is set aside in favour of the default
//! journal (also with a notice), and the app stays usable throughout.
//!
//! Grown one story at a time from the Story-2.2 open/load/save slice, the module is now split by
//! concern into submodules (all re-exported here, so `state::…` paths are unchanged):
//!
//! - [`messages`] — the posture-gated `MSG_*` notices + their substitution helpers (FR13);
//! - [`undo`] — the snapshot-stack undo/redo history (Story 2.9, FR32);
//! - [`journal_io`] — open/create/switch journals, sync-folder safety, locks, backups (5.4/5.5);
//! - [`restore`] — the assess→confirm restore-from-backup flow (Story 5.4, FR61);
//! - [`watchlist`] — watched-securities CRUD (Story 4.1, FR34);
//! - [`holdings`] — portfolios, the holdings register, trailing stops (4.3/4.5/4.7/6.1/6.2);
//! - [`studies`] — study lifecycle (create/list/reopen, archive/delete) + the engine call site;
//! - [`export_import`] — the portable JSON envelopes (Stories 5.2/5.3, FR59/FR60);
//! - [`refresh`] — the provider fetch/refresh cell rail (Stories 3.1/3.3–3.6);
//! - [`confront`] — the read-only reopen-and-confront view + price-history cache (5.1, FR50);
//! - [`cells`] — the cell/judgment editing rail (soft-lock, review tags, paste, rationale; Epic 2).
//!
//! This file keeps the shared trunk: [`JournalState`] itself, startup open/create, the plain
//! accessors, and the small helpers several submodules lean on.

use std::path::{Path, PathBuf};

use steadyinvest_contract::{ForecastLowOption, Judgment, Timestamp};
use steadyinvest_persistence::{Error as PersistError, Journal, clear_lock, lock_is_stale};
use uuid::Uuid;

use crate::clock::{Clock, IdGen};

mod cells;
mod concentration;
mod confront;
mod export_import;
mod fx;
mod holdings;
mod journal_io;
mod ledger;
mod messages;
mod refresh;
mod replacement;
mod restore;
mod studies;
mod undo;
mod watchlist;

#[cfg(test)]
mod tests;

pub use cells::*;
pub use concentration::*;
pub use confront::*;
pub(crate) use holdings::effective_currency;
pub use journal_io::*;
pub use messages::*;
pub use refresh::*;
pub use replacement::*;
pub use restore::*;
pub use undo::*;

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
        ttm_eps: None,
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
    /// A validated backup parked awaiting confirmation (Story 5.4): a restore is **never applied
    /// silently** (FR61) — `request_restore` parks the candidate, `confirm_restore` applies it.
    pending_restore: Option<PendingRestore>,
    /// The user-selected **active** portfolio (Story 6.1, FR37). `None` = use the first portfolio
    /// (deterministic). `main.rs` loads it from / persists it to `AppConfig.active_portfolio_id`; it
    /// is in-memory here (validated against the live portfolio list by [`Self::active_portfolio`]).
    active_portfolio_id: Option<Uuid>,
}

/// The result of opening/creating/switching a journal (Story 5.5) — the identity + version the caller
/// records in the recent-journals pointer, and whether a sync-folder warning applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOutcome {
    pub journal_id: Uuid,
    pub logical_version: u64,
    /// `true` when the journal lives in a detected sync folder and was opened in the sync-safe
    /// (`DELETE`) mode — the UI surfaces the warning + the recommended pattern (ADD8).
    pub sync_warning: bool,
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
        if let Some(path) = configured
            && path.exists()
        {
            // Story 5.5: a STALE lock (left by a crashed prior run — no live owner) on the
            // configured journal is auto-reclaimed at startup, so a post-crash relaunch reopens the
            // user's own journal rather than failing `LockHeld` and orphaning it onto the default. A
            // LIVE lock (a genuine second instance) is not stale → left intact → the open refuses.
            if lock_is_stale(path) {
                let _ = clear_lock(path);
            }
            match Journal::open_with_mode(path, sync_mode_for(path)) {
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
                            pending_restore: None,
                            active_portfolio_id: None,
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
                    pending_restore: None,
                    active_portfolio_id: None,
                },
                Some(MSG_NO_DATA_DIR.to_string()),
            );
        };

        let result = if path.exists() {
            Journal::open_with_mode(&path, sync_mode_for(&path))
        } else {
            if let Some(parent) = path.parent()
                && let Err(error) = std::fs::create_dir_all(parent)
            {
                tracing::warn!("data dir {} not created: {error}", parent.display());
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
                        pending_restore: None,
                        active_portfolio_id: None,
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
                        pending_restore: None,
                        active_portfolio_id: None,
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

    /// The open journal's identity (UUID), or `None` when no journal is open. Used to name a
    /// whole-journal export file (Story 5.3).
    pub fn journal_id(&self) -> Option<Uuid> {
        self.journal.as_ref().map(|j| j.id())
    }

    /// The open journal's monotonic `logical_version`, or `0` when no journal is open / unreadable
    /// (Story 5.5) — for the recent-journals last-seen pointer.
    pub fn logical_version_or_zero(&self) -> u64 {
        self.journal
            .as_ref()
            .and_then(|j| j.logical_version().ok())
            .unwrap_or(0)
    }

    /// The app's "now" from the injected [`Clock`] (ADD15) — the single wall-clock source. Used by
    /// the holdings price-refresh (Story 4.4) to stamp the transient per-ticker `as_of` freshness,
    /// so tests pin it deterministically via the `FixedClock` double.
    pub fn now(&self) -> Timestamp {
        self.clock.now()
    }

    /// The journal's current logical version — test-only, to prove a true no-op writes nothing
    /// (Story 3.4: a resolve with no pending must not bump the version / append an FR51 revision).
    #[cfg(test)]
    pub fn logical_version(&self) -> u64 {
        self.journal
            .as_ref()
            .and_then(|j| j.logical_version().ok())
            .unwrap_or(0)
    }
}

/// A neutral RFC3339 timestamp rendered for the dashboard list: the date portion only (the time of
/// day is not meaningful in the v1 list). A non-RFC3339 string passes through unchanged — this is a
/// display transform, it never repairs a value.
pub fn created_at_date(ts: &Timestamp) -> String {
    ts.0.split('T').next().unwrap_or(&ts.0).to_string()
}

/// Map a persistence error from a watchlist write to a neutral notice (Story 4.1): a newer-schema
/// journal reads as read-only, anything else as the generic save-failure (cause appended).
fn watch_error(error: PersistError) -> String {
    match error {
        PersistError::NewerJournalSchema { .. } => MSG_READ_ONLY_WRITE.to_string(),
        other => format!("{MSG_SAVE_FAILED} {other}"),
    }
}
