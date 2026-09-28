//! `McpAccess` — the gated access surface of the MCP server (Story 8.3, arch §Phase 4 A2 / A3 /
//! A11, NFR-A1–A4).
//!
//! **What it is.** The ONLY persistence entry point the `steadyinvest-mcp` binary (Story 8.4) gets:
//! typed methods that read studies and insert drafts — never a [`rusqlite::Connection`], never a
//! [`Journal`](crate::Journal). Whoever holds a connection can remove its authorizer, so no public
//! signature here yields one (the clippy boundary of the `mcp` crate lands with that crate, 8.4).
//!
//! **Capability asymmetry by construction (A3).** Every call opens its own connection(s) and
//! installs a SQLite **authorizer** before preparing anything:
//! - the **read** connection (`SQLITE_OPEN_READ_ONLY`) may read [`MCP_READABLE_TABLES`] and
//!   SQLite's catalogue only — an **allowlist**: the portfolio, the watchlist, the caches and any
//!   table a later migration adds are `SQLITE_DENY` (never `SQLITE_IGNORE`, which would read NULLs
//!   silently);
//! - the **draft** connection may in addition `INSERT` into `ai_drafts`, and `UPDATE
//!   journal_meta.logical_version` **only** from the v8 trigger [`DRAFT_TRIGGER`] (so no statement
//!   can set the counter to an arbitrary value). Every other action — UPDATE / DELETE, DDL, `ATTACH`
//!   (which also covers `VACUUM` / `VACUUM INTO`), `PRAGMA` after setup… — is denied by the engine,
//!   at statement preparation (or, for the ATTACH behind `VACUUM`, when the statement runs), and
//!   **reported** as a typed [`Error::McpDenied`] carrying each [`McpDenial`], for the MCP server's
//!   log (this crate has no logger). An `INSERT OR REPLACE` that would overwrite an existing draft
//!   (the authorizer cannot see a conflict clause) is refused by the v9 trigger
//!   `trg_ai_drafts_refuse_existing_id` (Story 8.3 G3).
//!
//! **Per call, lock-free, version-gated (A2).** A call opens the dossier, works and closes it. It
//! never takes the app's single-instance lock, never migrates, never sets `journal_mode`; it sets
//! only `busy_timeout` and `foreign_keys = ON`, and refuses a dossier whose `user_version` is not
//! exactly this build's ([`Error::McpSchemaMismatch`]). It refuses by name ([`McpUnavailable`]) a
//! missing, empty or non-journal file, a dossier being restored, one that needs the app's recovery
//! first (a hot rollback journal), and a protected one it could only read by creating side files:
//! - a read on a **writable** WAL dossier leaves an empty `-wal` and a `-shm` beside it (SQLite
//!   needs them to read WAL, and a read-only connection cannot remove them) — expected and
//!   harmless: they carry the file's own (writable) mode and the app's next open uses them;
//! - a **write-protected file** with no content-holding side file is read in place, `mode=ro&
//!   immutable=1` — nothing is created beside it (the app's own rule, `Journal::open`);
//! - a protected file WITH unconsolidated writes in a side file, or a file in a protected
//!   **directory**, is refused by name ([`McpUnavailable::Protected`]) — reading it would create
//!   `r--r--r--` side files, or fail; the app reads such a dossier through a private copy.
//!
//! **One file, no stray descriptor (A11, SQLite "how to corrupt" §2.2).** On POSIX systems, closing
//! ANY descriptor of a database file releases every lock this process holds on it — so this module
//! never opens a descriptor of the dossier besides SQLite's own. The file identity a submission
//! re-checks is a `stat(2)` (device + inode) taken BEFORE the connection opens, plus SQLite's own
//! `SQLITE_FCNTL_HAS_MOVED` on the connection's file; on Windows (no such hazard, no
//! `HAS_MOVED`) it is a `same_file::Handle` opened before the connection and dropped after it.
//!
//! **Identity is the caller's (ADD15).** Nothing here calls a clock or a UUID generator: a draft's
//! id, its `created_at` and the method version its fingerprint is computed under come from the
//! caller.
//!
//! **Submissions (A3, D2 / D4 / D6 / D8 / D10, A11).** A draft's checks and its insert run in ONE
//! `BEGIN IMMEDIATE` transaction: a refused draft writes nothing and is a typed
//! [`SubmissionRefusal`] with a stable snake-case [`code`](SubmissionRefusal::code) (the MCP server
//! renders the French message — Story 8.0 §3.3). Inside that transaction the file at the resolved
//! path is re-checked against the one the call opened (a restore swaps it — A11). Re-submitting the
//! same draft (same id, same content) is idempotent; the same id with other content is refused.

use crate::drafts::{
    DRAFT_COLUMNS, DraftRecord, check_payload, is_currency_code, is_rfc3339_utc, is_ticker,
    record_from_row, row_tuple,
};
use crate::error::{Error, Result};
use crate::journal::{apply_connection_local_pragmas, file_uri, read_journal_id, resolved_path};
use crate::migrations;
use crate::schema::{DRAFT_TRIGGER, MCP_READABLE_TABLES, SQLITE_INTERNAL_TABLES};
use crate::studies::{
    StudySummary, list_studies_in, parse_study_row, read_study_in, study_status_in,
};
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use steadyinvest_contract::{
    DRAFT_PAYLOAD_VERSION, DraftField, DraftFieldKind, DraftKind, DraftOrigin, DraftPayload,
    DraftStatus, DraftTarget, DraftValueProblem, Study, Timestamp, draft_fingerprint, is_blank,
};
use uuid::Uuid;

/// The largest page any MCP list returns (A2: responses are bounded, so no read holds a lock long).
pub const MAX_PAGE: u32 = 200;

/// The dossier a call read or wrote: its identity and its resolved path (O3 — every MCP response
/// names it; D10 — every submission carries back the one the AI read).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DossierIdentity {
    pub journal_id: Uuid,
    pub path: PathBuf,
}

/// A page request: `offset` rows skipped, at most `limit` returned (clamped to `1..=MAX_PAGE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    pub offset: u64,
    pub limit: u32,
}

impl Page {
    /// The first page of at most `limit` rows.
    pub fn first(limit: u32) -> Self {
        Page { offset: 0, limit }
    }

    fn bounded(self) -> (u32, u64) {
        (self.limit.clamp(1, MAX_PAGE), self.offset)
    }
}

/// One page of a list, with the total row count (so the caller knows whether more follow).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paged<T> {
    pub items: Vec<T>,
    pub offset: u64,
    pub total: u64,
}

/// A study as the MCP surface reads it: the whole contract study (data cells with provenance,
/// judgments, rationale, notes) and its lifecycle status. The computed outputs (O1) are built from
/// it by the MCP server with `report::form::build_snapshot` — the app's own construction (8.4).
#[derive(Debug, Clone, PartialEq)]
pub struct McpStudyRead {
    pub study: Study,
    /// `"active"` or `"archived"`.
    pub status: String,
}

/// One FR51 history entry of a study: the full study state at that moment.
#[derive(Debug, Clone, PartialEq)]
pub struct McpSnapshot {
    pub id: Uuid,
    pub created_at: Timestamp,
    pub study: Study,
}

/// One read of [`McpAccess::read_identified`] (Story 8.4 G3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpReadRequest {
    Studies(Page),
    Study(Uuid),
    History(Uuid, Page),
    Drafts(DraftFilter, Page),
    /// One draft of the record, by id.
    Draft(Uuid),
}

/// The result of a [`McpReadRequest`].
#[derive(Debug, Clone, PartialEq)]
pub enum McpRead {
    Studies(Paged<StudySummary>),
    Study(Box<McpStudyRead>),
    History(Paged<McpSnapshot>),
    Drafts(Paged<DraftRecord>),
    Draft(Option<Box<DraftRecord>>),
    /// A study-scoped request named a study the dossier does not hold.
    StudyMissing(Uuid),
}

/// A recorded submission: the draft's id and its STORED status — `pending` for a new draft; for
/// the same proposition sent again, whatever the owner has decided since (Story 8.4 G3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recorded {
    pub id: Uuid,
    pub status: DraftStatus,
}

/// Which drafts a [`McpAccess::list_drafts`] call returns (both filters optional).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DraftFilter {
    /// Only the drafts about this study (`study_id`).
    pub study_id: Option<Uuid>,
    /// Only the drafts in this status.
    pub status: Option<DraftStatus>,
}

/// One action the authorizer denied: its kind (`insert`, `update`, `pragma`, `attach`…) and its
/// object (`table.column`, a pragma name, a file…) — the data of the MCP server's log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpDenial {
    pub action: String,
    pub object: String,
}

/// The denied actions as one line (the [`Error::McpDenied`] Display).
pub(crate) fn denials_text(denials: &[McpDenial]) -> String {
    if denials.is_empty() {
        return "a statement".to_string();
    }
    denials
        .iter()
        .map(|d| format!("{} {}", d.action, d.object))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The longest comment a draft may carry, in characters (Story 8.3 G3 E8).
pub const MAX_COMMENT_CHARS: usize = 10_000;
/// The longest note text a note draft may carry, in characters.
pub const MAX_NOTE_CHARS: usize = 10_000;
/// The longest company name a draft study may propose, in characters.
pub const MAX_COMPANY_NAME_CHARS: usize = 200;
/// The longest client or model name of a draft's origin, in characters.
pub const MAX_ORIGIN_CHARS: usize = 100;
/// The longest proposed value text, in characters (Story 8.4 G3: checked before any parse, so a
/// refusal never echoes a huge text).
pub const MAX_PROPOSED_VALUE_CHARS: usize = 100;

/// Why the MCP access surface cannot use a dossier at all (nothing was read or written). The MCP
/// server renders each by its [`code`](McpUnavailable::code) (Story 8.0 §3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpUnavailable {
    /// No file at the resolved path.
    Missing,
    /// The file is empty, is not a SQLite database, or is not a journal.
    NotADossier,
    /// A restore of the dossier is in progress (its `-restoring` marker names a live process), or
    /// the file was swapped while a read ran.
    RestoreInProgress,
    /// A restore was interrupted (a marker or staging copy left by a process that is gone): the app
    /// clears it when it next opens the dossier (G3 N2).
    RestoreInterrupted,
    /// The dossier stayed locked by another connection beyond the wait (G3 N4).
    Busy,
    /// The dossier needs the app's recovery first (a hot rollback journal left by a crash, or WAL
    /// side files a read-only connection cannot initialise).
    NeedsRecovery,
    /// Reading it would create side files beside a protected file (unconsolidated writes), or in a
    /// protected directory; writing it is impossible. `directory` names which is protected.
    Protected { directory: bool },
    /// The dossier file's identity could not be read.
    IdentityUnreadable { detail: String },
}

impl McpUnavailable {
    /// The stable code the MCP tool returns.
    pub fn code(&self) -> &'static str {
        match self {
            McpUnavailable::Missing => "no_dossier",
            McpUnavailable::NotADossier => "not_a_dossier",
            McpUnavailable::RestoreInProgress => "dossier_busy",
            McpUnavailable::RestoreInterrupted => "restore_interrupted",
            McpUnavailable::Busy => "dossier_locked",
            McpUnavailable::NeedsRecovery => "dossier_needs_recovery",
            McpUnavailable::Protected { .. } => "dossier_protected",
            McpUnavailable::IdentityUnreadable { .. } => "dossier_identity_unreadable",
        }
    }
}

impl std::fmt::Display for McpUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpUnavailable::Missing => f.write_str("no file is at the dossier path"),
            McpUnavailable::NotADossier => f.write_str("the file is not a journal"),
            McpUnavailable::RestoreInProgress => {
                f.write_str("a restore of the dossier is in progress")
            }
            McpUnavailable::RestoreInterrupted => f.write_str(
                "a restore of the dossier was interrupted; the app clears it when it opens the dossier",
            ),
            McpUnavailable::Busy => f.write_str("the dossier stayed locked by another connection"),
            McpUnavailable::NeedsRecovery => {
                f.write_str("the dossier needs the app's recovery first")
            }
            McpUnavailable::Protected { directory } => write!(
                f,
                "the dossier {} is protected against writing",
                if *directory { "directory" } else { "file" }
            ),
            McpUnavailable::IdentityUnreadable { detail } => {
                write!(f, "the dossier file identity could not be read: {detail}")
            }
        }
    }
}

/// A draft proposed by an AI client, as the MCP server hands it over (Story 8.4 builds it from the
/// tool call). Which fields a kind uses: study → `security_ticker`, `native_currency`,
/// `company_name`; note → `study_id`, `note_text`; cell / judgment → `study_id`, `target`,
/// `proposed_value`. Every kind: `comment`, `origin`, the caller's `id` / `created_at`, the
/// `dossier` the AI read, and the `method_version` the fingerprint is computed under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftSubmission {
    pub id: Uuid,
    pub created_at: Timestamp,
    pub kind: DraftKind,
    pub study_id: Option<Uuid>,
    pub security_ticker: Option<String>,
    pub native_currency: Option<String>,
    pub company_name: Option<String>,
    pub target: Option<DraftTarget>,
    pub proposed_value: Option<String>,
    pub note_text: Option<String>,
    pub comment: String,
    pub origin: DraftOrigin,
    pub dossier: DossierIdentity,
    pub method_version: String,
}

/// Why a submitted draft was refused — nothing was written. Each variant carries the data its
/// French message needs (rendered by the MCP server, Story 8.0 §3.3); [`Self::code`] is the stable
/// English code the tool returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmissionRefusal {
    /// The dossier the AI read is not the one resolved for this call (D10).
    DossierMismatch {
        read: DossierIdentity,
        current: DossierIdentity,
    },
    /// The dossier file was replaced (a restore) during the write (A11).
    DossierReplaced,
    /// The target study does not exist in the dossier.
    StudyNotFound { study_id: Uuid },
    /// The target study is archived: a note, cell or judgment draft on it is refused (Story 8.3
    /// G3 — conservative; the owner unarchives it first).
    StudyArchived { ticker: String },
    /// The comment is blank (NFR-A4).
    EmptyComment,
    /// The client or the model is blank (NFR-A4).
    MissingOrigin,
    /// A text is longer than its cap (`field` = `comment`, `note_text`, `company_name`,
    /// `origin_client`, `origin_model`, `proposed_value`).
    TextTooLong {
        field: &'static str,
        max: usize,
        len: usize,
    },
    /// A draft study's identifier or currency does not follow the rule (8.0 §4.1).
    IdentifierInvalid { ticker: String, currency: String },
    /// The target names no draftable field of the draft's kind (D6).
    FieldNotDraftable { field: String },
    /// The target's fiscal year is not a year of the study — a draft never adds a year.
    YearNotInStudy { year: i32, ticker: String },
    /// The proposed value is not a plain decimal in the field's unit.
    ValueUnparsable { text: String, field: DraftField },
    /// The proposed value is not one of the option field's names.
    ValueNotAnOption { text: String, field: DraftField },
    /// The proposed number is outside the proposal bounds (|value| < 10^15, ≤ 10 decimals).
    ValueOutOfRange { text: String, field: DraftField },
    /// A note draft's text is blank.
    EmptyNoteText,
    /// The target already has a pending draft (D4).
    TargetHasPending {
        field: DraftField,
        fiscal_year: Option<i32>,
        pending_id: Uuid,
    },
    /// The security is already studied in the same currency (D2) — archived studies included.
    StudyExists {
        ticker: String,
        currency: String,
        study_id: Uuid,
    },
    /// A draft study for the same security and currency is already pending (D8).
    DraftStudyPending {
        ticker: String,
        currency: String,
        draft_id: Uuid,
    },
    /// A draft with this id already exists with another content (a re-submission of the SAME
    /// draft is idempotent and not refused).
    DraftIdConflict { id: Uuid },
}

impl SubmissionRefusal {
    /// The stable code the MCP tool returns (Story 8.0 §3.3, extended by Story 8.3).
    pub fn code(&self) -> &'static str {
        match self {
            SubmissionRefusal::DossierMismatch { .. } => "dossier_mismatch",
            SubmissionRefusal::DossierReplaced => "dossier_replaced",
            SubmissionRefusal::StudyNotFound { .. } => "study_not_found",
            SubmissionRefusal::StudyArchived { .. } => "study_archived",
            SubmissionRefusal::EmptyComment => "empty_comment",
            SubmissionRefusal::MissingOrigin => "missing_origin",
            SubmissionRefusal::TextTooLong { .. } => "text_too_long",
            SubmissionRefusal::IdentifierInvalid { .. } => "identifier_invalid",
            SubmissionRefusal::FieldNotDraftable { .. } => "field_not_draftable",
            SubmissionRefusal::YearNotInStudy { .. } => "year_not_in_study",
            SubmissionRefusal::ValueUnparsable { .. } => "value_unparsable",
            SubmissionRefusal::ValueNotAnOption { .. } => "value_not_an_option",
            SubmissionRefusal::ValueOutOfRange { .. } => "value_out_of_range",
            SubmissionRefusal::EmptyNoteText => "empty_note_text",
            SubmissionRefusal::TargetHasPending { .. } => "target_has_pending",
            SubmissionRefusal::StudyExists { .. } => "study_exists",
            SubmissionRefusal::DraftStudyPending { .. } => "draft_study_pending",
            SubmissionRefusal::DraftIdConflict { .. } => "draft_id_conflict",
        }
    }
}

/// The outcome of a refused or failed submission: a typed refusal (the AI's proposal does not pass
/// a check — nothing written) or a failure (the dossier could not be read or written).
#[derive(Debug)]
pub enum SubmitError {
    Refused(SubmissionRefusal),
    Failed(Error),
    /// The caller cancelled the call before the insert (Story 8.4 G3): nothing was written.
    Cancelled,
}

impl SubmitError {
    /// The stable MCP code of this outcome: the refusal's, or the failure's when it has one
    /// ([`Error::mcp_code`]); `None` for a failure without a named MCP cause (logged as such).
    pub fn code(&self) -> Option<&'static str> {
        match self {
            SubmitError::Refused(r) => Some(r.code()),
            SubmitError::Failed(e) => e.mcp_code(),
            SubmitError::Cancelled => Some("cancelled"),
        }
    }
}

impl From<Error> for SubmitError {
    fn from(e: Error) -> Self {
        SubmitError::Failed(e)
    }
}

impl From<SubmissionRefusal> for SubmitError {
    fn from(r: SubmissionRefusal) -> Self {
        SubmitError::Refused(r)
    }
}

/// The gated access to one dossier (Story 8.3). Holds only the resolved path: every method opens
/// its own connection, works and drops it before returning.
#[derive(Debug, Clone)]
pub struct McpAccess {
    path: PathBuf,
}

/// Which authorizer a connection carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Policy {
    Read,
    Draft,
}

/// The identity of the dossier file, taken WITHOUT opening a descriptor on Unix (`stat(2)`: device
/// + inode) — closing any descriptor of the file would release this process's SQLite locks on it.
#[cfg(unix)]
#[derive(Debug, PartialEq, Eq)]
struct FileId {
    dev: u64,
    ino: u64,
}

#[cfg(unix)]
fn file_id(path: &Path) -> std::io::Result<FileId> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::metadata(path)?;
    Ok(FileId {
        dev: m.dev(),
        ino: m.ino(),
    })
}

/// The identity of the dossier file on Windows: an open `same_file::Handle` (volume serial + file
/// index). Windows has no POSIX-lock hazard; the handle is opened BEFORE the connection and dropped
/// AFTER it (field order of [`Gated`]).
#[cfg(not(unix))]
#[derive(Debug, PartialEq, Eq)]
struct FileId(same_file::Handle);

#[cfg(not(unix))]
fn file_id(path: &Path) -> std::io::Result<FileId> {
    same_file::Handle::from_path(path).map(FileId)
}

/// A connection with its authorizer installed and the denials it recorded.
struct Gated {
    conn: Connection,
    denials: Arc<Mutex<Vec<McpDenial>>>,
    /// The file identity taken BEFORE the connection opened. Declared after `conn`: the connection
    /// closes first (on Windows the identity is an open handle).
    opened: FileId,
}

/// Map a statement failure: a statement the authorizer denied becomes [`Error::McpDenied`] naming
/// each denial; the named SQLite states a dossier can be in become [`McpUnavailable`]; anything
/// else stays the SQLite error it is.
fn map_sqlite(e: rusqlite::Error, denials: &Mutex<Vec<McpDenial>>) -> Error {
    use rusqlite::ffi;
    if let rusqlite::Error::SqliteFailure(code, _) = &e {
        if code.code == rusqlite::ErrorCode::AuthorizationForStatementDenied {
            let denials = denials
                .lock()
                .map(|d| d.clone())
                .unwrap_or_else(|poisoned| poisoned.into_inner().clone());
            return Error::McpDenied { denials };
        }
        if code.code == rusqlite::ErrorCode::NotADatabase {
            return unavailable(McpUnavailable::NotADossier);
        }
        if matches!(
            code.extended_code,
            ffi::SQLITE_READONLY_ROLLBACK
                | ffi::SQLITE_READONLY_RECOVERY
                | ffi::SQLITE_READONLY_CANTINIT
        ) {
            return unavailable(McpUnavailable::NeedsRecovery);
        }
    }
    Error::Sqlite(e)
}

fn unavailable(reason: McpUnavailable) -> Error {
    Error::McpUnavailable { reason }
}

/// Whether SQLite reports the connection's own file as moved or unlinked since it opened it
/// (`SQLITE_FCNTL_HAS_MOVED` — a `stat` of its path by SQLite, no descriptor). A VFS without the
/// control (Windows) answers "not moved"; the [`FileId`] re-check covers it there.
fn has_moved(conn: &Connection) -> Result<bool> {
    use rusqlite::ffi;
    let mut moved: std::os::raw::c_int = 0;
    // SAFETY: `conn.handle()` is the live `sqlite3*` of a connection this function borrows (it
    // outlives the call); "main" is a NUL-terminated C string; SQLITE_FCNTL_HAS_MOVED writes one
    // `int` through the pointer, which points at a live, aligned `c_int`.
    let rc = unsafe {
        ffi::sqlite3_file_control(
            conn.handle(),
            c"main".as_ptr(),
            ffi::SQLITE_FCNTL_HAS_MOVED,
            (&mut moved as *mut std::os::raw::c_int).cast(),
        )
    };
    match rc {
        ffi::SQLITE_OK => Ok(moved != 0),
        ffi::SQLITE_NOTFOUND => Ok(false),
        other => Err(unavailable(McpUnavailable::IdentityUnreadable {
            detail: format!("SQLITE_FCNTL_HAS_MOVED returned {other}"),
        })),
    }
}

fn is_readable(table: &str) -> bool {
    MCP_READABLE_TABLES.contains(&table) || SQLITE_INTERNAL_TABLES.contains(&table)
}

/// The read connection's policy (A3): allowlisted reads, `SELECT`, functions, transactions, and
/// READING `user_version` (the schema re-check inside a submission — never setting it). Everything
/// else — and any action this rusqlite version does not name (`#[non_exhaustive]`) — is denied.
pub(crate) fn read_policy(ctx: &AuthContext<'_>) -> Authorization {
    match ctx.action {
        AuthAction::Read { table_name, .. } if is_readable(table_name) => Authorization::Allow,
        AuthAction::Pragma {
            pragma_name: "user_version",
            pragma_value: None,
        } => Authorization::Allow,
        AuthAction::Select
        | AuthAction::Function { .. }
        | AuthAction::Transaction { .. }
        | AuthAction::Recursive => Authorization::Allow,
        _ => Authorization::Deny,
    }
}

/// The draft connection's policy (A3): the read policy, plus `INSERT` into `ai_drafts` from
/// top-level SQL, plus `UPDATE journal_meta.logical_version` **only** from [`DRAFT_TRIGGER`].
pub(crate) fn draft_policy(ctx: &AuthContext<'_>) -> Authorization {
    match ctx.action {
        AuthAction::Insert {
            table_name: "ai_drafts",
        } if ctx.accessor.is_none() => Authorization::Allow,
        AuthAction::Update {
            table_name: "journal_meta",
            column_name: "logical_version",
        } if ctx.accessor == Some(DRAFT_TRIGGER) => Authorization::Allow,
        _ => read_policy(ctx),
    }
}

/// The action kind and object of a denied action, for the log.
fn denial_of(action: &AuthAction<'_>) -> McpDenial {
    let (action, object) = match *action {
        AuthAction::Read {
            table_name,
            column_name,
        } => ("read", format!("{table_name}.{column_name}")),
        AuthAction::Insert { table_name } => ("insert", table_name.to_string()),
        AuthAction::Update {
            table_name,
            column_name,
        } => ("update", format!("{table_name}.{column_name}")),
        AuthAction::Delete { table_name } => ("delete", table_name.to_string()),
        AuthAction::Pragma {
            pragma_name,
            pragma_value,
        } => (
            "pragma",
            match pragma_value {
                Some(v) => format!("{pragma_name}={v}"),
                None => pragma_name.to_string(),
            },
        ),
        AuthAction::Attach { filename } => ("attach", filename.to_string()),
        AuthAction::Detach { database_name } => ("detach", database_name.to_string()),
        AuthAction::CreateTable { table_name } | AuthAction::CreateTempTable { table_name } => {
            ("create table", table_name.to_string())
        }
        AuthAction::DropTable { table_name } | AuthAction::DropTempTable { table_name } => {
            ("drop table", table_name.to_string())
        }
        AuthAction::CreateIndex { index_name, .. }
        | AuthAction::CreateTempIndex { index_name, .. } => {
            ("create index", index_name.to_string())
        }
        AuthAction::DropIndex { index_name, .. } | AuthAction::DropTempIndex { index_name, .. } => {
            ("drop index", index_name.to_string())
        }
        AuthAction::CreateTrigger { trigger_name, .. }
        | AuthAction::CreateTempTrigger { trigger_name, .. } => {
            ("create trigger", trigger_name.to_string())
        }
        AuthAction::DropTrigger { trigger_name, .. }
        | AuthAction::DropTempTrigger { trigger_name, .. } => {
            ("drop trigger", trigger_name.to_string())
        }
        AuthAction::CreateView { view_name } | AuthAction::CreateTempView { view_name } => {
            ("create view", view_name.to_string())
        }
        AuthAction::DropView { view_name } | AuthAction::DropTempView { view_name } => {
            ("drop view", view_name.to_string())
        }
        AuthAction::AlterTable { table_name, .. } => ("alter table", table_name.to_string()),
        AuthAction::Reindex { index_name } => ("reindex", index_name.to_string()),
        AuthAction::Analyze { table_name } => ("analyze", table_name.to_string()),
        AuthAction::CreateVtable { table_name, .. } => {
            ("create virtual table", table_name.to_string())
        }
        AuthAction::DropVtable { table_name, .. } => ("drop virtual table", table_name.to_string()),
        AuthAction::Savepoint { savepoint_name, .. } => ("savepoint", savepoint_name.to_string()),
        AuthAction::Function { function_name } => ("function", function_name.to_string()),
        AuthAction::Unknown { code, .. } => ("action", format!("code {code}")),
        _ => ("action", "unnamed".to_string()),
    };
    McpDenial {
        action: action.to_string(),
        object,
    }
}

/// Two identifiers (tickers, currencies) name the same thing: compared ignoring ASCII case and
/// surrounding spaces — the app's `same_ticker` rule (G1 P review L-h). Stored studies may carry a
/// hand-typed spelling the submission rule would refuse (`chf`, ` NESN.SW`).
fn same_identifier(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// A text within its character cap, or the refusal naming it.
fn within(field: &'static str, text: &str, max: usize) -> std::result::Result<(), SubmitError> {
    let len = text.chars().count();
    if len > max {
        return Err(SubmissionRefusal::TextTooLong { field, max, len }.into());
    }
    Ok(())
}

/// Test seams of a submission: right after the connection opens (a restore can swap the file
/// there — A11), and inside the `IMMEDIATE` transaction once the identities are checked (a second
/// process probes the write lock there). Production passes no-ops.
pub(crate) struct SubmitHooks<'h> {
    pub(crate) after_open: &'h mut dyn FnMut(),
    pub(crate) in_transaction: &'h mut dyn FnMut(),
}

impl McpAccess {
    /// The gated access to the dossier at `path` (resolved once, like the app — symlinks
    /// followed). Opens nothing yet.
    pub fn at(path: impl AsRef<Path>) -> Self {
        McpAccess {
            path: resolved_path(path.as_ref()),
        }
    }

    /// The resolved path this access works on.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A restore running on the dossier, or one left interrupted, refuses every call (G3 N2/N4).
    fn refuse_during_restore(&self) -> Result<()> {
        use crate::restore::RestoreState;
        match crate::restore::restore_state(&self.path) {
            RestoreState::Idle => Ok(()),
            RestoreState::Running => Err(unavailable(McpUnavailable::RestoreInProgress)),
            RestoreState::Interrupted => Err(unavailable(McpUnavailable::RestoreInterrupted)),
        }
    }

    /// A lock that outlasted the wait is named (G3 N4): the restore's own when one is running, a
    /// plain « locked » otherwise.
    fn name_busy(&self, e: Error) -> Error {
        let busy = matches!(&e, Error::Sqlite(rusqlite::Error::SqliteFailure(f, _))
            if matches!(f.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked));
        if !busy {
            return e;
        }
        match self.refuse_during_restore() {
            Err(named) => named,
            Ok(()) => unavailable(McpUnavailable::Busy),
        }
    }

    /// Whether the file at the path is still the one `opened` names, and the connection's file has
    /// not moved (A11). A path that no longer holds a file is a move; an identity that cannot be
    /// read is its own error, never a guessed « replaced ».
    fn still_the_opened_file(&self, conn: &Connection, opened: &FileId) -> Result<bool> {
        if has_moved(conn)? {
            return Ok(false);
        }
        match file_id(&self.path) {
            Ok(now) => Ok(now == *opened),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(unavailable(McpUnavailable::IdentityUnreadable {
                detail: e.to_string(),
            })),
        }
    }

    /// Open one gated connection. In order: the restore marker, the file (missing / empty), its
    /// identity (BEFORE the connection — A11), its protection (SQLite's own probe — no stray
    /// descriptor), the connection itself (read-only, `immutable` for a protected file with no
    /// content side file, or read-write for drafts; never `CREATE`), `after_open` (test seam), the
    /// restore marker again, the connection-local pragmas, the version gate, THEN the authorizer —
    /// nothing is prepared before the gate is in place except the pragmas and the `user_version`
    /// read.
    fn open(&self, policy: Policy, after_open: &mut dyn FnMut()) -> Result<Gated> {
        self.refuse_during_restore()?;
        let meta = match std::fs::metadata(&self.path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(unavailable(McpUnavailable::Missing));
            }
            Err(e) => {
                return Err(Error::Sqlite(rusqlite::Error::InvalidPath(
                    format!("{}: {e}", self.path.display()).into(),
                )));
            }
        };
        if !meta.is_file() || meta.len() == 0 {
            return Err(unavailable(McpUnavailable::NotADossier));
        }
        let opened = file_id(&self.path).map_err(|e| {
            unavailable(McpUnavailable::IdentityUnreadable {
                detail: e.to_string(),
            })
        })?;
        let file_protected = {
            // SQLite's own detection (a READ_WRITE open of a file the OS will not let it write falls
            // back to read-only and says so) — a SQLite connection, so no stray descriptor; nothing
            // is read, so nothing is created beside the file.
            let probe = Connection::open_with_flags(
                &self.path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            probe.is_readonly(rusqlite::MAIN_DB)?
        };
        let directory_protected = self
            .path
            .parent()
            .and_then(|dir| std::fs::metadata(dir).ok())
            .is_some_and(|m| m.permissions().readonly());
        let content_side_file = ["-wal", "-journal"].iter().any(|suffix| {
            let mut p = self.path.as_os_str().to_os_string();
            p.push(suffix);
            std::fs::metadata(PathBuf::from(p)).is_ok_and(|m| m.len() > 0)
        });
        let conn = match policy {
            Policy::Read if file_protected && !content_side_file => Connection::open_with_flags(
                format!("{}?mode=ro&immutable=1", file_uri(&self.path)),
                OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_URI
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?,
            _ if file_protected || directory_protected => {
                return Err(unavailable(McpUnavailable::Protected {
                    directory: !file_protected,
                }));
            }
            Policy::Read => Connection::open_with_flags(
                &self.path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?,
            Policy::Draft => Connection::open_with_flags(
                &self.path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?,
        };
        after_open();
        self.refuse_during_restore()?;
        let denials = Arc::new(Mutex::new(Vec::new()));
        apply_connection_local_pragmas(&conn)?;
        let file_user_version = migrations::user_version(&conn).map_err(|e| lift(e, &denials))?;
        let supported = migrations::latest_version(migrations::REGISTRY);
        if file_user_version == 0 {
            // Every journal is at user_version ≥ 1 from its creation: a user_version 0 file is some
            // other SQLite database.
            return Err(unavailable(McpUnavailable::NotADossier));
        }
        if file_user_version != i64::from(supported) {
            return Err(Error::McpSchemaMismatch {
                file_user_version,
                supported,
            });
        }
        let record = Arc::clone(&denials);
        conn.authorizer(Some(move |ctx: AuthContext<'_>| {
            let verdict = match policy {
                Policy::Read => read_policy(&ctx),
                Policy::Draft => draft_policy(&ctx),
            };
            if verdict != Authorization::Allow {
                let denial = denial_of(&ctx.action);
                match record.lock() {
                    Ok(mut d) => d.push(denial),
                    Err(poisoned) => poisoned.into_inner().push(denial),
                }
            }
            verdict
        }))?;
        Ok(Gated {
            conn,
            denials,
            opened,
        })
    }

    /// Run `f` in one short read transaction on a fresh read connection (A2); a file swapped while
    /// it ran (a restore) makes the read [`McpUnavailable::RestoreInProgress`] — it may have read
    /// the replaced file.
    fn read<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        self.read_inner(f).map_err(|e| self.name_busy(e))
    }

    fn read_inner<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        // Kept whole (not destructured): its fields drop in declaration order — the connection
        // before the identity (Windows: an open handle).
        let mut gated = self.open(Policy::Read, &mut || {})?;
        let out = {
            let tx = gated
                .conn
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .map_err(|e| map_sqlite(e, &gated.denials))?;
            let out = f(&tx);
            drop(tx); // a read transaction: rolling back is committing nothing
            out
        };
        let out = out.map_err(|e| lift(e, &gated.denials))?;
        if !self.still_the_opened_file(&gated.conn, &gated.opened)? {
            return Err(unavailable(McpUnavailable::RestoreInProgress));
        }
        Ok(out)
    }

    /// The dossier this access reads (O3): its `journal_id` and resolved path.
    pub fn identity(&self) -> Result<DossierIdentity> {
        let journal_id = self.read(read_journal_id)?;
        Ok(DossierIdentity {
            journal_id,
            path: self.path.clone(),
        })
    }

    /// One page of the studies, ordered by `(created_at, id)`, with their total count.
    pub fn list_studies(&self, page: Page) -> Result<Paged<StudySummary>> {
        self.read(|conn| query_studies(conn, page))
    }

    /// One study (FR69): the whole contract study and its status, `None` when the dossier holds
    /// no such study.
    pub fn read_study(&self, id: Uuid) -> Result<Option<McpStudyRead>> {
        self.read(|conn| query_study(conn, id))
    }

    /// One page of a study's FR51 history, newest first (each entry the full study state).
    pub fn read_history(&self, study_id: Uuid, page: Page) -> Result<Paged<McpSnapshot>> {
        self.read(|conn| query_history(conn, study_id, page))
    }

    /// One page of the drafts record (FR77), filtered, ordered by `(created_at, id)`.
    pub fn list_drafts(&self, filter: DraftFilter, page: Page) -> Result<Paged<DraftRecord>> {
        self.read(|conn| query_drafts(conn, filter, page))
    }

    /// One read AND the identity of the dossier it read, in the SAME read transaction (Story 8.4
    /// G3 — a response names exactly the dossier its data came from). A study-scoped request on a
    /// study the dossier does not hold answers [`McpRead::StudyMissing`].
    pub fn read_identified(&self, request: McpReadRequest) -> Result<(DossierIdentity, McpRead)> {
        self.read(|conn| {
            let identity = DossierIdentity {
                journal_id: read_journal_id(conn)?,
                path: self.path.clone(),
            };
            let study_known =
                |id: Uuid| -> Result<bool> { Ok(study_status_in(conn, id)?.is_some()) };
            let out = match request {
                McpReadRequest::Studies(page) => McpRead::Studies(query_studies(conn, page)?),
                McpReadRequest::Study(id) => match query_study(conn, id)? {
                    Some(read) => McpRead::Study(Box::new(read)),
                    None => McpRead::StudyMissing(id),
                },
                McpReadRequest::History(id, page) => {
                    if study_known(id)? {
                        McpRead::History(query_history(conn, id, page)?)
                    } else {
                        McpRead::StudyMissing(id)
                    }
                }
                McpReadRequest::Drafts(filter, page) => match filter.study_id {
                    Some(id) if !study_known(id)? => McpRead::StudyMissing(id),
                    _ => McpRead::Drafts(query_drafts(conn, filter, page)?),
                },
                McpReadRequest::Draft(id) => {
                    McpRead::Draft(crate::drafts::read_draft_in(conn, id)?.map(Box::new))
                }
            };
            Ok((identity, out))
        })
    }

    /// Submit a draft (Story 8.3 AC 7): every check and the insert run in ONE `BEGIN IMMEDIATE`
    /// transaction; a refused draft writes nothing. Returns the draft's id (the caller's). The same
    /// draft submitted again (same id, same content) returns its id and writes nothing.
    pub fn submit_draft(&self, sub: &DraftSubmission) -> std::result::Result<Uuid, SubmitError> {
        self.submit_draft_recorded(sub, &|| false).map(|r| r.id)
    }

    /// [`Self::submit_draft`] returning the STORED status too (a retry of a decided draft answers
    /// its decision), and checking `cancelled` right before the insert (Story 8.4 G3): a cancelled
    /// call writes nothing ([`SubmitError::Cancelled`]).
    pub fn submit_draft_recorded(
        &self,
        sub: &DraftSubmission,
        cancelled: &dyn Fn() -> bool,
    ) -> std::result::Result<Recorded, SubmitError> {
        self.submit_draft_with(
            sub,
            SubmitHooks {
                after_open: &mut || {},
                in_transaction: &mut || {},
            },
            cancelled,
        )
    }

    /// [`Self::submit_draft`] with its test seams ([`SubmitHooks`]).
    pub(crate) fn submit_draft_with(
        &self,
        sub: &DraftSubmission,
        hooks: SubmitHooks<'_>,
        cancelled: &dyn Fn() -> bool,
    ) -> std::result::Result<Recorded, SubmitError> {
        check_shape(sub)?;
        // Kept whole: the connection drops before the identity (Windows: an open handle).
        let mut gated = self
            .open(Policy::Draft, hooks.after_open)
            .map_err(|e| SubmitError::Failed(self.name_busy(e)))?;
        let outcome = {
            let tx = gated
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|e| SubmitError::Failed(map_sqlite(e, &gated.denials)))?;
            match self.check_and_insert(&tx, sub, &gated.opened, hooks.in_transaction, cancelled) {
                Ok(status) => tx
                    .commit()
                    .map(|()| status)
                    .map_err(|e| SubmitError::Failed(map_sqlite(e, &gated.denials))),
                Err(e) => Err(e), // the transaction rolls back on drop: nothing written
            }
        };
        outcome
            .map(|status| Recorded { id: sub.id, status })
            .map_err(|e| match e {
                SubmitError::Failed(err) => {
                    SubmitError::Failed(self.name_busy(lift(err, &gated.denials)))
                }
                other => other,
            })
    }

    /// The submission checks, in the story's order (T5.3), then the insert — inside the caller's
    /// `IMMEDIATE` transaction.
    fn check_and_insert(
        &self,
        tx: &Connection,
        sub: &DraftSubmission,
        opened: &FileId,
        in_transaction: &mut dyn FnMut(),
        cancelled: &dyn Fn() -> bool,
    ) -> std::result::Result<DraftStatus, SubmitError> {
        // 1. A11 — still the file this call opened (a restore renames another over it).
        if !self.still_the_opened_file(tx, opened)? {
            return Err(SubmissionRefusal::DossierReplaced.into());
        }
        // 2. A2 — the schema, re-read under the write lock.
        let file_user_version = migrations::user_version(tx)?;
        let supported = migrations::latest_version(migrations::REGISTRY);
        if file_user_version != i64::from(supported) {
            return Err(Error::McpSchemaMismatch {
                file_user_version,
                supported,
            }
            .into());
        }
        // 3. D10 — the dossier the AI read.
        let current = DossierIdentity {
            journal_id: read_journal_id(tx)?,
            path: self.path.clone(),
        };
        let read = DossierIdentity {
            journal_id: sub.dossier.journal_id,
            path: resolved_path(&sub.dossier.path),
        };
        if read != current {
            return Err(SubmissionRefusal::DossierMismatch {
                read: sub.dossier.clone(),
                current,
            }
            .into());
        }
        in_transaction();
        // 4. The same id again: the same draft is idempotent, another one is refused.
        if let Some(existing) = existing_draft(tx, sub.id)? {
            return if same_submission(&existing, sub) {
                Ok(existing.status)
            } else {
                Err(SubmissionRefusal::DraftIdConflict { id: sub.id }.into())
            };
        }
        // 5. NFR-A4 — comment and origin, then the length caps.
        if is_blank(&sub.comment) {
            return Err(SubmissionRefusal::EmptyComment.into());
        }
        if is_blank(&sub.origin.client) || is_blank(&sub.origin.model) {
            return Err(SubmissionRefusal::MissingOrigin.into());
        }
        within("comment", &sub.comment, MAX_COMMENT_CHARS)?;
        within("origin_client", &sub.origin.client, MAX_ORIGIN_CHARS)?;
        within("origin_model", &sub.origin.model, MAX_ORIGIN_CHARS)?;
        let (study_id, security_ticker, native_currency, payload) = match sub.kind {
            DraftKind::Study => self.draft_study(tx, sub)?,
            DraftKind::Note => self.draft_note(tx, sub)?,
            DraftKind::Cell | DraftKind::Judgment => self.draft_value(tx, sub)?,
        };
        let payload = serde_json::to_string(&payload).map_err(Error::from)?;
        // The shape rule shared with the read and the import — a draft is absent or right.
        check_payload(&payload, sub.kind).map_err(|problem| Error::McpInvalidCall {
            detail: format!("the built payload does not fit its kind: {problem}"),
        })?;
        if cancelled() {
            return Err(SubmitError::Cancelled);
        }
        tx.execute(
            "INSERT INTO ai_drafts
                 (id, kind, study_id, security_ticker, native_currency, status, created_at,
                  decided_at, comment, origin_client, origin_model, stale_at_decision,
                  edited_before_validation, created_study_id, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, NULL, ?7, ?8, ?9, NULL, NULL, NULL, ?10)",
            rusqlite::params![
                sub.id.to_string(),
                sub.kind.as_str(),
                study_id.map(|s| s.to_string()),
                security_ticker,
                native_currency,
                sub.created_at.0,
                sub.comment,
                sub.origin.client,
                sub.origin.model,
                payload
            ],
        )
        .map_err(Error::Sqlite)?;
        Ok(DraftStatus::Pending)
    }

    /// A draft study (D2, D8): identifier rule, name cap, then the two duplicate checks.
    #[allow(clippy::type_complexity)]
    fn draft_study(
        &self,
        tx: &Connection,
        sub: &DraftSubmission,
    ) -> std::result::Result<(Option<Uuid>, String, Option<String>, DraftPayload), SubmitError>
    {
        let ticker = sub.security_ticker.clone().unwrap_or_default();
        let currency = sub.native_currency.clone().unwrap_or_default();
        if !is_ticker(&ticker) || !is_currency_code(&currency) {
            return Err(SubmissionRefusal::IdentifierInvalid { ticker, currency }.into());
        }
        let company_name = sub.company_name.clone().filter(|name| !is_blank(name));
        if let Some(name) = &company_name {
            within("company_name", name, MAX_COMPANY_NAME_CHARS)?;
        }
        // D2 — already studied in the same currency (archived studies count: still in the dossier).
        let mut stmt = tx
            .prepare("SELECT id, security_ticker FROM studies ORDER BY created_at, id")
            .map_err(Error::Sqlite)?;
        let candidates = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
            .map_err(Error::Sqlite)?;
        for (id, stored_ticker) in candidates {
            if !same_identifier(&stored_ticker, &ticker) {
                continue;
            }
            let id = crate::util::parse_uuid(&id, "studies.id")?;
            let study = read_study_in(tx, id)?.ok_or_else(|| Error::CorruptPayload {
                detail: format!("study {id} vanished inside the submission transaction"),
            })?;
            if same_identifier(&study.native_currency, &currency) {
                return Err(SubmissionRefusal::StudyExists {
                    ticker,
                    currency,
                    study_id: id,
                }
                .into());
            }
        }
        // D8 — already pending as a draft study in the same currency.
        let mut stmt = tx
            .prepare(
                "SELECT id, security_ticker, native_currency FROM ai_drafts
                 WHERE kind = 'study' AND status = 'pending' ORDER BY created_at, id",
            )
            .map_err(Error::Sqlite)?;
        let pending = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
            .map_err(Error::Sqlite)?;
        if let Some((id, _, _)) = pending.iter().find(|(_, t, c)| {
            same_identifier(t, &ticker)
                && c.as_deref().is_some_and(|c| same_identifier(c, &currency))
        }) {
            return Err(SubmissionRefusal::DraftStudyPending {
                ticker,
                currency,
                draft_id: crate::util::parse_uuid(id, "ai_drafts.id")?,
            }
            .into());
        }
        Ok((
            None,
            ticker,
            Some(currency),
            DraftPayload {
                version: DRAFT_PAYLOAD_VERSION,
                target: None,
                proposed_value: None,
                note_text: None,
                company_name,
                base_fingerprint: None,
            },
        ))
    }

    /// The target study of a note / cell / judgment draft, looked up (never trusted): it must exist
    /// and not be archived.
    fn target_study(
        &self,
        tx: &Connection,
        sub: &DraftSubmission,
    ) -> std::result::Result<Study, SubmitError> {
        let study_id = sub
            .study_id
            .ok_or_else(|| invalid("a note, cell or judgment draft names its study"))?;
        let study =
            read_study_in(tx, study_id)?.ok_or(SubmissionRefusal::StudyNotFound { study_id })?;
        if study_status_in(tx, study_id)?.as_deref() == Some("archived") {
            return Err(SubmissionRefusal::StudyArchived {
                ticker: study.security_ticker,
            }
            .into());
        }
        Ok(study)
    }

    /// A note draft: the study exists and is active, the text is not blank and within its cap.
    #[allow(clippy::type_complexity)]
    fn draft_note(
        &self,
        tx: &Connection,
        sub: &DraftSubmission,
    ) -> std::result::Result<(Option<Uuid>, String, Option<String>, DraftPayload), SubmitError>
    {
        let study = self.target_study(tx, sub)?;
        let text = sub
            .note_text
            .clone()
            .ok_or_else(|| invalid("a note draft carries its text"))?;
        if is_blank(&text) {
            return Err(SubmissionRefusal::EmptyNoteText.into());
        }
        within("note_text", &text, MAX_NOTE_CHARS)?;
        Ok((
            Some(study.id),
            study.security_ticker,
            None,
            DraftPayload {
                version: DRAFT_PAYLOAD_VERSION,
                target: None,
                proposed_value: None,
                note_text: Some(text),
                company_name: None,
                base_fingerprint: None,
            },
        ))
    }

    /// A cell / judgment draft: the study exists and is active; the field is draftable and of the
    /// draft's kind; a cell's year is a year of the study; the value parses in the field's unit;
    /// the target has no pending draft (D4); the base fingerprint is computed in this transaction
    /// (A7).
    #[allow(clippy::type_complexity)]
    fn draft_value(
        &self,
        tx: &Connection,
        sub: &DraftSubmission,
    ) -> std::result::Result<(Option<Uuid>, String, Option<String>, DraftPayload), SubmitError>
    {
        let study = self.target_study(tx, sub)?;
        let target = sub
            .target
            .clone()
            .ok_or_else(|| invalid("a cell or judgment draft names its target"))?;
        let target_kind_matches = matches!(
            (&target, sub.kind),
            (DraftTarget::Cell { .. }, DraftKind::Cell)
                | (DraftTarget::Judgment { field: _ }, DraftKind::Judgment)
        );
        let field_key = match &target {
            DraftTarget::Cell { field, .. } | DraftTarget::Judgment { field } => field.clone(),
        };
        let (field, year) = DraftField::of_target(&target)
            .filter(|_| target_kind_matches)
            .ok_or(SubmissionRefusal::FieldNotDraftable { field: field_key })?;
        if field.kind() == DraftFieldKind::Cell {
            let year = year.unwrap_or_default();
            if !study.years.iter().any(|y| y.year == year) {
                return Err(SubmissionRefusal::YearNotInStudy {
                    year,
                    ticker: study.security_ticker,
                }
                .into());
            }
        }
        let text = sub
            .proposed_value
            .clone()
            .ok_or_else(|| invalid("a cell or judgment draft carries its proposed value"))?;
        within("proposed_value", &text, MAX_PROPOSED_VALUE_CHARS)?;
        if let Err(problem) = field.parse_value(&text) {
            return Err(match problem {
                DraftValueProblem::NotANumber => SubmissionRefusal::ValueUnparsable { text, field },
                DraftValueProblem::NotAnOption => {
                    SubmissionRefusal::ValueNotAnOption { text, field }
                }
                DraftValueProblem::OutOfRange => SubmissionRefusal::ValueOutOfRange { text, field },
            }
            .into());
        }
        // D4 — one pending draft per target.
        let mut stmt = tx
            .prepare(
                "SELECT id, payload FROM ai_drafts
                 WHERE study_id = ?1 AND status = 'pending' AND kind IN ('cell', 'judgment')
                 ORDER BY created_at, id",
            )
            .map_err(Error::Sqlite)?;
        let pending = stmt
            .query_map(rusqlite::params![study.id.to_string()], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
            .map_err(Error::Sqlite)?;
        for (id, payload) in pending {
            let parsed: DraftPayload =
                serde_json::from_str(&payload).map_err(|e| Error::CorruptPayload {
                    detail: format!("ai_drafts.payload of draft {id}: {e}"),
                })?;
            let same_target = parsed
                .target
                .as_ref()
                .and_then(DraftField::of_target)
                .is_some_and(|t| t == (field, year));
            if same_target {
                return Err(SubmissionRefusal::TargetHasPending {
                    field,
                    fiscal_year: year,
                    pending_id: crate::util::parse_uuid(&id, "ai_drafts.id")?,
                }
                .into());
            }
        }
        let base_fingerprint = draft_fingerprint(&study, &target, &sub.method_version)
            .ok_or_else(|| invalid("the target has no fingerprint after its checks"))?;
        Ok((
            Some(study.id),
            study.security_ticker,
            None,
            DraftPayload {
                version: DRAFT_PAYLOAD_VERSION,
                target: Some(target),
                proposed_value: Some(text.trim().to_string()),
                note_text: None,
                company_name: None,
                base_fingerprint: Some(base_fingerprint),
            },
        ))
    }
}

/// Lift a failure raised on a gated connection: a SQLite error goes through [`map_sqlite`] (a
/// denial, a named dossier state); every other error is kept.
fn lift(e: Error, denials: &Mutex<Vec<McpDenial>>) -> Error {
    match e {
        Error::Sqlite(inner) => map_sqlite(inner, denials),
        other => other,
    }
}

/// The fields a submission of each kind must carry — and must NOT carry (Story 8.3 G3 L1): a call
/// that mixes kinds is a caller (MCP server) defect, refused before the dossier is opened.
fn check_shape(sub: &DraftSubmission) -> std::result::Result<(), SubmitError> {
    if !is_rfc3339_utc(&sub.created_at.0) {
        return Err(invalid(&format!(
            "created_at {:?} is not an RFC3339 UTC timestamp",
            sub.created_at.0
        )));
    }
    let has = |o: bool, what: &str| if o { Err(invalid(what)) } else { Ok(()) };
    match sub.kind {
        DraftKind::Study => {
            has(sub.study_id.is_some(), "a draft study names no study")?;
            has(sub.target.is_some(), "a draft study carries no target")?;
            has(
                sub.proposed_value.is_some(),
                "a draft study carries no value",
            )?;
            has(
                sub.note_text.is_some(),
                "a draft study carries no note text",
            )?;
            has(
                sub.security_ticker.is_none(),
                "a draft study carries its ticker",
            )?;
            has(
                sub.native_currency.is_none(),
                "a draft study carries its currency",
            )?;
        }
        DraftKind::Note => {
            has(sub.study_id.is_none(), "a note draft names its study")?;
            has(sub.note_text.is_none(), "a note draft carries its text")?;
            has(sub.target.is_some(), "a note draft carries no target")?;
            has(
                sub.proposed_value.is_some(),
                "a note draft carries no value",
            )?;
            has(
                sub.security_ticker.is_some(),
                "a note draft carries no ticker",
            )?;
            has(
                sub.native_currency.is_some(),
                "a note draft carries no currency",
            )?;
            has(
                sub.company_name.is_some(),
                "a note draft carries no company name",
            )?;
        }
        DraftKind::Cell | DraftKind::Judgment => {
            has(sub.study_id.is_none(), "a value draft names its study")?;
            has(sub.target.is_none(), "a value draft names its target")?;
            has(
                sub.proposed_value.is_none(),
                "a value draft carries its value",
            )?;
            has(
                sub.note_text.is_some(),
                "a value draft carries no note text",
            )?;
            has(
                sub.security_ticker.is_some(),
                "a value draft carries no ticker",
            )?;
            has(
                sub.native_currency.is_some(),
                "a value draft carries no currency",
            )?;
            has(
                sub.company_name.is_some(),
                "a value draft carries no company name",
            )?;
        }
    }
    Ok(())
}

/// The stored row of draft `id`, if any.
/// One page of study summaries (with the total).
fn query_studies(conn: &Connection, page: Page) -> Result<Paged<StudySummary>> {
    let bounded = page.bounded();
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM studies", [], |r| r.get(0))?;
    Ok(Paged {
        items: list_studies_in(conn, Some(bounded))?,
        offset: bounded.1,
        total: u64::try_from(total).unwrap_or(0),
    })
}

/// One study and its status.
fn query_study(conn: &Connection, id: Uuid) -> Result<Option<McpStudyRead>> {
    let Some(study) = read_study_in(conn, id)? else {
        return Ok(None);
    };
    let status = study_status_in(conn, id)?.unwrap_or_else(|| "active".to_string());
    Ok(Some(McpStudyRead { study, status }))
}

/// One page of a study's FR51 history, newest first.
fn query_history(conn: &Connection, study_id: Uuid, page: Page) -> Result<Paged<McpSnapshot>> {
    let (limit, offset) = page.bounded();
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM judgments WHERE study_id = ?1",
        rusqlite::params![study_id.to_string()],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, created_at, schema_version, payload FROM judgments
         WHERE study_id = ?1 ORDER BY created_at DESC, rowid DESC LIMIT ?2 OFFSET ?3",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![
            study_id.to_string(),
            i64::from(limit),
            i64::try_from(offset).unwrap_or(i64::MAX)
        ],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        },
    )?;
    let mut items = Vec::new();
    for row in rows {
        let (id, created_at, schema_version, payload) = row?;
        items.push(McpSnapshot {
            id: crate::util::parse_uuid(&id, "judgments.id")?,
            created_at: Timestamp(created_at),
            study: parse_study_row(schema_version, &payload)?,
        });
    }
    Ok(Paged {
        items,
        offset,
        total: u64::try_from(total).unwrap_or(0),
    })
}

/// One page of the drafts record, filtered.
fn query_drafts(conn: &Connection, filter: DraftFilter, page: Page) -> Result<Paged<DraftRecord>> {
    let (limit, offset) = page.bounded();
    let study = filter.study_id.map(|s| s.to_string());
    let status = filter.status.map(DraftStatus::as_str);
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ai_drafts
         WHERE (?1 IS NULL OR study_id = ?1) AND (?2 IS NULL OR status = ?2)",
        rusqlite::params![study, status],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {DRAFT_COLUMNS} FROM ai_drafts
         WHERE (?1 IS NULL OR study_id = ?1) AND (?2 IS NULL OR status = ?2)
         ORDER BY created_at, id LIMIT ?3 OFFSET ?4"
    ))?;
    let rows = stmt.query_map(
        rusqlite::params![
            study,
            status,
            i64::from(limit),
            i64::try_from(offset).unwrap_or(i64::MAX)
        ],
        row_tuple,
    )?;
    let mut items = Vec::new();
    for row in rows {
        items.push(record_from_row(row?)?);
    }
    Ok(Paged {
        items,
        offset,
        total: u64::try_from(total).unwrap_or(0),
    })
}

fn existing_draft(tx: &Connection, id: Uuid) -> Result<Option<DraftRecord>> {
    let row = tx
        .query_row(
            &format!("SELECT {DRAFT_COLUMNS} FROM ai_drafts WHERE id = ?1"),
            rusqlite::params![id.to_string()],
            row_tuple,
        )
        .optional()?;
    row.map(record_from_row).transpose()
}

/// Whether a stored draft IS this submission (a re-submission after a lost reply): the same kind,
/// study, comment, origin and proposal. The creation time is NOT compared (Story 8.4 G3): it is
/// the server's clock at each attempt, so a retry never repeats it; the stored one stands. The ticker of a note / value draft is the stored
/// study's and the base fingerprint is computed at insert, so neither is compared.
fn same_submission(existing: &DraftRecord, sub: &DraftSubmission) -> bool {
    let Ok(payload) = serde_json::from_str::<DraftPayload>(&existing.payload) else {
        return false;
    };
    let study_fields = match sub.kind {
        DraftKind::Study => {
            sub.security_ticker.as_deref() == Some(existing.security_ticker.as_str())
                && sub.native_currency == existing.native_currency
                && sub.company_name.clone().filter(|n| !is_blank(n)) == payload.company_name
        }
        _ => true,
    };
    existing.kind == sub.kind
        && existing.study_id == sub.study_id
        && existing.comment == sub.comment
        && existing.origin_client == sub.origin.client
        && existing.origin_model == sub.origin.model
        && payload.target == sub.target
        && payload.proposed_value.as_deref() == sub.proposed_value.as_deref().map(str::trim)
        && payload.note_text == sub.note_text
        && study_fields
}

fn invalid(detail: &str) -> SubmitError {
    SubmitError::Failed(Error::McpInvalidCall {
        detail: detail.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::{Journal, JournalMode};
    use crate::schema::MCP_DENIED_TABLES;
    use rusqlite::hooks::TransactionOperation;
    use steadyinvest_contract::{
        Cell, Coverage, ForecastLowOption, Freshness, Judgment, Money, Provenance, Review,
        SCHEMA_VERSION, Source, YearData,
    };
    use tempfile::TempDir;

    const JID: u128 = 0x83_0000;
    const STUDY: u128 = 0x83_0001;

    fn ts(s: &str) -> Timestamp {
        Timestamp(s.to_string())
    }

    fn money(s: &str) -> Money {
        serde_json::from_str(&format!("\"{s}\"")).expect("decimal parses")
    }

    fn cell(v: &str) -> Cell {
        Cell {
            value: Some(money(v)),
            source: Source::Manual,
            freshness: Freshness::Current,
            review: Review::None,
            coverage: Coverage::Present,
            provenance: Provenance {
                ai_origin: None,
                source: Source::Manual,
                logical_version: 1,
                timestamp: ts("2026-09-28T08:00:00Z"),
                hash_of_dependencies: "t".to_string(),
            },
            pending: None,
        }
    }

    fn study() -> Study {
        let mut s = Study::new(
            Uuid::from_u128(STUDY),
            Uuid::from_u128(JID),
            "NESN.SW",
            "CHF",
            Judgment {
                ai_placed: Default::default(),
                estimated_high_eps: Some(money("5.20")),
                estimated_low_eps: None,
                projected_sales_growth_pct: None,
                projected_eps_growth_pct: None,
                judged_avg_high_pe: None,
                judged_avg_low_pe: None,
                forecast_low_option: ForecastLowOption::AvgLowPriceLast5y,
                recent_severe_low: None,
                current_price: None,
                present_full_year_dividend: None,
                ttm_eps: None,
            },
            ts("2026-09-28T08:00:00Z"),
        );
        s.years = vec![YearData {
            year: 2025,
            sales: cell("1000"),
            eps: cell("4.1"),
            high_price: cell("110"),
            low_price: cell("80"),
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
        }];
        s
    }

    /// A closed dossier (DELETE mode — self-contained file, so its bytes can be hashed) holding one
    /// study.
    fn dossier(dir: &TempDir) -> PathBuf {
        dossier_in(dir, JournalMode::Delete)
    }

    fn dossier_in(dir: &TempDir, mode: JournalMode) -> PathBuf {
        let path = dir.path().join("dossier.db");
        let mut j = Journal::create_with_mode(
            &path,
            Uuid::from_u128(JID),
            &ts("2026-09-28T07:00:00Z"),
            mode,
        )
        .expect("create");
        j.put_study_with_history(&study(), &ts("2026-09-28T08:00:00Z"))
            .expect("study");
        drop(j);
        path
    }

    fn file_hash(path: &Path) -> String {
        steadyinvest_contract::sha256_hex(&std::fs::read(path).expect("dossier bytes"))
    }

    fn submission(path: &Path) -> DraftSubmission {
        DraftSubmission {
            id: Uuid::from_u128(0x83_d001),
            created_at: ts("2026-09-28T09:00:00Z"),
            kind: DraftKind::Note,
            study_id: Some(Uuid::from_u128(STUDY)),
            security_ticker: None,
            native_currency: None,
            company_name: None,
            target: None,
            proposed_value: None,
            note_text: Some("Marge en hausse".to_string()),
            comment: "vu dans le rapport annuel".to_string(),
            origin: DraftOrigin {
                client: "claude-code".to_string(),
                model: "test-model".to_string(),
            },
            dossier: DossierIdentity {
                journal_id: Uuid::from_u128(JID),
                path: path.to_path_buf(),
            },
            method_version: "ssg-1.2.0".to_string(),
        }
    }

    fn ctx<'a>(action: AuthAction<'a>, accessor: Option<&'a str>) -> AuthContext<'a> {
        AuthContext {
            action,
            database_name: Some("main"),
            accessor,
        }
    }

    #[test]
    fn the_read_policy_allows_the_allowlist_only() {
        for table in MCP_READABLE_TABLES.iter().chain(SQLITE_INTERNAL_TABLES) {
            let read = AuthAction::Read {
                table_name: table,
                column_name: "id",
            };
            assert_eq!(
                read_policy(&ctx(read, None)),
                Authorization::Allow,
                "{table}"
            );
        }
        for table in MCP_DENIED_TABLES.iter().chain(&["some_later_table"]) {
            // An empty column name is how `SELECT count(*)` reads a table — denied by table name.
            for column in ["id", ""] {
                let read = AuthAction::Read {
                    table_name: table,
                    column_name: column,
                };
                assert_eq!(
                    read_policy(&ctx(read, None)),
                    Authorization::Deny,
                    "{table}"
                );
            }
        }
        assert_eq!(
            read_policy(&ctx(
                AuthAction::Pragma {
                    pragma_name: "user_version",
                    pragma_value: None,
                },
                None
            )),
            Authorization::Allow,
            "reading user_version (the schema re-check) is allowed"
        );
        assert_eq!(
            read_policy(&ctx(
                AuthAction::Pragma {
                    pragma_name: "user_version",
                    pragma_value: Some("9"),
                },
                None
            )),
            Authorization::Deny,
            "setting it is not"
        );
        for allowed in [
            AuthAction::Select,
            AuthAction::Recursive,
            AuthAction::Function {
                function_name: "lower",
            },
            AuthAction::Transaction {
                operation: TransactionOperation::Begin,
            },
        ] {
            assert_eq!(read_policy(&ctx(allowed, None)), Authorization::Allow);
        }
    }

    #[test]
    fn the_read_policy_denies_every_write_and_every_schema_or_file_action() {
        let denied = [
            AuthAction::Insert {
                table_name: "ai_drafts",
            },
            AuthAction::Update {
                table_name: "journal_meta",
                column_name: "logical_version",
            },
            AuthAction::Delete {
                table_name: "studies",
            },
            AuthAction::Pragma {
                pragma_name: "journal_mode",
                pragma_value: Some("DELETE"),
            },
            AuthAction::Attach {
                filename: "/tmp/x.db",
            },
            AuthAction::Detach { database_name: "x" },
            AuthAction::CreateTable { table_name: "x" },
            AuthAction::CreateTempTable { table_name: "x" },
            AuthAction::DropTable {
                table_name: "studies",
            },
            AuthAction::AlterTable {
                database_name: "main",
                table_name: "studies",
            },
            AuthAction::CreateTrigger {
                trigger_name: "t",
                table_name: "studies",
            },
            AuthAction::CreateView { view_name: "v" },
            AuthAction::Reindex { index_name: "i" },
            AuthAction::Analyze {
                table_name: "studies",
            },
            AuthAction::CreateVtable {
                table_name: "v",
                module_name: "fts5",
            },
            AuthAction::Savepoint {
                operation: TransactionOperation::Begin,
                savepoint_name: "s",
            },
            AuthAction::Unknown {
                code: 99,
                arg1: None,
                arg2: None,
            },
        ];
        for action in denied {
            assert_eq!(
                read_policy(&ctx(action, None)),
                Authorization::Deny,
                "{action:?}"
            );
            if !matches!(
                action,
                AuthAction::Insert { .. } | AuthAction::Update { .. }
            ) {
                assert_eq!(
                    draft_policy(&ctx(action, None)),
                    Authorization::Deny,
                    "{action:?}"
                );
            }
        }
    }

    #[test]
    fn the_draft_policy_allows_the_insert_and_the_counter_only_from_the_trigger() {
        let insert = AuthAction::Insert {
            table_name: "ai_drafts",
        };
        assert_eq!(draft_policy(&ctx(insert, None)), Authorization::Allow);
        let bump = AuthAction::Update {
            table_name: "journal_meta",
            column_name: "logical_version",
        };
        assert_eq!(
            draft_policy(&ctx(bump, Some("trg_ai_drafts_bump_logical_version"))),
            Authorization::Allow
        );
        // A direct update of the counter (top-level SQL) or from any other trigger is denied.
        assert_eq!(draft_policy(&ctx(bump, None)), Authorization::Deny);
        assert_eq!(
            draft_policy(&ctx(bump, Some("trg_something_else"))),
            Authorization::Deny
        );
        for other in [
            AuthAction::Insert {
                table_name: "studies",
            },
            AuthAction::Update {
                table_name: "ai_drafts",
                column_name: "status",
            },
            AuthAction::Delete {
                table_name: "ai_drafts",
            },
            AuthAction::Update {
                table_name: "journal_meta",
                column_name: "journal_id",
            },
        ] {
            assert_eq!(
                draft_policy(&ctx(other, None)),
                Authorization::Deny,
                "{other:?}"
            );
        }
    }

    #[test]
    fn every_table_of_the_latest_schema_is_classified() {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier(&dir);
        let conn = Connection::open(&path).expect("raw open");
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .expect("catalogue");
        let tables: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .expect("names")
            .collect::<rusqlite::Result<_>>()
            .expect("collect");
        assert!(!tables.is_empty());
        for table in tables {
            let readable = MCP_READABLE_TABLES.contains(&table.as_str())
                || SQLITE_INTERNAL_TABLES.contains(&table.as_str());
            let denied = MCP_DENIED_TABLES.contains(&table.as_str());
            assert!(
                readable ^ denied,
                "table {table} is not classified for the MCP access surface (arch A3): put it \
                 in MCP_READABLE_TABLES or MCP_DENIED_TABLES on purpose"
            );
        }
    }

    /// Every statement that is not a study read or a draft insert fails at preparation with a
    /// typed denial, on the read connection AND on the draft connection, and the file's bytes do
    /// not change (NFR-A1, FR14, FR68 [P4]).
    #[test]
    fn every_other_statement_is_denied_by_the_engine_and_the_dossier_is_unchanged() {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier(&dir);
        let other = dir.path().join("other.db");
        let copy = dir.path().join("copy.db");
        let attach = format!("ATTACH DATABASE '{}' AS o", other.display());
        let statements = [
            "INSERT INTO studies (id, journal_id, security_ticker, created_at, status, \
             schema_version, payload) VALUES ('x','x','X','t','active',1,'{}')",
            "UPDATE studies SET payload = '{}'",
            "UPDATE studies SET status = 'archived'",
            "DELETE FROM studies",
            "UPDATE judgments SET payload = '{}'",
            "DELETE FROM judgments",
            "INSERT INTO portfolios (id, name, created_at) VALUES ('p','P','t')",
            "UPDATE holdings SET quantity = '0'",
            "DELETE FROM transactions",
            "UPDATE watchlist_items SET position = 0",
            "DELETE FROM fx_rates",
            "DELETE FROM price_history",
            "UPDATE ai_drafts SET status = 'rejected'",
            "DELETE FROM ai_drafts",
            "UPDATE journal_meta SET logical_version = 0",
            "UPDATE journal_meta SET journal_id = 'x'",
            "CREATE TABLE intruder (a)",
            "CREATE TEMP TABLE intruder (a)",
            "DROP TABLE studies",
            "ALTER TABLE studies ADD COLUMN x TEXT",
            "CREATE TRIGGER t AFTER INSERT ON ai_drafts BEGIN DELETE FROM studies; END",
            "CREATE VIEW v AS SELECT * FROM studies",
            "CREATE INDEX i ON studies(created_at)",
            "DROP INDEX idx_ai_drafts_status",
            &attach,
            &format!("VACUUM INTO '{}'", copy.display()),
            "VACUUM",
            "PRAGMA journal_mode = WAL",
            "PRAGMA user_version = 9",
            "PRAGMA foreign_keys = OFF",
            "PRAGMA writable_schema = ON",
            "ANALYZE",
            "REINDEX",
            "SELECT * FROM holdings",
            "SELECT count(*) FROM transactions",
            "SELECT s.id FROM studies s JOIN watchlist_items w ON w.study_id = s.id",
            "SAVEPOINT s",
            // F8 — further paths a statement could take.
            "DELETE FROM journal_meta",
            "INSERT INTO journal_meta (id, journal_id, logical_version, created_at) \
             VALUES (2, 'x', 0, 't')",
            "DROP TRIGGER trg_ai_drafts_bump_logical_version",
            "DROP TRIGGER trg_ai_drafts_refuse_existing_id",
            "PRAGMA wal_checkpoint(TRUNCATE)",
            "PRAGMA table_info(holdings)",
            "WITH x AS (SELECT * FROM holdings) SELECT * FROM x",
            "INSERT INTO ai_drafts (id, kind, study_id, security_ticker, native_currency, status, \
             created_at, comment, origin_client, origin_model, payload) \
             SELECT lower(hex(randomblob(16))), 'study', NULL, security_ticker, 'CHF', \
             'pending', 't', quantity, 'c', 'm', '{}' FROM holdings",
        ];
        // The denial each key statement must name (action, object prefix) — on both connections.
        let expected: &[(&str, &str, &str)] = &[
            (
                "UPDATE journal_meta SET logical_version = 0",
                "update",
                "journal_meta.logical_version",
            ),
            ("PRAGMA user_version = 9", "pragma", "user_version=9"),
            ("SELECT * FROM holdings", "read", "holdings."),
            ("DELETE FROM ai_drafts", "delete", "ai_drafts"),
            ("DROP TABLE studies", "delete", "sqlite_master"),
            // SQLite authorizes the catalogue write first — the first denial stops the statement.
            ("CREATE TABLE intruder (a)", "insert", "sqlite_master"),
            ("PRAGMA journal_mode = WAL", "pragma", "journal_mode=WAL"),
        ];
        let before = file_hash(&path);
        let access = McpAccess::at(&path);
        // Refused by SQLite itself before any authorization (the catalogue is read-only unless
        // `writable_schema`, which the authorizer denies): an error, and the file unchanged.
        for policy in [Policy::Read, Policy::Draft] {
            let gated = access.open(policy, &mut || {}).expect("gated open");
            gated
                .conn
                .execute_batch("UPDATE sqlite_master SET sql = ''")
                .expect_err("the catalogue is not writable");
        }
        for policy in [Policy::Read, Policy::Draft] {
            for sql in statements {
                let gated = access.open(policy, &mut || {}).expect("gated open");
                // Executed, not only prepared: some actions (the ATTACH behind `VACUUM INTO`) are
                // authorized when the statement runs.
                let result = gated.conn.execute_batch(sql);
                let err = result.expect_err(&format!("{policy:?} must deny {sql}"));
                match map_sqlite(err, &gated.denials) {
                    Error::McpDenied { denials } => {
                        assert!(!denials.is_empty(), "{policy:?} {sql}: denial recorded");
                        if let Some((_, action, object)) =
                            expected.iter().find(|(stmt, _, _)| *stmt == sql)
                        {
                            assert!(
                                denials
                                    .iter()
                                    .any(|d| d.action == *action && d.object.starts_with(object)),
                                "{policy:?} {sql}: expected a {action} {object}… denial, got \
                                 {denials:?}"
                            );
                        }
                    }
                    other => panic!("{policy:?} {sql}: expected a typed denial, got {other:?}"),
                }
            }
        }
        assert_eq!(
            file_hash(&path),
            before,
            "no denied statement touched the dossier"
        );
        assert!(
            !other.exists() && !copy.exists(),
            "no file was attached or written"
        );
        assert!(
            !path.with_file_name("dossier.db-lock").exists(),
            "the MCP access never takes the app's instance lock"
        );
    }

    #[test]
    fn the_draft_connection_inserts_a_draft_and_only_the_trigger_bumps_the_counter() {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier(&dir);
        let before = Journal::open(&path)
            .expect("open")
            .logical_version()
            .expect("v");
        let access = McpAccess::at(&path);
        access
            .submit_draft(&submission(&path))
            .expect("a well-formed note draft is accepted");
        let after = Journal::open(&path)
            .expect("open")
            .logical_version()
            .expect("v");
        assert_eq!(after, before + 1, "one inserted draft, one trigger bump");
    }

    /// A11 — a restore swaps the file between the submission's open and its transaction: the
    /// submission is refused `dossier_replaced`, and the restored file carries no draft from it.
    #[test]
    fn a_restore_during_a_submission_refuses_it_and_the_restored_file_has_no_draft() {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier(&dir);
        let backup = dir.path().join("backup.db");
        Journal::open(&path)
            .expect("open")
            .backup_to(&backup)
            .expect("backup");
        let access = McpAccess::at(&path);
        let outcome = access.submit_draft_with(
            &submission(&path),
            SubmitHooks {
                after_open: &mut || {
                    crate::restore::restore_journal_file(&path, &backup).expect("restore");
                },
                in_transaction: &mut || {},
            },
            &|| false,
        );
        match outcome {
            Err(SubmitError::Refused(SubmissionRefusal::DossierReplaced)) => {}
            other => panic!("expected dossier_replaced, got {other:?}"),
        }
        let drafts = McpAccess::at(&path)
            .list_drafts(DraftFilter::default(), Page::first(10))
            .expect("read the restored dossier");
        assert_eq!(drafts.total, 0, "no draft reached the restored file");
        let reopened = Journal::open(&path).expect("the restored dossier opens");
        assert!(!reopened.is_read_only(), "its sidecars were not damaged");
    }

    /// A11 — a restore waits for a write in progress on the live file and refuses by name when it
    /// does not end: the live dossier is untouched.
    #[test]
    fn a_restore_refuses_while_a_write_holds_the_live_file() {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier(&dir);
        let backup = dir.path().join("backup.db");
        Journal::open(&path)
            .expect("open")
            .backup_to(&backup)
            .expect("backup");
        let writer = Connection::open(&path).expect("writer");
        writer
            .execute_batch("BEGIN IMMEDIATE")
            .expect("a write in progress");
        let before = file_hash(&path);
        let err = crate::restore::restore_journal_file_with_wait(
            &path,
            &backup,
            std::time::Duration::from_millis(150),
        )
        .expect_err("the restore waits, then refuses");
        assert!(matches!(err, Error::Restore { .. }), "{err:?}");
        assert_eq!(file_hash(&path), before, "the live dossier is untouched");
        writer.execute_batch("ROLLBACK").expect("end");
    }

    #[test]
    fn the_denials_text_names_each_action() {
        let text = denials_text(&[
            McpDenial {
                action: "update".into(),
                object: "studies.payload".into(),
            },
            McpDenial {
                action: "pragma".into(),
                object: "user_version=9".into(),
            },
        ]);
        assert_eq!(text, "update studies.payload, pragma user_version=9");
        assert_eq!(denials_text(&[]), "a statement");
        assert_eq!(SCHEMA_VERSION, steadyinvest_contract::SCHEMA_VERSION);
    }

    // ── Story 8.3 G3 ──────────────────────────────────────────────────────────────────────────

    /// M3: an `INSERT OR REPLACE` / `REPLACE INTO` over an existing draft (the authorizer cannot
    /// see a conflict clause) is refused by the v9 id guard; the decided draft stays as decided.
    #[test]
    fn a_replace_over_an_existing_draft_is_refused_and_the_decided_row_stays() {
        use crate::drafts::{DraftDecisionWrite, DraftVerdict};
        let dir = TempDir::new().expect("tempdir");
        let path = dossier(&dir);
        let access = McpAccess::at(&path);
        let id = access.submit_draft(&submission(&path)).expect("draft");
        Journal::open(&path)
            .expect("open")
            .decide_draft(DraftDecisionWrite {
                draft_id: id,
                verdict: DraftVerdict::Rejected,
                decided_at: &ts("2026-09-28T10:00:00Z"),
                stale_at_decision: None,
                edited_before_validation: None,
                study: None,
            })
            .expect("rejected");
        for verb in ["INSERT OR REPLACE INTO", "REPLACE INTO"] {
            let gated = access.open(Policy::Draft, &mut || {}).expect("gated");
            let sql = format!(
                "{verb} ai_drafts (id, kind, study_id, security_ticker, native_currency, status, \
                 created_at, comment, origin_client, origin_model, payload) \
                 VALUES ('{id}', 'note', '{}', 'NESN.SW', NULL, 'pending', \
                 '2026-09-28T11:00:00Z', 'x', 'c', 'm', '{{\"version\":1,\"note_text\":\"x\"}}')",
                Uuid::from_u128(STUDY)
            );
            let err = gated.conn.execute_batch(&sql).expect_err("refused");
            match map_sqlite(err, &gated.denials) {
                Error::Sqlite(rusqlite::Error::SqliteFailure(f, _)) => {
                    assert_eq!(f.code, rusqlite::ErrorCode::ConstraintViolation, "{verb}")
                }
                other => panic!("{verb}: expected the id guard, got {other:?}"),
            }
        }
        let stored = Journal::open(&path)
            .expect("open")
            .get_draft(id)
            .expect("read")
            .expect("present");
        assert_eq!(
            stored.status,
            DraftStatus::Rejected,
            "the decided draft is intact"
        );
    }

    /// The child side of the cross-process probe: a no-op in a normal run; spawned with
    /// `STEADYINVEST_MCP_PROBE_DB`, it tries to take the write lock at once and exits 0 (taken) or
    /// 3 (refused).
    #[test]
    fn cross_process_write_probe() {
        let Ok(path) = std::env::var("STEADYINVEST_MCP_PROBE_DB") else {
            return;
        };
        let conn = Connection::open(&path).expect("probe open");
        conn.busy_timeout(std::time::Duration::ZERO)
            .expect("no wait");
        let code = match conn.execute_batch("BEGIN IMMEDIATE") {
            Ok(()) => 0,
            Err(_) => 3,
        };
        std::process::exit(code);
    }

    /// Whether ANOTHER PROCESS can take the dossier's write lock right now.
    fn another_process_takes_the_write_lock(path: &Path) -> bool {
        let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "mcp_access::tests::cross_process_write_probe",
                "--test-threads=1",
            ])
            .env("STEADYINVEST_MCP_PROBE_DB", path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("the probe process runs");
        match status.code() {
            Some(0) => true,
            Some(3) => false,
            other => panic!("the probe process ended with {other:?}"),
        }
    }

    /// G3 CRITICAL 1: while a submission holds `BEGIN IMMEDIATE` — AFTER its file-identity check —
    /// another process cannot take the write lock (the check opened no descriptor of the dossier:
    /// closing one would release every POSIX lock of this process on it). DELETE and WAL.
    #[test]
    fn another_process_cannot_write_while_a_submission_holds_its_transaction() {
        for mode in [JournalMode::Delete, JournalMode::Wal] {
            let dir = TempDir::new().expect("tempdir");
            let path = dossier_in(&dir, mode);
            let mut taken = None;
            McpAccess::at(&path)
                .submit_draft_with(
                    &submission(&path),
                    SubmitHooks {
                        after_open: &mut || {},
                        in_transaction: &mut || {
                            taken = Some(another_process_takes_the_write_lock(&path));
                        },
                    },
                    &|| false,
                )
                .expect("accepted");
            assert_eq!(
                taken,
                Some(false),
                "{mode:?}: the write lock held against another process"
            );
            assert!(
                another_process_takes_the_write_lock(&path),
                "{mode:?}: and released after the call"
            );
        }
    }

    /// The control of the probe above (DELETE mode, Unix): opening and closing a plain descriptor of
    /// the dossier inside the transaction DOES release the lock — the hazard the identity check
    /// avoids, and proof that the probe can see it.
    #[cfg(unix)]
    #[test]
    fn the_probe_sees_a_lock_lost_to_a_stray_descriptor() {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier(&dir);
        let mut taken = None;
        McpAccess::at(&path)
            .submit_draft_with(
                &submission(&path),
                SubmitHooks {
                    after_open: &mut || {},
                    in_transaction: &mut || {
                        drop(std::fs::File::open(&path).expect("a stray descriptor"));
                        taken = Some(another_process_takes_the_write_lock(&path));
                    },
                },
                &|| false,
            )
            .expect("accepted");
        assert_eq!(
            taken,
            Some(true),
            "closing a stray descriptor released the lock"
        );
    }

    /// G3 HIGH 2 (WAL): a restore swaps the file right after the submission's connection opened:
    /// the submission is refused `dossier_replaced` and no draft is lost — or, if the restore could
    /// not take the file, the draft lands in the live dossier.
    #[test]
    fn a_restore_right_after_the_open_never_loses_a_draft_in_wal_mode() {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier_in(&dir, JournalMode::Wal);
        let backup = dir.path().join("backup.db");
        Journal::open(&path)
            .expect("open")
            .backup_to(&backup)
            .expect("backup");
        let mut restored = None;
        let outcome = McpAccess::at(&path).submit_draft_with(
            &submission(&path),
            SubmitHooks {
                after_open: &mut || {
                    restored = Some(crate::restore::restore_journal_file_with_wait(
                        &path,
                        &backup,
                        std::time::Duration::from_millis(300),
                    ));
                },
                in_transaction: &mut || {},
            },
            &|| false,
        );
        let drafts = McpAccess::at(&path)
            .list_drafts(DraftFilter::default(), Page::first(10))
            .expect("read the live dossier")
            .total;
        match (restored, outcome) {
            (Some(Ok(())), Err(SubmitError::Refused(SubmissionRefusal::DossierReplaced))) => {
                assert_eq!(drafts, 0, "the restored file carries no draft")
            }
            (Some(Err(Error::Restore { .. })), Ok(_)) => {
                assert_eq!(
                    drafts, 1,
                    "the restore was refused; the draft is in the live file"
                )
            }
            other => panic!("a draft could be lost: {other:?}"),
        }
    }

    /// G3 M4 (WAL): a restore holds its lock while a submission starts; the submission waits on
    /// it, and ends refused (`dossier_replaced` / a restore in progress) — or, had it opened after
    /// the swap, accepted INTO the restored file. Never accepted and lost.
    #[test]
    fn a_submission_waiting_on_a_restore_lock_never_loses_its_draft() {
        use std::sync::Barrier;
        let dir = TempDir::new().expect("tempdir");
        let path = dossier_in(&dir, JournalMode::Wal);
        let backup = dir.path().join("backup.db");
        Journal::open(&path)
            .expect("open")
            .backup_to(&backup)
            .expect("backup");
        let barrier = Arc::new(Barrier::new(2));
        let (b, p, bk) = (Arc::clone(&barrier), path.clone(), backup.clone());
        let restore = std::thread::spawn(move || {
            crate::restore::restore_with(
                &p,
                &bk,
                None,
                std::time::Duration::from_secs(5),
                crate::restore::RestoreHooks {
                    after_lock: Some(&mut || {
                        b.wait();
                        // Best effort: give the submission time to reach the lock (the outcome is
                        // asserted for every interleaving).
                        std::thread::sleep(std::time::Duration::from_millis(150));
                    }),
                    after_snapshot: None,
                    after_swap: None,
                },
            )
        });
        barrier.wait();
        let outcome = McpAccess::at(&path).submit_draft(&submission(&path));
        restore
            .join()
            .expect("restore thread")
            .expect("the restore succeeds");
        let drafts = McpAccess::at(&path)
            .list_drafts(DraftFilter::default(), Page::first(10))
            .expect("read the restored dossier")
            .total;
        match outcome {
            Err(SubmitError::Refused(SubmissionRefusal::DossierReplaced))
            | Err(SubmitError::Failed(Error::McpUnavailable {
                reason: McpUnavailable::RestoreInProgress,
            })) => assert_eq!(drafts, 0),
            Ok(_) => assert_eq!(drafts, 1, "accepted into the restored file"),
            other => panic!("unexpected outcome {other:?}"),
        }
    }

    /// G3 M4: a restore takes the live file out of WAL — refused by name while another connection
    /// still reads it (its `-wal` / `-shm` would alias the restored file's), done once it closes.
    #[test]
    fn a_restore_waits_for_readers_to_leave_wal_and_refuses_by_name_otherwise() {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier_in(&dir, JournalMode::Wal);
        let backup = dir.path().join("backup.db");
        Journal::open(&path)
            .expect("open")
            .backup_to(&backup)
            .expect("backup");
        let reader = Connection::open(&path).expect("reader");
        reader
            .execute_batch("BEGIN; SELECT count(*) FROM studies;")
            .expect("a read in progress");
        let err = crate::restore::restore_journal_file_with_wait(
            &path,
            &backup,
            std::time::Duration::from_millis(200),
        )
        .expect_err("a reader holds the WAL");
        assert!(matches!(err, Error::Restore { .. }), "{err:?}");
        drop(reader);
        crate::restore::restore_journal_file_with_wait(
            &path,
            &backup,
            std::time::Duration::from_millis(500),
        )
        .expect("restored once the reader left");
        for suffix in ["-wal", "-shm"] {
            assert!(
                !dir.path().join(format!("dossier.db{suffix}")).exists(),
                "no {suffix} survives the swap"
            );
        }
        let reopened = Journal::open(&path).expect("the restored dossier opens");
        assert!(!reopened.is_read_only());
    }

    /// G3 M4/N2: while a restore runs (its marker names a live process) the MCP access reads and
    /// writes nothing (`dossier_busy`); a marker or staging copy left by a process that is gone is
    /// `restore_interrupted` — until the app opens the dossier, which clears them (holding its lock).
    #[test]
    fn a_running_or_interrupted_restore_refuses_calls_until_the_app_clears_it() {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier(&dir);
        let marker = dir.path().join("dossier.db-restoring");
        let access = McpAccess::at(&path);
        // Running: the marker names this (live) process.
        let pid = std::process::id();
        let start = crate::journal::process_start_time(pid).unwrap_or(0);
        std::fs::write(&marker, format!("{pid} {start}")).expect("marker");
        match access.list_studies(Page::first(1)) {
            Err(
                e @ Error::McpUnavailable {
                    reason: McpUnavailable::RestoreInProgress,
                },
            ) => assert_eq!(e.mcp_code(), Some("dossier_busy")),
            other => panic!("expected dossier_busy, got {other:?}"),
        }
        let err = access
            .submit_draft(&submission(&path))
            .expect_err("refused");
        assert_eq!(err.code(), Some("dossier_busy"));
        // The app does not clear a live restore's marker.
        assert!(
            Journal::open(&path)
                .expect("open")
                .cleared_restore_leftovers()
                .is_empty()
        );
        assert!(marker.exists());
        // Interrupted: a dead owner's marker, and a staging copy.
        std::fs::write(&marker, "4294967294 1").expect("dead marker");
        std::fs::write(dir.path().join("dossier.db-restore-incoming"), b"x").expect("staging");
        match access.list_studies(Page::first(1)) {
            Err(
                e @ Error::McpUnavailable {
                    reason: McpUnavailable::RestoreInterrupted,
                },
            ) => assert_eq!(e.mcp_code(), Some("restore_interrupted")),
            other => panic!("expected restore_interrupted, got {other:?}"),
        }
        // The app's next open clears both (and reports them for its log); MCP works again.
        let cleared = Journal::open(&path)
            .expect("open")
            .cleared_restore_leftovers()
            .to_vec();
        assert_eq!(cleared.len(), 2, "{cleared:?}");
        assert_eq!(
            access.list_studies(Page::first(1)).expect("usable").total,
            1
        );
    }

    /// G3 N4: a call that meets a lock beyond its wait is named — not a raw SQLite busy.
    #[test]
    fn a_lock_outlasting_the_wait_is_named() {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier(&dir);
        let holder = Connection::open(&path).expect("holder");
        holder
            .execute_batch("BEGIN EXCLUSIVE")
            .expect("an exclusive lock (DELETE mode)");
        let access = McpAccess::at(&path);
        // The call waits its 5 s busy_timeout, then says why.
        match access.list_studies(Page::first(1)) {
            Err(
                e @ Error::McpUnavailable {
                    reason: McpUnavailable::Busy,
                },
            ) => assert_eq!(e.mcp_code(), Some("dossier_locked")),
            other => panic!("expected dossier_locked, got {other:?}"),
        }
        holder.execute_batch("ROLLBACK").expect("end");
    }

    /// G3 N1: an MCP call in the window right after the swap (lock still held, marker still there)
    /// is refused `dossier_busy` — it creates no side file the restore would then delete — and the
    /// restore deletes NOTHING after the rename: a draft accepted after the restore survives.
    #[test]
    fn nothing_is_deleted_after_the_swap_and_calls_in_its_window_are_refused() {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier_in(&dir, JournalMode::Wal);
        let backup = dir.path().join("backup.db");
        Journal::open(&path)
            .expect("open")
            .backup_to(&backup)
            .expect("backup");
        let mut in_window = None;
        crate::restore::restore_with(
            &path,
            &backup,
            None,
            std::time::Duration::from_secs(2),
            crate::restore::RestoreHooks {
                after_lock: None,
                after_snapshot: None,
                after_swap: Some(&mut || {
                    in_window = Some(McpAccess::at(&path).submit_draft(&submission(&path)));
                }),
            },
        )
        .expect("restored");
        match in_window {
            Some(Err(e)) => assert_eq!(e.code(), Some("dossier_busy"), "{e:?}"),
            other => panic!("a call in the swap window must be refused: {other:?}"),
        }
        assert!(
            !dir.path().join("dossier.db-restoring").exists(),
            "the marker went last"
        );
        // After the restore: an MCP write lands, and nothing removes its side files behind it.
        let id = McpAccess::at(&path)
            .submit_draft(&submission(&path))
            .expect("accepted after the restore");
        let back = Journal::open(&path)
            .expect("the restored dossier opens")
            .get_draft(id)
            .expect("read");
        assert!(
            back.is_some(),
            "the draft accepted after the restore is there"
        );
    }

    /// G3 N3: the snapshot's byte copy closes no descriptor while the restore holds its lock —
    /// another PROCESS still cannot write once the snapshot is written (the POSIX-lock hazard).
    #[test]
    fn the_snapshot_copy_keeps_the_restore_lock_against_another_process() {
        let dir = TempDir::new().expect("tempdir");
        let path = dossier_in(&dir, JournalMode::Wal);
        let backup = dir.path().join("backup.db");
        Journal::open(&path)
            .expect("open")
            .backup_to(&backup)
            .expect("backup");
        let snapshot = dir.path().join("dossier.db-prerestore");
        let mut taken = None;
        crate::restore::restore_with(
            &path,
            &backup,
            Some(&snapshot),
            std::time::Duration::from_secs(2),
            crate::restore::RestoreHooks {
                after_lock: None,
                after_snapshot: Some(&mut || {
                    taken = Some(another_process_takes_the_write_lock(&path));
                }),
                after_swap: None,
            },
        )
        .expect("restored");
        assert_eq!(taken, Some(false), "the lock survived the snapshot copy");
        assert!(snapshot.exists());
    }

    #[test]
    fn named_sqlite_states_map_to_their_reasons() {
        let denials = Mutex::new(Vec::new());
        let failure =
            |code: i32| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), None);
        for (code, reason) in [
            (
                rusqlite::ffi::SQLITE_READONLY_ROLLBACK,
                McpUnavailable::NeedsRecovery,
            ),
            (
                rusqlite::ffi::SQLITE_READONLY_RECOVERY,
                McpUnavailable::NeedsRecovery,
            ),
            (
                rusqlite::ffi::SQLITE_READONLY_CANTINIT,
                McpUnavailable::NeedsRecovery,
            ),
            (rusqlite::ffi::SQLITE_NOTADB, McpUnavailable::NotADossier),
        ] {
            match map_sqlite(failure(code), &denials) {
                Error::McpUnavailable { reason: got } => assert_eq!(got, reason, "{code}"),
                other => panic!("{code}: {other:?}"),
            }
        }
        assert!(matches!(
            map_sqlite(failure(rusqlite::ffi::SQLITE_BUSY), &denials),
            Error::Sqlite(_)
        ));
    }
}
