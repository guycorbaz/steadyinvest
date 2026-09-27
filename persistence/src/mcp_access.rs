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
//!   log (this crate has no logger).
//!
//! **Per call, lock-free, version-gated (A2).** A call opens the dossier, works and closes it. It
//! never takes the app's single-instance lock, never migrates, never sets `journal_mode`; it sets
//! only `busy_timeout` and `foreign_keys = ON`, and refuses a dossier whose `user_version` is not
//! exactly this build's ([`Error::McpSchemaMismatch`]). A read on a closed WAL dossier leaves an
//! empty `-wal` and a `-shm` beside it (SQLite needs them to read WAL, and a read-only connection
//! cannot remove them) — expected and harmless: they are writable (the file's own mode) and the
//! app's next open uses them normally.
//!
//! **Identity is the caller's (ADD15).** Nothing here calls a clock or a UUID generator: a draft's
//! id, its `created_at` and the method version its fingerprint is computed under come from the
//! caller.
//!
//! **Submissions (A3, D2 / D4 / D6 / D8 / D10, A11).** A draft's checks and its insert run in ONE
//! `BEGIN IMMEDIATE` transaction: a refused draft writes nothing and is a typed
//! [`SubmissionRefusal`] with a stable snake-case [`code`](SubmissionRefusal::code) (the MCP server
//! renders the French message — Story 8.0 §3.3). Inside that transaction the file at the resolved
//! path is re-checked against the one the call opened (a restore swaps it — A11).

use crate::drafts::{
    DRAFT_COLUMNS, DraftRecord, check_payload, is_currency_code, is_rfc3339_utc, record_from_row,
    row_tuple,
};
use crate::error::{Error, Result};
use crate::journal::{apply_connection_local_pragmas, read_journal_id, resolved_path};
use crate::migrations;
use crate::schema::{DRAFT_TRIGGER, MCP_READABLE_TABLES, SQLITE_INTERNAL_TABLES};
use crate::studies::{
    StudySummary, list_studies_in, parse_study_row, read_study_in, study_status_in,
};
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
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
    /// The comment is blank (NFR-A4).
    EmptyComment,
    /// The client or the model is blank (NFR-A4).
    MissingOrigin,
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
}

impl SubmissionRefusal {
    /// The stable code the MCP tool returns (Story 8.0 §3.3; `value_out_of_range` and
    /// `empty_note_text` added by Story 8.3).
    pub fn code(&self) -> &'static str {
        match self {
            SubmissionRefusal::DossierMismatch { .. } => "dossier_mismatch",
            SubmissionRefusal::DossierReplaced => "dossier_replaced",
            SubmissionRefusal::StudyNotFound { .. } => "study_not_found",
            SubmissionRefusal::EmptyComment => "empty_comment",
            SubmissionRefusal::MissingOrigin => "missing_origin",
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
        }
    }
}

/// The outcome of a refused or failed submission: a typed refusal (the AI's proposal does not pass
/// a check — nothing written) or a failure (the dossier could not be read or written).
#[derive(Debug)]
pub enum SubmitError {
    Refused(SubmissionRefusal),
    Failed(Error),
}

impl From<Error> for SubmitError {
    fn from(e: Error) -> Self {
        SubmitError::Failed(e)
    }
}

impl From<rusqlite::Error> for SubmitError {
    fn from(e: rusqlite::Error) -> Self {
        SubmitError::Failed(Error::Sqlite(e))
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

/// A connection with its authorizer installed and the denials it recorded.
struct Gated {
    conn: Connection,
    denials: Arc<Mutex<Vec<McpDenial>>>,
}

impl Gated {
    /// Map a statement failure: a preparation the authorizer denied becomes [`Error::McpDenied`]
    /// naming each denial; anything else stays the SQLite error it is.
    fn map(&self, e: rusqlite::Error) -> Error {
        let denied = matches!(
            &e,
            rusqlite::Error::SqliteFailure(code, _)
                if code.code == rusqlite::ErrorCode::AuthorizationForStatementDenied
        );
        if denied {
            let denials = self
                .denials
                .lock()
                .map(|d| d.clone())
                .unwrap_or_else(|poisoned| poisoned.into_inner().clone());
            Error::McpDenied { denials }
        } else {
            Error::Sqlite(e)
        }
    }
}

fn is_readable(table: &str) -> bool {
    MCP_READABLE_TABLES.contains(&table) || SQLITE_INTERNAL_TABLES.contains(&table)
}

/// The read connection's policy (A3): allowlisted reads, `SELECT`, functions, transactions.
/// Everything else — and any action this rusqlite version does not name (`#[non_exhaustive]`) —
/// is denied.
pub(crate) fn read_policy(ctx: &AuthContext<'_>) -> Authorization {
    match ctx.action {
        AuthAction::Read { table_name, .. } if is_readable(table_name) => Authorization::Allow,
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

/// A ticker as the 8.0 spec's `identifier_invalid` rule has it: `[A-Z0-9.\-]{1,20}`.
fn is_ticker(ticker: &str) -> bool {
    (1..=20).contains(&ticker.len())
        && ticker
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
}

/// Two tickers name the same security: compared ignoring ASCII case and surrounding spaces — the
/// app's `same_ticker` rule (G1 P review L-h).
fn same_ticker(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
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

    /// Open one gated connection: flags (never `CREATE`), connection-local pragmas, the version
    /// gate, THEN the authorizer — nothing is prepared before the gate is in place except the
    /// pragmas and the `user_version` read.
    fn open(&self, policy: Policy) -> Result<Gated> {
        let flags = match policy {
            Policy::Read => OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            Policy::Draft => OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        };
        let conn = Connection::open_with_flags(&self.path, flags)?;
        apply_connection_local_pragmas(&conn)?;
        let file_user_version = migrations::user_version(&conn)?;
        let supported = migrations::latest_version(migrations::REGISTRY);
        if file_user_version != i64::from(supported) {
            return Err(Error::McpSchemaMismatch {
                file_user_version,
                supported,
            });
        }
        let denials = Arc::new(Mutex::new(Vec::new()));
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
        Ok(Gated { conn, denials })
    }

    /// Run `f` in one short read transaction on a fresh read connection (A2).
    fn read<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let mut gated = self.open(Policy::Read)?;
        let tx = gated
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(Error::Sqlite)?;
        let out = f(&tx);
        drop(tx); // a read transaction: rolling back is committing nothing
        out.map_err(|e| match e {
            Error::Sqlite(inner) => gated.map(inner),
            other => other,
        })
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
        let bounded = page.bounded();
        self.read(|conn| {
            let total: i64 = conn.query_row("SELECT COUNT(*) FROM studies", [], |r| r.get(0))?;
            Ok(Paged {
                items: list_studies_in(conn, Some(bounded))?,
                offset: bounded.1,
                total: u64::try_from(total).unwrap_or(0),
            })
        })
    }

    /// One study (FR69): the whole contract study and its status, `None` when the dossier holds
    /// no such study.
    pub fn read_study(&self, id: Uuid) -> Result<Option<McpStudyRead>> {
        self.read(|conn| {
            let Some(study) = read_study_in(conn, id)? else {
                return Ok(None);
            };
            let status = study_status_in(conn, id)?.unwrap_or_else(|| "active".to_string());
            Ok(Some(McpStudyRead { study, status }))
        })
    }

    /// One page of a study's FR51 history, newest first (each entry the full study state).
    pub fn read_history(&self, study_id: Uuid, page: Page) -> Result<Paged<McpSnapshot>> {
        let (limit, offset) = page.bounded();
        self.read(|conn| {
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
        })
    }

    /// One page of the drafts record (FR77), filtered, ordered by `(created_at, id)`.
    pub fn list_drafts(&self, filter: DraftFilter, page: Page) -> Result<Paged<DraftRecord>> {
        let (limit, offset) = page.bounded();
        let study = filter.study_id.map(|s| s.to_string());
        let status = filter.status.map(DraftStatus::as_str);
        self.read(|conn| {
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
        })
    }

    /// Submit a draft (Story 8.3 AC 7): every check and the insert run in ONE `BEGIN IMMEDIATE`
    /// transaction; a refused draft writes nothing. Returns the draft's id (the caller's).
    pub fn submit_draft(&self, sub: &DraftSubmission) -> std::result::Result<Uuid, SubmitError> {
        self.submit_draft_after_open(sub, || {})
    }

    /// [`Self::submit_draft`] with a seam between the connection's open (and the file identity it
    /// takes) and the `IMMEDIATE` transaction — the window a restore can swap the file in (A11). The
    /// tests drive a real restore there; production passes a no-op.
    pub(crate) fn submit_draft_after_open(
        &self,
        sub: &DraftSubmission,
        after_open: impl FnOnce(),
    ) -> std::result::Result<Uuid, SubmitError> {
        if !is_rfc3339_utc(&sub.created_at.0) {
            return Err(Error::McpInvalidCall {
                detail: format!(
                    "created_at {:?} is not an RFC3339 UTC timestamp",
                    sub.created_at.0
                ),
            }
            .into());
        }
        let mut gated = self.open(Policy::Draft)?;
        // A11: the file this call opened — re-checked inside the transaction.
        let opened = same_file::Handle::from_path(&self.path).map_err(|e| {
            SubmitError::Failed(Error::Restore {
                detail: format!("the dossier file identity could not be read: {e}"),
            })
        })?;
        after_open();
        let outcome = {
            let tx = gated
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(Error::Sqlite)?;
            let result = self.check_and_insert(&tx, sub, &opened);
            match result {
                Ok(()) => tx
                    .commit()
                    .map_err(Error::Sqlite)
                    .map_err(SubmitError::Failed),
                Err(e) => Err(e), // the transaction rolls back on drop: nothing written
            }
        };
        outcome.map(|()| sub.id).map_err(|e| match e {
            SubmitError::Failed(Error::Sqlite(inner)) => SubmitError::Failed(gated.map(inner)),
            other => other,
        })
    }

    /// The submission checks, in the story's order (T5.3), then the insert — inside the caller's
    /// `IMMEDIATE` transaction.
    fn check_and_insert(
        &self,
        tx: &Connection,
        sub: &DraftSubmission,
        opened: &same_file::Handle,
    ) -> std::result::Result<(), SubmitError> {
        // 1. A11 — still the file this call opened (a restore renames another over it).
        let still_ours = same_file::Handle::from_path(&self.path).is_ok_and(|h| h == *opened);
        if !still_ours {
            return Err(SubmissionRefusal::DossierReplaced.into());
        }
        // 2. D10 — the dossier the AI read.
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
        // 3. NFR-A4 — comment and origin.
        if is_blank(&sub.comment) {
            return Err(SubmissionRefusal::EmptyComment.into());
        }
        if is_blank(&sub.origin.client) || is_blank(&sub.origin.model) {
            return Err(SubmissionRefusal::MissingOrigin.into());
        }
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
        )?;
        Ok(())
    }

    /// A draft study (D2, D8): identifier rule, then the two duplicate checks.
    #[allow(clippy::type_complexity)]
    fn draft_study(
        &self,
        tx: &Connection,
        sub: &DraftSubmission,
    ) -> std::result::Result<(Option<Uuid>, String, Option<String>, DraftPayload), SubmitError>
    {
        if sub.study_id.is_some() || sub.target.is_some() || sub.note_text.is_some() {
            return Err(invalid(
                "a draft study carries no study, target or note text",
            ));
        }
        let ticker = sub.security_ticker.clone().unwrap_or_default();
        let currency = sub.native_currency.clone().unwrap_or_default();
        if !is_ticker(&ticker) || !is_currency_code(&currency) {
            return Err(SubmissionRefusal::IdentifierInvalid { ticker, currency }.into());
        }
        // D2 — already studied in the same currency (archived studies count: still in the dossier).
        let mut stmt =
            tx.prepare("SELECT id, security_ticker FROM studies ORDER BY created_at, id")?;
        let candidates = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, stored_ticker) in candidates {
            if !same_ticker(&stored_ticker, &ticker) {
                continue;
            }
            let id = crate::util::parse_uuid(&id, "studies.id")?;
            let study = read_study_in(tx, id)?.ok_or_else(|| Error::CorruptPayload {
                detail: format!("study {id} vanished inside the submission transaction"),
            })?;
            if study.native_currency == currency {
                return Err(SubmissionRefusal::StudyExists {
                    ticker,
                    currency,
                    study_id: id,
                }
                .into());
            }
        }
        // D8 — already pending as a draft study in the same currency.
        let mut stmt = tx.prepare(
            "SELECT id, security_ticker FROM ai_drafts
             WHERE kind = 'study' AND status = 'pending' AND native_currency = ?1
             ORDER BY created_at, id",
        )?;
        let pending = stmt
            .query_map(rusqlite::params![currency], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if let Some((id, _)) = pending.iter().find(|(_, t)| same_ticker(t, &ticker)) {
            return Err(SubmissionRefusal::DraftStudyPending {
                ticker,
                currency,
                draft_id: crate::util::parse_uuid(id, "ai_drafts.id")?,
            }
            .into());
        }
        let company_name = sub.company_name.clone().filter(|name| !is_blank(name));
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

    /// The target study of a note / cell / judgment draft, looked up (never trusted): its id and
    /// the stored study.
    fn target_study(
        &self,
        tx: &Connection,
        sub: &DraftSubmission,
    ) -> std::result::Result<Study, SubmitError> {
        let study_id = sub
            .study_id
            .ok_or_else(|| invalid("a note, cell or judgment draft names its study"))?;
        read_study_in(tx, study_id)?
            .ok_or_else(|| SubmissionRefusal::StudyNotFound { study_id }.into())
    }

    /// A note draft: the study exists, the text is not blank.
    #[allow(clippy::type_complexity)]
    fn draft_note(
        &self,
        tx: &Connection,
        sub: &DraftSubmission,
    ) -> std::result::Result<(Option<Uuid>, String, Option<String>, DraftPayload), SubmitError>
    {
        if sub.target.is_some() || sub.proposed_value.is_some() || sub.security_ticker.is_some() {
            return Err(invalid(
                "a note draft carries no target, value or identifier",
            ));
        }
        let study = self.target_study(tx, sub)?;
        let text = sub
            .note_text
            .clone()
            .ok_or_else(|| invalid("a note draft carries its text"))?;
        if is_blank(&text) {
            return Err(SubmissionRefusal::EmptyNoteText.into());
        }
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

    /// A cell / judgment draft: the study exists; the field is draftable and of the draft's kind;
    /// a cell's year is a year of the study; the value parses in the field's unit; the target has
    /// no pending draft (D4); the base fingerprint is computed in this transaction (A7).
    #[allow(clippy::type_complexity)]
    fn draft_value(
        &self,
        tx: &Connection,
        sub: &DraftSubmission,
    ) -> std::result::Result<(Option<Uuid>, String, Option<String>, DraftPayload), SubmitError>
    {
        if sub.note_text.is_some() || sub.security_ticker.is_some() || sub.company_name.is_some() {
            return Err(invalid("a value draft carries no note text or identifier"));
        }
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
        let mut stmt = tx.prepare(
            "SELECT id, payload FROM ai_drafts
             WHERE study_id = ?1 AND status = 'pending' AND kind IN ('cell', 'judgment')
             ORDER BY created_at, id",
        )?;
        let pending = stmt
            .query_map(rusqlite::params![study.id.to_string()], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
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
        let path = dir.path().join("dossier.db");
        let mut j = Journal::create_with_mode(
            &path,
            Uuid::from_u128(JID),
            &ts("2026-09-28T07:00:00Z"),
            JournalMode::Delete,
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
            &format!("ATTACH DATABASE '{}' AS o", other.display()),
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
        ];
        let before = file_hash(&path);
        let access = McpAccess::at(&path);
        for policy in [Policy::Read, Policy::Draft] {
            for sql in statements {
                let gated = access.open(policy).expect("gated open");
                // Executed, not only prepared: some actions (the ATTACH behind `VACUUM INTO`) are
                // authorized when the statement runs.
                let result = gated.conn.execute_batch(sql);
                let err = result.expect_err(&format!("{policy:?} must deny {sql}"));
                match gated.map(err) {
                    Error::McpDenied { denials } => {
                        assert!(!denials.is_empty(), "{policy:?} {sql}: denial recorded")
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
        let outcome = access.submit_draft_after_open(&submission(&path), || {
            crate::restore::restore_journal_file(&path, &backup).expect("restore");
        });
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
}
