//! Exact monetary / decimal value for the data contract.
//!
//! [`Money`] wraps [`rust_decimal::Decimal`] and serializes to/from a JSON **string** (never a JSON
//! number / float), so precision is preserved exactly across persistence and export. This enforces
//! the "Decimal in JSON = string" contract in one place.

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// An exact decimal money/ratio value. Equality/ordering are by numeric value (via `Decimal`).
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
        Decimal::from_str(&s)
            .map(Money)
            .map_err(serde::de::Error::custom)
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
            let m = Money::from(Decimal::from_str(raw).unwrap());
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
}
