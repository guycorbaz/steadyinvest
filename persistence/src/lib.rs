//! steadyinvest-persistence — local SQLite journal (hybrid model).
//!
//! The **only** crate that touches SQLite (via bundled `rusqlite`). Normalized tables for
//! aggregated data (DDL-only in v1), versioned serde-JSON blobs for `studies`/`judgments`,
//! journal identity (UUID + monotonic logical version, ADD6) and a `PRAGMA user_version`
//! migrations harness. Money is stored as TEXT decimal strings, **never** as `REAL` — no decimal
//! arithmetic happens in SQL, ever (NFR-C1).
//!
//! Identity and time are **caller-supplied** (ADD15): nothing here calls `Uuid::new_v4()` or a
//! clock. A journal file written by a newer schema opens **read-only** with a neutral,
//! cause-named error on writes (NFR-R3). Epic 5 added whole-journal export/import (`export`),
//! raw-file backup/restore (`restore`), the sync-folder guard + single-instance lock
//! (`journal`), and the local price-history cache (`price_history`). Epic 6 added the dated,
//! source-aware FX-rate store (`fx`, Story 6.5, FR28). Epic 8 adds the AI drafts table
//! (`drafts`, migration v8, Story 8.2a) and the gated MCP access surface (`mcp_access`, Story 8.3):
//! typed study reads and draft inserts behind a SQLite authorizer — never a connection.

mod drafts;
mod error;
mod export;
mod fx;
mod holdings;
mod journal;
mod mcp_access;
mod migrations;
mod price_history;
mod restore;
mod schema;
mod studies;
mod transactions;
mod util;
mod watchlist;

pub use drafts::{DraftDecisionWrite, DraftRecord, DraftStep, DraftVerdict, StudyWrite};
pub use error::{Error, ErrorKind, Result};
pub use export::{
    ImportSummary, JournalExport, JournalSnapshot, JudgmentSnapshotRecord, StudyRecord,
    inspect_journal_envelope,
};
pub use fx::FxRateItem;
pub use holdings::{DeletePortfolioOutcome, HoldingItem, PortfolioItem};
pub use journal::{
    Journal, JournalMode, ReadOnlyCause, clear_lock, lock_is_stale, resolved_path,
    sweep_stale_read_copies,
};
pub use mcp_access::{
    DossierIdentity, DraftFilter, DraftSubmission, MAX_COMMENT_CHARS, MAX_COMPANY_NAME_CHARS,
    MAX_NOTE_CHARS, MAX_ORIGIN_CHARS, MAX_PAGE, McpAccess, McpDenial, McpSnapshot, McpStudyRead,
    McpUnavailable, Page, Paged, SubmissionRefusal, SubmitError,
};
pub use restore::{BackupInfo, inspect_backup, restore_journal_file};
pub use studies::{JudgmentSnapshotSummary, StudySummary};
pub use transactions::{KIND_BUY, KIND_DIVIDEND, KIND_SELL, LedgerEntry, TransactionItem};
pub use watchlist::WatchItem;
