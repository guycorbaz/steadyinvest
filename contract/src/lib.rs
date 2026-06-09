//! steadyinvest-contract — versioned serde data contract.
//!
//! The single shared vocabulary across `core`, `ingestion`, `persistence`, `report`, `app` and the
//! future read-only MCP/AI façade. It is decoupled from Slint and SQLite. The real types
//! (`Study`, `Judgment`, `Cell` with source × freshness × tri-state review, provenance, portfolio,
//! FX, export envelope) arrive in Story 1.3.

/// Serialized data-contract schema version. Bumped — together with a migration — on any breaking
/// change to the persisted/exported shapes.
pub const SCHEMA_VERSION: u32 = 1;
