//! steadyinvest-persistence — local SQLite journal (hybrid model).
//!
//! The **only** crate that touches SQLite (via bundled `rusqlite`). Normalized tables for
//! aggregated data, a versioned JSON blob for studies/judgments, journal identity, migrations and
//! export/import/restore arrive in Story 1.10. Money is stored as TEXT decimal strings, never as
//! `REAL`.

// Intentionally empty in the scaffold; the store lands in Story 1.10.
#![allow(unused_crate_dependencies)]
