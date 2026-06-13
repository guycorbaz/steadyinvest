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

use steadyinvest_contract::{ForecastLowOption, Judgment, Study, Timestamp};
use steadyinvest_persistence::{Error as PersistError, Journal, StudySummary};
use uuid::Uuid;

use crate::clock::{Clock, IdGen};

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
];

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
}
