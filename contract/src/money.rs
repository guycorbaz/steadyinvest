//! Exact monetary / decimal value for the data contract.
//!
//! [`Money`] wraps [`rust_decimal::Decimal`] and serializes to/from a JSON **string** (never a JSON
//! number / float), so precision is preserved exactly across persistence and export. This enforces
//! the "Decimal in JSON = string" contract in one place.
//!
//! Deserialization is **exact and canonical**: it uses `Decimal::from_str_exact` (errors instead of
//! silently rounding inputs beyond `Decimal`'s 28-digit precision) and rejects non-canonical spellings
//! (scientific notation `1e5`, leading `+`, `-0`, underscores) by requiring the input to equal its own
//! re-serialized form. Our own `serialize` always emits the canonical form, so anything we write
//! round-trips.
//!
//! **Equality vs serialization:** `Money` derives **value-based** `Eq`/`Ord`/`Hash` (via `Decimal`),
//! so `Money("3.0") == Money("3")`. But `serialize` **preserves scale**, so those two serialize to
//! different strings (`"3.0"` vs `"3"`). Therefore anything that hashes/compares *serialized bytes*
//! (e.g. a content hash feeding `Provenance.hash_of_dependencies`) MUST normalize first — do not
//! assume `a == b` implies identical JSON.

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// An exact decimal money/ratio value. Equality/ordering are **by numeric value** (via `Decimal`):
/// `Money` built from `"3.0"` equals one built from `"3"`. Serialization preserves scale (see module
/// docs) — do not hash serialized bytes assuming value-equality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Money(Decimal);

impl Money {
    /// Wrap a `Decimal`.
    pub const fn from_decimal(value: Decimal) -> Self {
        Money(value)
    }

    /// The underlying exact decimal.
    pub fn as_decimal(self) -> Decimal {
        self.0
    }
}

impl From<Decimal> for Money {
    fn from(value: Decimal) -> Self {
        Money(value)
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for Money {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Preserve the exact decimal (including scale) as a string — never a float.
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Money {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        // Exact: error (never silently round) if the string exceeds Decimal's precision.
        let parsed = Decimal::from_str_exact(&s).map_err(serde::de::Error::custom)?;
        // Canonical: reject non-canonical spellings (scientific notation, leading '+', '-0',
        // underscores, …). Our own serialize emits the canonical form, so we always round-trip.
        if parsed.to_string() != s {
            return Err(serde::de::Error::custom(format!(
                "non-canonical decimal string: {s:?} (expected {:?})",
                parsed.to_string()
            )));
        }
        Ok(Money(parsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_json_string_not_number() {
        let m = Money::from(Decimal::new(14150, 2)); // 141.50
        let v = serde_json::to_value(m).unwrap();
        assert!(
            v.is_string(),
            "money must serialize as a JSON string, got {v:?}"
        );
        assert_eq!(v, serde_json::json!("141.50"));
    }

    #[test]
    fn round_trips_preserving_scale() {
        for raw in ["0", "141.50", "-0.07", "1322.500000", "200"] {
            let m = Money::from(Decimal::from_str_exact(raw).unwrap());
            let json = serde_json::to_string(&m).unwrap();
            let back: Money = serde_json::from_str(&json).unwrap();
            assert_eq!(m, back);
        }
    }

    #[test]
    fn rejects_non_decimal_string() {
        let r: Result<Money, _> = serde_json::from_str("\"not-a-number\"");
        assert!(r.is_err());
    }

    #[test]
    fn rejects_json_number() {
        // A bare number is not accepted — the contract is string-only.
        let r: Result<Money, _> = serde_json::from_str("141.50");
        assert!(r.is_err());
    }

    #[test]
    fn rejects_non_canonical_spellings() {
        // Scientific notation, leading '+', '-0', underscores, leading/trailing dot — all rejected.
        for bad in [
            "\"1e5\"",
            "\"1E5\"",
            "\"+1\"",
            "\"-0\"",
            "\"1_000\"",
            "\"1.\"",
            "\".5\"",
        ] {
            let r: Result<Money, _> = serde_json::from_str(bad);
            assert!(r.is_err(), "expected rejection of non-canonical {bad}");
        }
    }

    #[test]
    fn rejects_precision_loss_instead_of_silently_rounding() {
        // A scale-29 string exceeds Decimal's 28-digit precision: must ERROR, never round to 0.
        let r: Result<Money, _> = serde_json::from_str("\"0.00000000000000000000000000001\"");
        assert!(
            r.is_err(),
            "must reject (not silently round) excess-precision money"
        );
    }

    #[test]
    fn rejects_bare_number_inside_a_struct_field() {
        // The realistic case: a float where a money string is expected, nested in an object.
        #[derive(serde::Deserialize)]
        struct Holder {
            #[allow(dead_code)]
            amount: Money,
        }
        let r: Result<Holder, _> = serde_json::from_str(r#"{"amount":141.50}"#);
        assert!(
            r.is_err(),
            "money must never accept a JSON number, even nested"
        );
    }
}
