//! The per-cell data-state model (FR17–FR20): every data point carries, independently queryable,
//! its **source** × **freshness** × **review** × **coverage**, plus its [`Provenance`].
//!
//! A missing value is `value: None` — a genuine gap, **never coerced to 0** (`unknown/insufficient`
//! is first-class).

use crate::money::Money;
use crate::provenance::Provenance;
use serde::{Deserialize, Serialize};

/// Where a cell's value came from (FR17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Provider,
    Manual,
    Derived,
}

/// Freshness of a cell's value (FR23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Current,
    Stale,
}

/// User-set review tag (FR20) — tri-state, NEVER `0/1/2`. `none` → `to_review` (?) → `validated` (✓).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    None,
    ToReview,
    Validated,
}

/// Per-cell coverage state (FR19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Present,
    ToFill,
    NotAvailableAccepted,
}

/// One data cell: an optional exact value plus its full, independently-queryable data state and
/// provenance. Forward-compatible: `value` defaults to `None` if absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    #[serde(default)]
    pub value: Option<Money>,
    pub source: Source,
    pub freshness: Freshness,
    pub review: Review,
    pub coverage: Coverage,
    pub provenance: Provenance,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::{Provenance, Timestamp};
    use rust_decimal::Decimal;

    fn sample_provenance() -> Provenance {
        Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: Timestamp("2026-06-09T00:00:00Z".to_string()),
            hash_of_dependencies: "deadbeef".to_string(),
        }
    }

    #[test]
    fn enum_wire_formats_are_snake_case() {
        assert_eq!(
            serde_json::to_string(&Source::Provider).unwrap(),
            "\"provider\""
        );
        assert_eq!(
            serde_json::to_string(&Freshness::Stale).unwrap(),
            "\"stale\""
        );
        assert_eq!(
            serde_json::to_string(&Review::ToReview).unwrap(),
            "\"to_review\""
        );
        assert_eq!(
            serde_json::to_string(&Coverage::NotAvailableAccepted).unwrap(),
            "\"not_available_accepted\""
        );
    }

    #[test]
    fn cell_value_defaults_to_none_when_absent() {
        // Forward-compat: a cell JSON missing "value" deserializes to None (serde default).
        let json = r#"{"source":"manual","freshness":"current","review":"none",
            "coverage":"to_fill","provenance":{"source":"manual","logical_version":1,
            "timestamp":"2026-06-09T00:00:00Z","hash_of_dependencies":"x"}}"#;
        let cell: Cell = serde_json::from_str(json).unwrap();
        assert_eq!(cell.value, None);
    }

    #[test]
    fn unknown_extra_field_is_tolerated() {
        // Forward-compat: NO deny_unknown_fields — an older build tolerates a newer file's field.
        let json = r#"{"value":"12.34","source":"provider","freshness":"current",
            "review":"validated","coverage":"present","provenance":{"source":"provider",
            "logical_version":2,"timestamp":"2026-06-09T00:00:00Z","hash_of_dependencies":"h"},
            "future_field_from_a_newer_build":42}"#;
        let cell: Cell = serde_json::from_str(json).unwrap();
        assert_eq!(cell.value, Some(Money::from(Decimal::new(1234, 2))));
        assert_eq!(cell.review, Review::Validated);
    }

    #[test]
    fn cell_round_trips() {
        let cell = Cell {
            value: Some(Money::from(Decimal::new(-7, 2))), // -0.07
            source: Source::Derived,
            freshness: Freshness::Stale,
            review: Review::ToReview,
            coverage: Coverage::Present,
            provenance: sample_provenance(),
        };
        let json = serde_json::to_string(&cell).unwrap();
        assert_eq!(serde_json::from_str::<Cell>(&json).unwrap(), cell);
    }
}
