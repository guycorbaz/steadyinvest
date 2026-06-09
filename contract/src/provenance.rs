//! Provenance — the dated proof attached to every asserted fact, realizing the Foundational
//! Invariant at the type level: `(source, logical_version, timestamp, hash_of_dependencies)`.
//!
//! **Validation contract:** `Timestamp` (RFC3339 UTC) and `hash_of_dependencies` (hex digest) are
//! stored as plain `String`s; `contract` performs no format validation (it stays free of any
//! time/hash dependency). The **producing layers validate at construction**: the injected `Clock`
//! (app/core) emits RFC3339 UTC timestamps; the recompute path emits the hex digest. The contract's
//! job is only to carry the canonical strings. (Validating constructors / `TryFrom` may be added when
//! those layers land — see deferred-work.)

use crate::cell::Source;
use serde::{Deserialize, Serialize};

/// An RFC3339 UTC timestamp, stored as a string so `contract` stays free of any time/clock
/// dependency (the actual clock is injected in `app`/`core`). Example: `2026-06-09T14:30:00Z`.
/// Not validated here — see the module-level validation contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamp(pub String);

/// Dated proof of how a fact was produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source: Source,
    pub logical_version: u64,
    pub timestamp: Timestamp,
    /// Hex digest (e.g. SHA-256) of the inputs this fact descends from — a `String` so `contract`
    /// needs no hashing crate. NOTE: when computing this over serialized cells, **normalize** decimal
    /// values first — `Money` equality is by value but its serialization preserves scale
    /// (`"3.0"` ≠ `"3"`), so value-equal inputs can otherwise produce different digests.
    pub hash_of_dependencies: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_round_trips() {
        let p = Provenance {
            source: Source::Provider,
            logical_version: 42,
            timestamp: Timestamp("2026-06-09T14:30:00Z".to_string()),
            hash_of_dependencies: "abc123".to_string(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Provenance>(&json).unwrap(), p);
    }
}
