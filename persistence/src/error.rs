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

    /// The journal file (or the directory holding it) is protected against writing at the OS level
    /// — a `chmod 444` file, a read-only directory or medium. The journal is opened read-only and
    /// write methods return this variant; `directory` names WHICH is protected (the file wins when
    /// both are), so the refusal never names the wrong cause.
    #[error(
        "the journal {} is protected against writing; it is opened read-only",
        if *.directory { "directory" } else { "file" }
    )]
    WriteProtected { directory: bool },

    /// A write-protected journal whose schema is OLDER than this build: the pending migrations
    /// cannot run on a file that cannot be written, and reading an unmigrated file with this build's
    /// queries would misread it — so the open is refused and the file stays untouched. `directory`
    /// names WHICH is protected (G3 M5: a writable file in a protected directory is not a
    /// protected file).
    #[error(
        "this journal's {} is protected against writing and its schema (file user_version \
         {file_user_version}) is older than this build's ({supported}); it was not opened",
        if *.directory { "directory" } else { "file" }
    )]
    WriteProtectedOutdated {
        file_user_version: i64,
        supported: u32,
        directory: bool,
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
    #[error(
        "the import file does not match its integrity fingerprint (corrupt or incomplete); nothing was applied"
    )]
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

    /// A protected journal is read through a private copy (its unconsolidated writes cannot be
    /// read in place without creating files beside it); that copy could not be prepared.
    #[error("the private read copy of the protected journal could not be prepared: {detail}")]
    ReadCopy { detail: String },
}

/// The KIND of a failure, for a caller that names causes in its own language (the app speaks
/// French and never shows this crate's — or SQLite's — English Display, 2026-09-26). A typed
/// classification, never a match on message text; `Other` when no named cause applies (the
/// technical detail then lives in the log only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The OS refuses the write: a protected file / directory / medium.
    WriteProtected,
    /// The journal is busy or locked by another access (SQLite BUSY/LOCKED, the instance lock).
    Locked,
    /// The file is damaged or is not a journal (SQLite CORRUPT/NOTADB, unparseable rows or meta).
    Corrupt,
    /// The disk is full (SQLite FULL).
    DiskFull,
    /// The file cannot be found or opened (SQLite CANTOPEN).
    Missing,
    /// A newer version of the app wrote the file or a row.
    NewerData,
    /// The journal is protected against writing and older than this build (its schema update
    /// cannot be written); `directory` names which is protected.
    ProtectedOutdated { directory: bool },
    /// The database file was replaced or moved while open (SQLite READONLY_DBMOVED — a sync
    /// tool swapping the file, G3 L1): its writes are refused until it is reopened.
    Replaced,
    /// A schema update of the file failed.
    Migration,
    /// No named cause.
    Other,
}

impl Error {
    /// This failure's [`ErrorKind`].
    pub fn kind(&self) -> ErrorKind {
        use rusqlite::ErrorCode as C;
        match self {
            Error::WriteProtected { .. } => ErrorKind::WriteProtected,
            Error::WriteProtectedOutdated { directory, .. } => ErrorKind::ProtectedOutdated {
                directory: *directory,
            },
            // Before the READONLY family: a moved/replaced file is not a protected one.
            Error::Sqlite(rusqlite::Error::SqliteFailure(code, _))
                if code.extended_code == rusqlite::ffi::SQLITE_READONLY_DBMOVED =>
            {
                ErrorKind::Replaced
            }
            Error::Sqlite(rusqlite::Error::SqliteFailure(code, _)) => match code.code {
                C::ReadOnly | C::PermissionDenied => ErrorKind::WriteProtected,
                C::DatabaseBusy | C::DatabaseLocked => ErrorKind::Locked,
                C::DatabaseCorrupt | C::NotADatabase => ErrorKind::Corrupt,
                C::DiskFull => ErrorKind::DiskFull,
                C::CannotOpen => ErrorKind::Missing,
                _ => ErrorKind::Other,
            },
            Error::LockHeld { .. } => ErrorKind::Locked,
            Error::CorruptPayload { .. } | Error::CorruptJournalMeta { .. } => ErrorKind::Corrupt,
            Error::NewerJournalSchema { .. } | Error::NewerRowSchema { .. } => ErrorKind::NewerData,
            Error::Migration { .. } => ErrorKind::Migration,
            _ => ErrorKind::Other,
        }
    }

    /// Whether this failure is the OS refusing a write — the API gate of a write-protected journal,
    /// or SQLite reporting a read-only database / a denied permission at write time (a file whose
    /// protection changed after it was opened). The app names this cause in French instead of
    /// surfacing SQLite's own English text (2026-09-26 on-screen defect).
    pub fn is_write_protected(&self) -> bool {
        self.kind() == ErrorKind::WriteProtected
    }
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
            Error::WriteProtected { directory: false },
            Error::WriteProtected { directory: true },
            Error::WriteProtectedOutdated {
                file_user_version: 3,
                supported: 9,
                directory: false,
            },
            Error::WriteProtectedOutdated {
                file_user_version: 3,
                supported: 9,
                directory: true,
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
            Error::ReadCopy {
                detail: "a file could not be copied".to_string(),
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
                | Error::WriteProtected { .. }
                | Error::WriteProtectedOutdated { .. }
                | Error::NewerRowSchema { .. }
                | Error::JournalIdentityMismatch { .. }
                | Error::Migration { .. }
                | Error::ImportIntegrity
                | Error::ImportVersion { .. }
                | Error::ImportMalformed { .. }
                | Error::LockHeld { .. }
                | Error::Lock { .. }
                | Error::HoldingHasTransactions
                | Error::Restore { .. }
                | Error::ReadCopy { .. } => {}
            }
        }
        // 18 variants; `WriteProtected` and `WriteProtectedOutdated` are sampled for both of
        // their causes (file, directory).
        assert_eq!(
            sample_errors().len(),
            20,
            "one sample per variant (+2 causes)"
        );
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
    fn write_protection_is_recognised_from_the_gate_and_from_sqlite() {
        let sqlite = |code| {
            Error::Sqlite(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(code),
                Some("attempt to write a readonly database".to_string()),
            ))
        };
        assert!(Error::WriteProtected { directory: true }.is_write_protected());
        assert!(sqlite(rusqlite::ffi::SQLITE_READONLY).is_write_protected());
        assert!(sqlite(rusqlite::ffi::SQLITE_PERM).is_write_protected());
        assert!(!sqlite(rusqlite::ffi::SQLITE_BUSY).is_write_protected());
        assert!(!Error::HoldingHasTransactions.is_write_protected());
    }

    #[test]
    fn every_failure_has_a_typed_kind() {
        let sqlite = |code| {
            Error::Sqlite(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(code),
                None,
            ))
        };
        use rusqlite::ffi;
        assert_eq!(sqlite(ffi::SQLITE_BUSY).kind(), ErrorKind::Locked);
        assert_eq!(sqlite(ffi::SQLITE_LOCKED).kind(), ErrorKind::Locked);
        assert_eq!(sqlite(ffi::SQLITE_CORRUPT).kind(), ErrorKind::Corrupt);
        assert_eq!(sqlite(ffi::SQLITE_NOTADB).kind(), ErrorKind::Corrupt);
        assert_eq!(sqlite(ffi::SQLITE_FULL).kind(), ErrorKind::DiskFull);
        assert_eq!(sqlite(ffi::SQLITE_CANTOPEN).kind(), ErrorKind::Missing);
        assert_eq!(
            sqlite(ffi::SQLITE_READONLY).kind(),
            ErrorKind::WriteProtected
        );
        assert_eq!(sqlite(ffi::SQLITE_ERROR).kind(), ErrorKind::Other);
        assert_eq!(Error::LockHeld { pid: 1 }.kind(), ErrorKind::Locked);
        assert_eq!(
            Error::CorruptPayload { detail: "x".into() }.kind(),
            ErrorKind::Corrupt
        );
        assert_eq!(
            Error::NewerRowSchema {
                row_schema_version: 9,
                supported: 1
            }
            .kind(),
            ErrorKind::NewerData
        );
        assert_eq!(Error::HoldingHasTransactions.kind(), ErrorKind::Other);
        let moved = Error::Sqlite(rusqlite::Error::SqliteFailure(
            ffi::Error::new(ffi::SQLITE_READONLY_DBMOVED),
            None,
        ));
        assert_eq!(moved.kind(), ErrorKind::Replaced);
        assert!(
            !moved.is_write_protected(),
            "a replaced file is not a protected one"
        );
        assert_eq!(
            Error::WriteProtectedOutdated {
                file_user_version: 1,
                supported: 9,
                directory: true
            }
            .kind(),
            ErrorKind::ProtectedOutdated { directory: true }
        );
    }

    #[test]
    fn banned_word_matcher_is_whole_word() {
        // Sanity for the local matcher: "steadyinvest" does not whole-word-contain "invest".
        assert!(!contains_word("steadyinvest journal", "invest"));
        assert!(contains_word("you should retry", "should"));
    }
}
