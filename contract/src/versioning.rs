//! Version axes for the data contract.
//!
//! There are **three** independent version axes in steadyinvest:
//! 1. **`SCHEMA_VERSION`** (this constant) — the serialized **data-contract** version (these serde
//!    types). Bumped together with a migration on any breaking change to the persisted/exported shapes.
//! 2. SQLite `PRAGMA user_version` — the on-disk SQL schema (lives in `steadyinvest-persistence`).
//! 3. `core::METHOD_VERSION` — the calculation semantics (lives in `steadyinvest-core`).
//!
//! Keep them distinct; never conflate `schema_version` with `method_version`.

/// Serialized data-contract schema version.
pub const SCHEMA_VERSION: u32 = 1;
