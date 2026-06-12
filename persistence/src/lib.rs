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
//! cause-named error on writes (NFR-R3). Export/import/backup and sync-guard are Epic 5.

mod error;
mod journal;
mod migrations;
mod schema;
mod studies;

pub use error::{Error, Result};
pub use journal::Journal;
pub use studies::StudySummary;
