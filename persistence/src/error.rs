//! Per-crate error discipline (ADD15): cause-named variants via `thiserror`, a crate-wide
//! [`Result`] alias, and **neutral, fact-stating messages** (FR13 posture — no imperative
//! action/recommendation verb, no advice). A crate-local posture test at the bottom gates the
//! wording, following the `core::golden`-local pattern from Story 1.9.

use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

/// Crate-wide result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong inside `steadyinvest-persistence`.
///
/// Messages are user-facing (the app surfaces them verbatim later), so they state facts in a
/// neutral voice: cause, observed value, expected value — never advice.
#[derive(Debug, Error)]
pub enum Error {
    /// Underlying SQLite (or file I/O surfaced through SQLite) failure.
    #[error("sqlite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// `Journal::create` refuses to overwrite an existing file.
    #[error("a file already exists at {}; create only writes new journal files", .0.display())]
    JournalExists(PathBuf),

    /// A stored blob (or one of its extracted columns) did not parse back into its contract type.
    #[error("stored payload did not parse as its contract type: {detail}")]
    CorruptPayload { detail: String },

    /// The `journal_meta` singleton row is absent or holds invalid data.
    #[error("journal_meta singleton row is missing or invalid: {detail}")]
    CorruptJournalMeta { detail: String },

    /// The journal file's SQL schema (`PRAGMA user_version`) is newer than this build knows.
    /// The journal is opened read-only (NFR-R3); write methods return this same variant.
    #[error(
        "this journal was written by a newer schema (file user_version {file_user_version}, \
         this build supports up to {supported}); it is opened read-only"
    )]
    NewerJournalSchema {
        file_user_version: i64,
        supported: u32,
    },

    /// A single row carries a `schema_version` newer than the contract this build was built with.
    /// The read fails loudly — never a silent partial parse.
    #[error(
        "this row was written by a newer data contract (row schema_version {row_schema_version}, \
         this build supports up to {supported}); it is not readable by this build"
    )]
    NewerRowSchema {
        row_schema_version: i64,
        supported: u32,
    },

    /// A `Study` whose `journal_id` differs from the open journal's identity cannot be written
    /// into it (identity integrity, ADD6).
    #[error(
        "study journal_id {study_journal_id} differs from the open journal's id {journal_id}; \
         the write did not happen"
    )]
    JournalIdentityMismatch {
        study_journal_id: Uuid,
        journal_id: Uuid,
    },

    /// A migration step failed; the file stays at its previous `user_version` (each step is its
    /// own transaction).
    #[error("migration to user_version {version} failed: {source}")]
    Migration {
        version: u32,
        #[source]
        source: Box<Error>,
    },

    /// A whole-journal import file did not match its integrity fingerprint — corrupt or incomplete
    /// (Story 5.3, NFR-R5). Nothing was applied.
    #[error("the import file does not match its integrity fingerprint (corrupt or incomplete); nothing was applied")]
    ImportIntegrity,

    /// A whole-journal import file was written under a `schema_version` this build does not support
    /// (Story 5.3). Nothing was applied — no silent coercion.
    #[error(
        "the import file was written under an incompatible format version (found {found}, \
         this build supports {supported}); nothing was applied"
    )]
    ImportVersion { found: u32, supported: u32 },

    /// A whole-journal import file is not a valid export envelope, or its payload is not a valid
    /// journal snapshot (Story 5.3). Nothing was applied.
    #[error("the import file is not a valid journal export: {detail}; nothing was applied")]
    ImportMalformed { detail: String },

    /// The journal is already open in another instance (Story 5.5, ADD6 single-instance lock): its
    /// lock sidecar is held by process `pid`. The open did not happen.
    #[error("this journal is already open in another instance (process {pid}); it was not opened")]
    LockHeld { pid: u32 },

    /// The single-instance lock sidecar could not be acquired or released for a reason other than it
    /// already being held (Story 5.5) — e.g. the directory is not writable.
    #[error("the journal lock could not be managed: {detail}")]
    Lock { detail: String },

    /// A holding still referenced by transaction rows was not removed — their `holding_id` FK needs
    /// a live referent (a recorded sale soft-deletes instead; see `Journal::record_sell`).
    #[error("transaction rows still reference this holding; it was not removed")]
    HoldingHasTransactions,

    /// A restore-from-backup file operation failed at the staging/swap step (Story 5.4) — e.g. the
    /// backup could not be copied beside the journal. The live journal is unchanged.
    #[error("the backup file could not be staged: {detail}; the journal is unchanged")]
    Restore { detail: String },
}

impl From<steadyinvest_contract::ImportError> for Error {
    fn from(e: steadyinvest_contract::ImportError) -> Self {
        use steadyinvest_contract::ImportError as Ie;
        match e {
            Ie::Integrity => Error::ImportIntegrity,
            Ie::Version { found, supported } => Error::ImportVersion { found, supported },
            Ie::Malformed(detail) => Error::ImportMalformed { detail },
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::CorruptPayload {
            detail: e.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Crate-local copies of `core::method::BANNED_VERBS_EN/FR` (persistence does not depend on
    /// `core`; adding that edge just for two const arrays would widen the dependency graph and the
    /// `Cargo.lock` delta this story pins). Source of truth: `core/src/method/mod.rs` — keep in
    /// sync by hand; Story 2.14 centralizes the posture gate.
    const BANNED_VERBS_EN: [&str; 16] = [
        "buy",
        "sell",
        "hold",
        "purchase",
        "acquire",
        "dump",
        "exit",
        "enter",
        "trade",
        "invest",
        "divest",
        "recommend",
        "suggest",
        "should",
        "must",
        "ought to",
    ];
    const BANNED_VERBS_FR: [&str; 10] = [
        "acheter",
        "vendre",
        "conserver",
        "garder",
        "acquérir",
        "investir",
        "recommander",
        "suggérer",
        "devrait",
        "il faut",
    ];

    /// Same whole-word matcher as `core::golden` (1.9): `_` and any non-alphanumeric char is a
    /// word boundary, match is case-insensitive.
    fn contains_word(haystack: &str, needle: &str) -> bool {
        let h = haystack.to_lowercase();
        let n = needle.to_lowercase();
        h.match_indices(&n).any(|(i, _)| {
            let before_ok = i == 0
                || !h[..i]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric());
            let after = i + n.len();
            let after_ok = after == h.len()
                || !h[after..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric());
            before_ok && after_ok
        })
    }

    /// One sample of every variant — the inventory the posture gate walks. A new variant that is
    /// not added here is caught by the exhaustive `match` below.
    fn sample_errors() -> Vec<Error> {
        let sqlite_err = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            Some("database is locked".to_string()),
        );
        vec![
            Error::Sqlite(sqlite_err),
            Error::JournalExists(PathBuf::from("/tmp/journal.db")),
            Error::CorruptPayload {
                detail: "expected value at line 1 column 2".to_string(),
            },
            Error::CorruptJournalMeta {
                detail: "the singleton row is absent".to_string(),
            },
            Error::NewerJournalSchema {
                file_user_version: 9,
                supported: 1,
            },
            Error::NewerRowSchema {
                row_schema_version: 9,
                supported: 1,
            },
            Error::JournalIdentityMismatch {
                study_journal_id: Uuid::from_u128(1),
                journal_id: Uuid::from_u128(2),
            },
            Error::Migration {
                version: 2,
                source: Box::new(Error::CorruptJournalMeta {
                    detail: "x".to_string(),
                }),
            },
            Error::ImportIntegrity,
            Error::ImportVersion {
                found: 9,
                supported: 1,
            },
            Error::ImportMalformed {
                detail: "expected value at line 1 column 2".to_string(),
            },
            Error::LockHeld { pid: 4321 },
            Error::Lock {
                detail: "the directory is not writable".to_string(),
            },
            Error::HoldingHasTransactions,
            Error::Restore {
                detail: "the copy failed".to_string(),
            },
        ]
    }

    #[test]
    fn every_variant_is_in_the_posture_inventory() {
        // Exhaustive match: adding a variant without extending sample_errors() fails to compile
        // here, so the posture gate can never silently skip a new message.
        for e in sample_errors() {
            match e {
                Error::Sqlite(_)
                | Error::JournalExists(_)
                | Error::CorruptPayload { .. }
                | Error::CorruptJournalMeta { .. }
                | Error::NewerJournalSchema { .. }
                | Error::NewerRowSchema { .. }
                | Error::JournalIdentityMismatch { .. }
                | Error::Migration { .. }
                | Error::ImportIntegrity
                | Error::ImportVersion { .. }
                | Error::ImportMalformed { .. }
                | Error::LockHeld { .. }
                | Error::Lock { .. }
                | Error::HoldingHasTransactions
                | Error::Restore { .. } => {}
            }
        }
        assert_eq!(sample_errors().len(), 15, "one sample per variant");
    }

    #[test]
    fn error_messages_are_neutral_no_banned_verb() {
        for e in sample_errors() {
            let msg = e.to_string();
            for banned in BANNED_VERBS_EN.iter().chain(BANNED_VERBS_FR.iter()) {
                assert!(
                    !contains_word(&msg, banned),
                    "persistence error message {msg:?} contains banned verb {banned:?} (FR13)"
                );
            }
        }
    }

    #[test]
    fn banned_word_matcher_is_whole_word() {
        // Sanity for the local matcher: "steadyinvest" does not whole-word-contain "invest".
        assert!(!contains_word("steadyinvest journal", "invest"));
        assert!(contains_word("you should retry", "should"));
    }
}
