//! Provenance — the dated proof attached to every asserted fact, realizing the Foundational
//! Invariant at the type level: `(source, logical_version, timestamp, hash_of_dependencies)`.

use crate::cell::Source;
use serde::{Deserialize, Serialize};

/// An RFC3339 UTC timestamp, stored as a string so `contract` stays free of any time/clock
/// dependency (the actual clock is injected in `app`/`core`). Example: `2026-06-09T14:30:00Z`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamp(pub String);

/// Dated proof of how a fact was produced. `hash_of_dependencies` is a hex digest (e.g. SHA-256) of
/// the inputs the fact descends from — kept as a `String` so `contract` needs no hashing crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source: Source,
    pub logical_version: u64,
    pub timestamp: Timestamp,
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
