//! Portable single-study export envelope (Story 5.2, FR59).
//!
//! A study's portable unit is **not** a raw `.db` copy — it is the serialized data contract (the
//! [`Study`] as JSON) wrapped in an envelope that carries the [`SCHEMA_VERSION`] it was written under
//! and a **SHA-256 integrity hash** over the canonical study bytes (architecture decision §"Export /
//! backup format"). This module owns the envelope + hashing only; **callers own file I/O** — the
//! `contract` surface stays serde + `rust_decimal` + `sha2`, no reading or writing of files.
//!
//! Round-trip: [`to_export_json`] → [`from_export_json`] yields an **equal** [`Study`] (identity
//! preserved — the `id` is part of the serialized study). Import **verifies** the hash (rejects
//! tamper/corruption) and the `schema_version` (rejects an unknown/newer version — a migration hook
//! is structured via [`ImportError::Version`], but with `SCHEMA_VERSION == 1` only the equal case
//! accepts; nothing is silently coerced).

use crate::study::Study;
use crate::versioning::SCHEMA_VERSION;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The on-disk export envelope. `payload` is the canonical serialized [`Study`] JSON; `integrity_hash`
/// is the lowercase-hex SHA-256 of `payload`'s UTF-8 bytes; `schema_version` is the study's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyExport {
    pub schema_version: u32,
    pub integrity_hash: String,
    pub payload: String,
}

/// Why an import was refused. Typed, never a panic, and carries **no secrets** (only versions and a
/// generic shape detail) — NFR-S1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The recomputed hash does not match the envelope's — tamper or corruption.
    Integrity,
    /// The envelope's `schema_version` is not the one this build supports (no silent coercion).
    Version { found: u32, supported: u32 },
    /// The string is not a valid export envelope, or its payload is not a valid study.
    Malformed(String),
}

/// Lowercase-hex SHA-256 of `bytes` (no `hex` crate dependency — formatted inline).
fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Serialize a [`Study`] into its portable export envelope JSON (Story 5.2, FR59). The hash is taken
/// over the **payload** (the study JSON), never over the envelope (which contains the hash).
pub fn to_export_json(study: &Study) -> String {
    // A struct serializes its fields in declaration order, so the same study yields the same bytes
    // (and hash) across runs/OSes — the canonical form. `Study` is always serializable.
    let payload = serde_json::to_string(study).expect("a Study always serializes");
    let envelope = StudyExport {
        schema_version: study.schema_version,
        integrity_hash: sha256_hex(payload.as_bytes()),
        payload,
    };
    serde_json::to_string(&envelope).expect("the envelope always serializes")
}

/// Parse + verify an export envelope back into a [`Study`] (Story 5.2, FR59/NFR-R5). Rejects a hash
/// mismatch ([`ImportError::Integrity`]), an unsupported `schema_version` ([`ImportError::Version`]),
/// or a malformed envelope/payload ([`ImportError::Malformed`]). Never panics.
pub fn from_export_json(text: &str) -> Result<Study, ImportError> {
    let envelope: StudyExport =
        serde_json::from_str(text).map_err(|e| ImportError::Malformed(e.to_string()))?;
    // Integrity first: a corrupt payload must not even be parsed as a study.
    if sha256_hex(envelope.payload.as_bytes()) != envelope.integrity_hash {
        return Err(ImportError::Integrity);
    }
    if envelope.schema_version != SCHEMA_VERSION {
        return Err(ImportError::Version {
            found: envelope.schema_version,
            supported: SCHEMA_VERSION,
        });
    }
    serde_json::from_str(&envelope.payload).map_err(|e| ImportError::Malformed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::money::Money;
    use crate::provenance::Timestamp;
    use crate::study::{ForecastLowOption, Judgment};
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn judgment() -> Judgment {
        Judgment {
            estimated_high_eps: None,
            estimated_low_eps: None,
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: None,
            forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
            recent_severe_low: None,
            current_price: Some(Money::from(Decimal::from_str_exact("100.50").unwrap())),
            present_full_year_dividend: None,
        }
    }

    fn sample() -> Study {
        let mut s = Study::new(
            Uuid::from_u128(0x5002),
            Uuid::from_u128(0xC0FFEE),
            "NESN",
            "CHF",
            judgment(),
            Timestamp("2026-06-29T00:00:00Z".to_string()),
        );
        s.rationale = Some("solid compounder".to_string());
        s
    }

    #[test]
    fn round_trip_preserves_an_equal_study() {
        let study = sample();
        let json = to_export_json(&study);
        let back = from_export_json(&json).expect("a freshly exported study re-imports");
        assert_eq!(
            back, study,
            "export → import yields an equal study (id preserved)"
        );
    }

    #[test]
    fn the_hash_is_deterministic_for_the_same_study() {
        let study = sample();
        assert_eq!(
            to_export_json(&study),
            to_export_json(&study),
            "the canonical export (and its hash) is stable"
        );
    }

    #[test]
    fn a_tampered_payload_is_rejected_for_integrity() {
        let json = to_export_json(&sample());
        // Flip a digit inside the payload (the ticker's currency or a number) without touching the
        // hash → the recomputed hash no longer matches.
        let tampered = json.replacen("NESN", "ROG0", 1);
        assert_ne!(tampered, json);
        assert_eq!(from_export_json(&tampered), Err(ImportError::Integrity));
    }

    #[test]
    fn an_unsupported_schema_version_is_rejected() {
        // Re-wrap the SAME payload under a bumped schema_version with a correct hash, so only the
        // version check can fire (not integrity).
        let study = sample();
        let payload = serde_json::to_string(&study).unwrap();
        let envelope = StudyExport {
            schema_version: SCHEMA_VERSION + 1,
            integrity_hash: sha256_hex(payload.as_bytes()),
            payload,
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert_eq!(
            from_export_json(&json),
            Err(ImportError::Version {
                found: SCHEMA_VERSION + 1,
                supported: SCHEMA_VERSION,
            })
        );
    }

    #[test]
    fn garbage_and_non_envelope_input_is_malformed_not_a_panic() {
        assert!(matches!(
            from_export_json("not json at all"),
            Err(ImportError::Malformed(_))
        ));
        assert!(matches!(
            from_export_json("{\"unrelated\": true}"),
            Err(ImportError::Malformed(_))
        ));
    }
}
