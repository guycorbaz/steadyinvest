//! `Study` and `Judgment` — the durable journal types. A `Study` holds the per-year input cells, the
//! user's judgment snapshot, an optional decision rationale, and the `schema_version` it was written
//! under. Field names align with `core::method`'s load-bearing keys so the engine (Story 1.8) can map
//! them directly. New/optional fields use `#[serde(default)]` for forward-compatibility.

use crate::cell::Cell;
use crate::money::Money;
use crate::provenance::Timestamp;
use crate::versioning::SCHEMA_VERSION;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One historical year of inputs. The four load-bearing fields (`sales`, `eps`, `high_price`,
/// `low_price`) make the year "usable" (method spec §4/§5); the rest are optional.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YearData {
    pub year: i32,
    pub sales: Cell,
    pub eps: Cell,
    pub high_price: Cell,
    pub low_price: Cell,
    #[serde(default)]
    pub dividend_per_share: Option<Cell>,
    #[serde(default)]
    pub pre_tax_profit: Option<Cell>,
    #[serde(default)]
    pub book_value_per_share: Option<Cell>,
}

/// Which method is used to choose the forecast low price (method spec §4, options a–d).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForecastLowOption {
    /// (a) average low P/E × estimated low EPS.
    AvgLowPeTimesEps,
    /// (b) average low price of the last 5 years.
    AvgLowPriceLast5y,
    /// (c) a recent severe market low.
    RecentSevereLow,
    /// (d) price the dividend will support.
    DividendSupported,
}

/// The user's judgment snapshot — exactly the inputs that gate the verdict (method spec §5
/// "load-bearing input"). Names mirror `core::method::LOAD_BEARING_JUDGMENT_INPUTS`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Judgment {
    #[serde(default)]
    pub estimated_high_eps: Option<Money>,
    #[serde(default)]
    pub estimated_low_eps: Option<Money>,
    #[serde(default)]
    pub judged_avg_high_pe: Option<Money>,
    #[serde(default)]
    pub judged_avg_low_pe: Option<Money>,
    pub forecast_low_option: ForecastLowOption,
    #[serde(default)]
    pub current_price: Option<Money>,
}

/// A durable stock study (one row of the journal). Carries the `schema_version` it was written under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Study {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub security_ticker: String,
    /// ISO-4217-style native currency of the security (calculations run in this currency).
    pub native_currency: String,
    #[serde(default)]
    pub years: Vec<YearData>,
    pub judgment: Judgment,
    /// First-class decision rationale (FR49) — the "why".
    #[serde(default)]
    pub rationale: Option<String>,
    pub created_at: Timestamp,
    pub schema_version: u32,
}

impl Study {
    /// Create a new study stamped with the current [`SCHEMA_VERSION`].
    pub fn new(
        id: Uuid,
        journal_id: Uuid,
        security_ticker: impl Into<String>,
        native_currency: impl Into<String>,
        judgment: Judgment,
        created_at: Timestamp,
    ) -> Self {
        Study {
            id,
            journal_id,
            security_ticker: security_ticker.into(),
            native_currency: native_currency.into(),
            years: Vec::new(),
            judgment,
            rationale: None,
            created_at,
            schema_version: SCHEMA_VERSION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_judgment() -> Judgment {
        Judgment {
            estimated_high_eps: None,
            estimated_low_eps: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: None,
            forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
            current_price: None,
        }
    }

    #[test]
    fn new_study_is_stamped_with_schema_version() {
        let s = Study::new(
            Uuid::nil(),
            Uuid::nil(),
            "NESN",
            "CHF",
            empty_judgment(),
            Timestamp("2026-06-09T00:00:00Z".to_string()),
        );
        assert_eq!(s.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn study_round_trips_and_tolerates_unknown_fields() {
        let s = Study::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "AAPL",
            "USD",
            empty_judgment(),
            Timestamp("2026-06-09T00:00:00Z".to_string()),
        );
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Study>(&json).unwrap(), s);

        // Forward-compat: a newer build's extra top-level field is ignored.
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v.as_object_mut()
            .unwrap()
            .insert("future".to_string(), serde_json::json!("x"));
        assert_eq!(
            serde_json::from_value::<Study>(v).unwrap(),
            s,
            "unknown extra field must be tolerated (no deny_unknown_fields)"
        );
    }
}
