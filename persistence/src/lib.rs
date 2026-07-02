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
//! (`journal`), and the local price-history cache (`price_history`).

mod error;
mod export;
mod holdings;
mod journal;
mod migrations;
mod price_history;
mod restore;
mod schema;
mod studies;
mod transactions;
mod util;
mod watchlist;

pub use error::{Error, Result};
pub use export::{ImportSummary, JournalExport, JournalSnapshot, StudyRecord};
pub use holdings::{DeletePortfolioOutcome, HoldingItem, PortfolioItem};
pub use journal::{clear_lock, lock_is_stale, Journal, JournalMode};
pub use restore::{inspect_backup, restore_journal_file, BackupInfo};
pub use studies::StudySummary;
pub use transactions::{LedgerEntry, TransactionItem, KIND_BUY, KIND_DIVIDEND, KIND_SELL};
pub use watchlist::WatchItem;
