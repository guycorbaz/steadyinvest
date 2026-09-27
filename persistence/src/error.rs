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

    /// The pre-restore snapshot could not be written under the restore lock (Story 8.3 G3 N3). The
    /// live journal is unchanged and no partial snapshot is left.
    #[error("the pre-restore snapshot could not be written: {detail}; the journal is unchanged")]
    RestoreSnapshot { detail: String },

    /// A protected journal is read through a private copy (its unconsolidated writes cannot be
    /// read in place without creating files beside it); that copy could not be prepared. `cause`
    /// is the file-system error's kind (a full disk is named as such).
    #[error("the private read copy of the protected journal could not be prepared: {detail}")]
    ReadCopy {
        detail: String,
        cause: std::io::ErrorKind,
    },

    /// The protected journal changed while it was being copied for reading (another account
    /// writing it — a directory we cannot write holds no lock), twice in a row. Nothing was read.
    #[error("the journal changed while it was being copied for reading; it was not opened")]
    ChangedDuringCopy,

    /// The journal's `-wal` / `-shm` is not writable by this account and could not be made so;
    /// the journal is opened read-only and write methods return this variant.
    #[error("a side file of the journal (-wal / -shm) is not writable; it is opened read-only")]
    SidecarNotWritable,

    /// A backup file could not be written, flushed or named. `cause` is the file-system error's
    /// kind (`AlreadyExists` when the name is taken — the caller picks another).
    #[error("the backup could not be written: {detail}")]
    Backup {
        detail: String,
        cause: std::io::ErrorKind,
    },

    /// A draft decision named a draft the dossier does not hold (Story 8.2b). Nothing was written.
    /// Its kind is `Other` — a missing draft row is never « file not found » (G3 B2/E7).
    #[error("draft {id} is not in the journal; nothing was written")]
    DraftNotFound { id: Uuid },

    /// A decision was asked for a draft that is no longer pending — it was decided meanwhile
    /// (Story 8.2b). `status` is the stored status (its snake-case spelling). Nothing was written.
    #[error("draft is already {status}, not pending; nothing was written")]
    DraftNotPending { status: String },

    /// An undo/redo step of a decision found the draft in another status than the step expects
    /// (Story 8.2b). Nothing was written.
    #[error("draft status is {found}, the step expected {expected}; nothing was written")]
    DraftStatusMismatch { expected: String, found: String },

    /// The stored study is no longer the one a decision was computed from — another write landed
    /// in between (Story 8.2b, arch A7/A8: the re-check inside the decision transaction). Nothing
    /// was written.
    #[error("the study changed since the decision was prepared; nothing was written")]
    StudyChangedSinceRead,

    /// A decision tried to write a study other than the draft's own (Story 8.2b G3 F9) — an
    /// internal inconsistency of the caller. Nothing was written.
    #[error("the decision's study {study_id} is not the draft's study; nothing was written")]
    DraftStudyMismatch { study_id: Uuid },

    /// The MCP access surface met a dossier whose SQL schema is not exactly this build's (Story 8.3,
    /// arch A2): it never migrates and never reads a schema it does not know. Nothing was read or
    /// written. `file_user_version` above `supported` = a newer dossier; below = an older one.
    #[error(
        "the dossier schema (file user_version {file_user_version}) differs from this MCP \
         access build's ({supported}); nothing was read or written"
    )]
    McpSchemaMismatch {
        file_user_version: i64,
        supported: u32,
    },

    /// The SQLite authorizer of the MCP access surface denied a statement (Story 8.3, arch A3):
    /// only study reads and draft inserts pass. `denials` names each denied action and its object,
    /// for the MCP server's log. Nothing was written.
    #[error("the MCP access surface denied {}; nothing was written", crate::mcp_access::denials_text(.denials))]
    McpDenied {
        denials: Vec<crate::mcp_access::McpDenial>,
    },

    /// A call to the MCP access surface was not well formed — a caller (MCP server) defect, never
    /// an AI proposal's fault (those are typed refusals): e.g. a draft kind without the data it
    /// needs, or a timestamp that is not RFC3339 UTC. Nothing was written.
    #[error("the MCP access call is not well formed: {detail}; nothing was written")]
    McpInvalidCall { detail: String },

    /// The MCP access surface cannot use the dossier at all (Story 8.3 G3): missing, not a journal,
    /// being restored, needing the app's recovery, protected, or of unreadable identity — each named
    /// by its [`McpUnavailable`](crate::mcp_access::McpUnavailable) reason. Nothing was read or
    /// written.
    #[error("the MCP access surface cannot use the dossier: {reason}; nothing was read or written")]
    McpUnavailable {
        reason: crate::mcp_access::McpUnavailable,
    },
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
    /// A protected journal's private read copy could not be prepared.
    ReadCopy,
    /// The journal changed while being copied for reading.
    ChangedDuringCopy,
    /// The database file was replaced or moved while open (SQLite READONLY_DBMOVED — a sync
    /// tool swapping the file, G3 L1): its writes are refused until it is reopened.
    Replaced,
    /// A schema update of the file failed.
    Migration,
    /// No named cause.
    Other,
}

/// The named kind of a file-system failure (second G3 M-f: a full disk is `DiskFull` wherever it
/// happens), `Other` when none applies.
fn io_kind(kind: std::io::ErrorKind) -> ErrorKind {
    use std::io::ErrorKind as K;
    match kind {
        K::StorageFull | K::QuotaExceeded => ErrorKind::DiskFull,
        K::PermissionDenied | K::ReadOnlyFilesystem => ErrorKind::WriteProtected,
        K::NotFound => ErrorKind::Missing,
        _ => ErrorKind::Other,
    }
}

impl Error {
    /// This failure's [`ErrorKind`].
    pub fn kind(&self) -> ErrorKind {
        use rusqlite::ErrorCode as C;
        match self {
            Error::WriteProtected { .. } => ErrorKind::WriteProtected,
            Error::ReadCopy { cause, .. } => match io_kind(*cause) {
                ErrorKind::Other => ErrorKind::ReadCopy,
                named => named,
            },
            Error::Backup { cause, .. } => io_kind(*cause),
            Error::ChangedDuringCopy => ErrorKind::ChangedDuringCopy,
            Error::SidecarNotWritable => ErrorKind::WriteProtected,
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
            Error::McpSchemaMismatch {
                file_user_version,
                supported,
            } if *file_user_version > i64::from(*supported) => ErrorKind::NewerData,
            Error::McpUnavailable { reason } => {
                use crate::mcp_access::McpUnavailable as U;
                match reason {
                    U::Missing => ErrorKind::Missing,
                    U::NotADossier => ErrorKind::Corrupt,
                    U::RestoreInProgress | U::RestoreInterrupted => ErrorKind::Replaced,
                    U::Busy => ErrorKind::Locked,
                    U::Protected { .. } => ErrorKind::WriteProtected,
                    U::NeedsRecovery | U::IdentityUnreadable { .. } => ErrorKind::Other,
                }
            }
            Error::Migration { .. } => ErrorKind::Migration,
            _ => ErrorKind::Other,
        }
    }

    /// The stable MCP code of this failure (Story 8.3 G3 F5; Story 8.0 §3.3), `None` when it has no
    /// named MCP cause (the MCP server logs it and answers a generic failure).
    pub fn mcp_code(&self) -> Option<&'static str> {
        match self {
            Error::McpSchemaMismatch { .. } => Some("schema_mismatch"),
            Error::McpDenied { .. } => Some("write_denied"),
            Error::McpInvalidCall { .. } => Some("invalid_call"),
            Error::McpUnavailable { reason } => Some(reason.code()),
            _ => None,
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
            Error::RestoreSnapshot {
                detail: "the snapshot file could not be created".to_string(),
            },
            Error::ReadCopy {
                detail: "a file could not be copied".to_string(),
                cause: std::io::ErrorKind::StorageFull,
            },
            Error::ChangedDuringCopy,
            Error::SidecarNotWritable,
            Error::Backup {
                detail: "the partial file: exists".to_string(),
                cause: std::io::ErrorKind::AlreadyExists,
            },
            Error::DraftNotFound {
                id: Uuid::from_u128(3),
            },
            Error::DraftNotPending {
                status: "validated".to_string(),
            },
            Error::DraftStatusMismatch {
                expected: "validated".to_string(),
                found: "rejected".to_string(),
            },
            Error::StudyChangedSinceRead,
            Error::DraftStudyMismatch {
                study_id: Uuid::from_u128(4),
            },
            Error::McpSchemaMismatch {
                file_user_version: 9,
                supported: 8,
            },
            Error::McpDenied {
                denials: vec![crate::mcp_access::McpDenial {
                    action: "update".to_string(),
                    object: "studies.payload".to_string(),
                }],
            },
            Error::McpInvalidCall {
                detail: "a note draft carries no study".to_string(),
            },
            Error::McpUnavailable {
                reason: crate::mcp_access::McpUnavailable::Protected { directory: true },
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
                | Error::RestoreSnapshot { .. }
                | Error::ReadCopy { .. }
                | Error::ChangedDuringCopy
                | Error::SidecarNotWritable
                | Error::Backup { .. }
                | Error::DraftNotFound { .. }
                | Error::DraftNotPending { .. }
                | Error::DraftStatusMismatch { .. }
                | Error::StudyChangedSinceRead
                | Error::DraftStudyMismatch { .. }
                | Error::McpSchemaMismatch { .. }
                | Error::McpDenied { .. }
                | Error::McpInvalidCall { .. }
                | Error::McpUnavailable { .. } => {}
            }
        }
        // 31 variants (21 + the five draft-decision variants of Story 8.2b + the four MCP access
        // variants and `RestoreSnapshot` of Story 8.3); `WriteProtected` and `WriteProtectedOutdated`
        // are sampled for both of their causes (file, directory). 8.3 delta: 28 → 33.
        assert_eq!(
            sample_errors().len(),
            33,
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
        // Story 8.3: a newer dossier met by the MCP access surface is newer data; an older one has
        // no named kind (the MCP server names it « open it in the app first »).
        let mcp_schema = |file_user_version| Error::McpSchemaMismatch {
            file_user_version,
            supported: 8,
        };
        assert_eq!(mcp_schema(9).kind(), ErrorKind::NewerData);
        assert_eq!(mcp_schema(7).kind(), ErrorKind::Other);
        assert_eq!(
            Error::McpDenied { denials: vec![] }.kind(),
            ErrorKind::Other
        );
        use crate::mcp_access::McpUnavailable as U;
        let unavailable = |reason| Error::McpUnavailable { reason };
        assert_eq!(unavailable(U::Missing).kind(), ErrorKind::Missing);
        assert_eq!(unavailable(U::NotADossier).kind(), ErrorKind::Corrupt);
        assert_eq!(
            unavailable(U::RestoreInProgress).kind(),
            ErrorKind::Replaced
        );
        assert_eq!(
            unavailable(U::Protected { directory: false }).kind(),
            ErrorKind::WriteProtected
        );
        // F5: the MCP code of every MCP failure; none for the others.
        assert_eq!(mcp_schema(9).mcp_code(), Some("schema_mismatch"));
        assert_eq!(
            Error::McpDenied { denials: vec![] }.mcp_code(),
            Some("write_denied")
        );
        assert_eq!(
            unavailable(U::RestoreInProgress).mcp_code(),
            Some("dossier_busy")
        );
        assert_eq!(unavailable(U::Missing).mcp_code(), Some("no_dossier"));
        assert_eq!(Error::StudyChangedSinceRead.mcp_code(), None);
    }

    #[test]
    fn banned_word_matcher_is_whole_word() {
        // Sanity for the local matcher: "steadyinvest" does not whole-word-contain "invest".
        assert!(!contains_word("steadyinvest journal", "invest"));
        assert!(contains_word("you should retry", "should"));
    }
}
