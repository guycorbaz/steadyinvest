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

use rust_decimal::Decimal;
use steadyinvest_contract::{ForecastLowOption, Judgment, Timestamp};
use steadyinvest_persistence::{
    Error as PersistError, Journal, ReadOnlyCause, clear_lock, lock_is_stale,
};
use uuid::Uuid;

use crate::clock::{Clock, IdGen};
use crate::viewmodel::format::{NumberFormat, NumberReading};
use steadyinvest_contract::Money;

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
mod review;
mod studies;
mod undo;
mod watchlist;

#[cfg(test)]
mod tests;

pub use cells::*;
pub use concentration::*;
pub use confront::*;
pub use export_import::ImportRequest;
pub use holdings::StudyChoice;
pub(crate) use holdings::{StopBasis, effective_currency, stop_basis};
pub use journal_io::*;
pub use messages::*;
pub use refresh::*;
pub use replacement::*;
pub use restore::*;
pub use review::*;
pub use undo::*;
pub(crate) use watchlist::same_ticker;

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
    /// Why the open journal is read-only (a newer-schema file, or a file / directory protected
    /// against writing), `None` when it is writable: writes are refused up front, by that cause.
    read_only: Option<ReadOnlyCause>,
    clock: Box<dyn Clock>,
    idgen: Box<dyn IdGen>,
    /// Undo/redo history for the currently-open study (Story 2.9). Reset on open.
    history: UndoHistory,
    /// A validated backup parked awaiting confirmation (Story 5.4): a restore is **never applied
    /// silently** (FR61) — `request_restore` parks the candidate, `confirm_restore` applies it.
    pending_restore: Option<PendingRestore>,
    /// A validated whole-journal envelope parked awaiting confirmation (issue #65): an OLDER
    /// same-journal import is a version regression the user must confirm —
    /// `request_import_journal` parks the text, `confirm_import_journal` applies it.
    pending_import: Option<String>,
    /// The user-selected **active** portfolio (Story 6.1, FR37). `None` = use the first portfolio
    /// (deterministic). `main.rs` loads it from / persists it to `AppConfig.active_portfolio_id`; it
    /// is in-memory here (validated against the live portfolio list by [`Self::active_portfolio`]).
    active_portfolio_id: Option<Uuid>,
    /// The user's number format (G1 I, #237), pushed by the wiring from app-config at startup and
    /// on every Réglages change: the rails read every user-typed amount through
    /// [`crate::viewmodel::format::parse_decimal`] under it (« 10,5 » under the comma format).
    number_format: NumberFormat,
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
    /// `true` when the target was the journal ALREADY open (re-selecting it is a no-op): the
    /// dossier did not change, so the session it carries must not be reset (G1 G review).
    pub unchanged: bool,
}

/// A user-typed amount under the user's number format (G1 I review): its value, or the named
/// refusal — `not_a_number` (the rail's own message) for a blank field or a text that is no number,
/// the format's ambiguous-number message for a number in another spelling. Never a guess.
pub(crate) fn read_typed(
    input: &str,
    format: NumberFormat,
    not_a_number: &str,
) -> Result<Decimal, String> {
    match crate::viewmodel::format::read_number(input, format) {
        NumberReading::Value(d) => Ok(d),
        NumberReading::Ambiguous => Err(ambiguous_number_message(format).to_string()),
        NumberReading::Blank | NumberReading::NotANumber => Err(not_a_number.to_string()),
    }
}

/// A user-typed study entry (cell, judgment field) under the user's number format (G1 I review):
/// blank → `None` (the field is cleared — an explicit gap); a number → its value; a text that is no
/// number, or an ambiguous one, is REFUSED with its reason — never turned into an empty hole.
pub fn typed_entry(input: &str, format: NumberFormat) -> Result<Option<Money>, String> {
    match crate::viewmodel::format::read_number(input, format) {
        NumberReading::Blank => Ok(None),
        NumberReading::Value(d) => Ok(Some(Money::from(d))),
        NumberReading::Ambiguous => Err(ambiguous_number_message(format).to_string()),
        NumberReading::NotANumber => Err(MSG_VALUE_NOT_A_NUMBER.to_string()),
    }
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
        let (state, notice) = Self::open_or_create_inner(configured, clock, idgen);
        // G1 P review (L-f): a `-prerestore` beside the open dossier (a restore whose rollback
        // failed) is named at startup — it may be the only copy of an original.
        let leftover = state
            .path
            .as_deref()
            .map(|live| path_with_suffix(live, "-prerestore"))
            .filter(|snapshot| std::fs::symlink_metadata(snapshot).is_ok())
            .map(|snapshot| prerestore_found_message(&snapshot));
        let notice = match (notice, leftover) {
            (Some(first), Some(second)) => Some(format!("{first} {second}")),
            (first, second) => first.or(second),
        };
        (state, notice)
    }

    fn open_or_create_inner(
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
                    let read_only = journal.read_only_cause();
                    return (
                        Self {
                            journal: Some(journal),
                            path: Some(path.to_path_buf()),
                            read_only,
                            clock,
                            idgen,
                            history: UndoHistory::default(),
                            pending_restore: None,
                            pending_import: None,
                            active_portfolio_id: None,
                            number_format: NumberFormat::default(),
                        },
                        read_only.map(|cause| read_only_notice(cause).to_string()),
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
                    read_only: None,
                    clock,
                    idgen,
                    history: UndoHistory::default(),
                    pending_restore: None,
                    pending_import: None,
                    active_portfolio_id: None,
                    number_format: NumberFormat::default(),
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
                let read_only = journal.read_only_cause();
                (
                    Self {
                        journal: Some(journal),
                        path: Some(path),
                        read_only,
                        clock,
                        idgen,
                        history: UndoHistory::default(),
                        pending_restore: None,
                        pending_import: None,
                        active_portfolio_id: None,
                        number_format: NumberFormat::default(),
                    },
                    read_only.map(|cause| read_only_notice(cause).to_string()),
                )
            }
            Err(error) => {
                tracing::warn!("default journal {} unavailable: {error}", path.display());
                (
                    Self {
                        journal: None,
                        path: None,
                        read_only: None,
                        clock,
                        idgen,
                        history: UndoHistory::default(),
                        pending_restore: None,
                        pending_import: None,
                        active_portfolio_id: None,
                        number_format: NumberFormat::default(),
                    },
                    Some(open_error(error)),
                )
            }
        }
    }

    /// The resolved on-disk path of the open journal, for persisting into app-config.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Set the user's number format the rails read typed amounts under (G1 I).
    pub fn set_number_format(&mut self, format: NumberFormat) {
        self.number_format = format;
    }

    /// The user's number format (G1 I): the rails' reading of typed amounts, and the spelling of
    /// the figures the wiring bakes from this state.
    pub fn number_format(&self) -> NumberFormat {
        self.number_format
    }

    /// Read a user-typed amount under the user's number format (G1 I) — see [`read_typed`].
    pub(crate) fn read_typed(&self, input: &str, not_a_number: &str) -> Result<Decimal, String> {
        read_typed(input, self.number_format, not_a_number)
    }

    /// True when the open journal is read-only (newer-schema file, or write-protected).
    pub fn is_read_only(&self) -> bool {
        self.read_only.is_some()
    }

    /// The open dossier's read-only STATE line (the startup banner, the location status), by
    /// cause — `None` when it is writable.
    pub fn read_only_notice(&self) -> Option<&'static str> {
        self.read_only.map(read_only_notice)
    }

    /// The read-only cause as the stable key the Slint read-only bands select their wording by
    /// (`Holdings.read-only-cause`): `"newer-schema"`, `"file"`, `"directory"`, or `""` when the
    /// dossier is writable. A key, never display text.
    pub fn read_only_cause_key(&self) -> &'static str {
        match self.read_only {
            None => "",
            Some(ReadOnlyCause::NewerSchema { .. }) => "newer-schema",
            Some(ReadOnlyCause::FileWriteProtected) => "file",
            Some(ReadOnlyCause::DirectoryWriteProtected) => "directory",
        }
    }

    /// The up-front refusal of a rail that would WRITE the open journal (G1 G, on-screen check):
    /// on a read-only journal, « Restaurer une sauvegarde… » and « Importer un dossier… » refuse
    /// at once with the reason — before any file picker or confirm (the write guards behind them
    /// stay as second guards). Every write rail opens with this guard, so each refusal names the
    /// cause actually in force (a protected file is not a newer schema).
    pub fn refuse_if_read_only(&self) -> Result<(), &'static str> {
        match self.read_only {
            Some(cause) => Err(read_only_refusal(cause)),
            None => Ok(()),
        }
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

/// A failed READ on a write rail (G1 P): named as a read failure — never « L'enregistrement a
/// échoué. » for a write that was never attempted. The persistence error's text is logged.
fn read_error(error: PersistError) -> String {
    tracing::warn!("journal read failed: {error}");
    MSG_READ_FAILED.to_string()
}

/// The open dossier's read-only state line, by cause (startup banner, location status).
pub(crate) fn read_only_notice(cause: ReadOnlyCause) -> &'static str {
    match cause {
        ReadOnlyCause::NewerSchema { .. } => MSG_STARTUP_READ_ONLY,
        ReadOnlyCause::FileWriteProtected => MSG_STARTUP_FILE_PROTECTED,
        ReadOnlyCause::DirectoryWriteProtected => MSG_STARTUP_DIR_PROTECTED,
    }
}

/// The up-front refusal of a write on a read-only dossier, by cause.
pub(crate) fn read_only_refusal(cause: ReadOnlyCause) -> &'static str {
    match cause {
        ReadOnlyCause::NewerSchema { .. } => MSG_READ_ONLY_WRITE,
        ReadOnlyCause::FileWriteProtected => MSG_READ_ONLY_FILE_WRITE,
        ReadOnlyCause::DirectoryWriteProtected => MSG_READ_ONLY_DIR_WRITE,
    }
}

/// Map a persistence error from a WRITE to its French refusal — the one mapping every write rail
/// shares. The read-only gates name their cause (newer schema, protected file, protected
/// directory); a write the OS refused on the spot (SQLite's READONLY / PERM — the protection
/// changed after the open) names that; anything else is the plain « L'enregistrement a échoué. ».
/// The persistence error's own text — SQLite's English above all — is LOGGED, never appended to
/// the French (2026-09-26 on-screen defect: « … sqlite operation failed: attempt to write a
/// readonly database » reached the refusal dialog).
pub(crate) fn save_error(error: PersistError) -> String {
    match error {
        PersistError::NewerJournalSchema { .. } => MSG_READ_ONLY_WRITE.to_string(),
        PersistError::WriteProtected { directory: false } => MSG_READ_ONLY_FILE_WRITE.to_string(),
        PersistError::WriteProtected { directory: true } => MSG_READ_ONLY_DIR_WRITE.to_string(),
        other if other.is_write_protected() => {
            tracing::warn!("journal write refused by the system: {other}");
            MSG_WRITE_REFUSED_BY_SYSTEM.to_string()
        }
        other => {
            tracing::warn!("journal write failed: {other}");
            with_cause(
                MSG_SAVE_FAILED,
                MSG_SAVE_FAILED_CAUSE,
                persist_cause(&other),
            )
        }
    }
}

/// The French cause of a persistence failure, from its TYPED kind — `None` when no named cause
/// applies (the message then says only what failed; the detail is in the log).
pub(crate) fn persist_cause(error: &PersistError) -> Option<&'static str> {
    use steadyinvest_persistence::ErrorKind as K;
    match error.kind() {
        K::WriteProtected => Some(MSG_CAUSE_PROTECTED),
        K::Locked => Some(MSG_CAUSE_LOCKED),
        K::Corrupt => Some(MSG_CAUSE_CORRUPT),
        K::DiskFull => Some(MSG_CAUSE_DISK_FULL),
        K::Missing => Some(MSG_CAUSE_MISSING),
        K::NewerData => Some(MSG_CAUSE_NEWER_DATA),
        K::Migration => Some(MSG_CAUSE_MIGRATION),
        K::Other => None,
    }
}

/// The French cause of a file-system failure, from its `io::ErrorKind`.
fn io_cause(error: &std::io::Error) -> Option<&'static str> {
    use std::io::ErrorKind as K;
    match error.kind() {
        K::PermissionDenied | K::ReadOnlyFilesystem => Some(MSG_CAUSE_PROTECTED),
        K::StorageFull => Some(MSG_CAUSE_DISK_FULL),
        K::NotFound => Some(MSG_CAUSE_MISSING),
        _ => None,
    }
}

/// `plain` when no cause is named, else `template` with its `{cause}` filled.
fn with_cause(plain: &str, template: &str, cause: Option<&str>) -> String {
    match cause {
        Some(cause) => template.replace("{cause}", cause),
        None => plain.to_string(),
    }
}

/// A failed READ of `subject` (one of the `MSG_SUBJECT_*`), named in French with its cause when
/// one applies — the persistence text logged only (2026-09-26: the read rails returned the
/// English Display, which their callers could put in a refusal or a notice).
pub(crate) fn read_failure(subject: &'static str, error: PersistError) -> String {
    tracing::warn!("journal read failed ({subject}): {error}");
    let message = match persist_cause(&error) {
        Some(cause) => MSG_READ_SUBJECT_FAILED_CAUSE.replace("{cause}", cause),
        None => MSG_READ_SUBJECT_FAILED.to_string(),
    };
    message.replace("{what}", subject)
}

/// A dossier that could not be opened, named in French with its cause (2026-09-26: the open
/// notice appended the persistence Display). The lock held by another instance and the
/// protected-and-outdated file keep their own messages (their callers match them first).
pub(crate) fn open_error(error: PersistError) -> String {
    tracing::warn!("journal open failed: {error}");
    match error {
        PersistError::LockHeld { .. } => MSG_JOURNAL_LOCKED.to_string(),
        PersistError::WriteProtectedOutdated { .. } => MSG_OPEN_PROTECTED_OUTDATED.to_string(),
        other => with_cause(
            MSG_JOURNAL_OPEN_FAILED,
            MSG_JOURNAL_OPEN_FAILED_CAUSE,
            persist_cause(&other),
        ),
    }
}

/// Map a file-system error from a WRITE beside the dossier (a backup copy) to its French refusal:
/// the OS's write refusals (permission, read-only file system) are named, a full disk or a
/// missing folder too, anything else is the plain save failure. The OS text is logged, never shown.
pub(crate) fn io_save_error(error: std::io::Error) -> String {
    tracing::warn!("file write failed: {error}");
    match error.kind() {
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem => {
            MSG_WRITE_REFUSED_BY_SYSTEM.to_string()
        }
        _ => with_cause(MSG_SAVE_FAILED, MSG_SAVE_FAILED_CAUSE, io_cause(&error)),
    }
}

/// Map a persistence error from a watchlist / holdings write to a neutral notice (Story 4.1): a
/// holding still referenced by transactions names that cause (G1 final review — matched on the
/// TYPED variant, never on its text); everything else goes through [`save_error`] (the read-only
/// causes named, the English text logged, never appended — G1 final review: a raw `transaction
/// rows still reference…` under « L'enregistrement a échoué. » was no cause the user could read).
fn watch_error(error: PersistError) -> String {
    match error {
        PersistError::HoldingHasTransactions => MSG_HOLDING_HAS_TRANSACTIONS.to_string(),
        other => save_error(other),
    }
}
