//! steadyinvest-contract — versioned serde data contract.
//!
//! The single shared vocabulary across `core`, `ingestion`, `persistence`, `report`, `app` and the
//! future read-only MCP/AI façade. It is decoupled from Slint and SQLite (serde / rust_decimal /
//! uuid only — no I/O).
//!
//! Conventions (binding — see `architecture.md` Format Patterns): serde JSON with `snake_case`
//! fields; new fields use `#[serde(default)]`; journal types do NOT use `deny_unknown_fields`
//! (forward-compatibility); money is exact [`Money`] serialized as a string; timestamps are RFC3339
//! UTC strings; tri-state review is an enum, never `0/1/2`.
//!
//! **Forward-compatibility policy.** New *fields* are tolerated in both directions (`#[serde(default)]`
//! together with no `deny_unknown_fields`). New *enum variants* are NOT silently tolerated: adding a
//! variant to any contract enum is a `schema_version` bump, and an older build encountering an unknown
//! enum value will fail to deserialize **on purpose** (fail-loud — an unknown `Source`/`Review` is a
//! data-correctness problem, not something to silently coerce to a fallback). Hence no
//! `non_exhaustive` / `serde(other)` on the domain enums.

pub mod cell;
pub mod money;
pub mod provenance;
pub mod study;
pub mod versioning;

// Portfolio / FX / export types arrive with their epics (Epic 4/6 portfolio & FX, Epic 5 export).

pub use cell::{Cell, Coverage, Freshness, PendingProvider, Review, Source};
pub use money::Money;
pub use provenance::{Provenance, Timestamp};
pub use study::{ForecastLowOption, Judgment, Study, YearData};
pub use versioning::SCHEMA_VERSION;
