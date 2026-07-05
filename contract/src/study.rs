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
    /// The reported fiscal year.
    pub year: i32,
    /// Aggregate revenue (load-bearing).
    pub sales: Cell,
    /// Earnings per share (load-bearing).
    pub eps: Cell,
    /// The year's high price (load-bearing).
    pub high_price: Cell,
    /// The year's low price (load-bearing).
    pub low_price: Cell,
    /// Dividend per share (optional).
    #[serde(default)]
    pub dividend_per_share: Option<Cell>,
    /// Pre-tax profit (optional).
    #[serde(default)]
    pub pre_tax_profit: Option<Cell>,
    /// Book value per share (optional).
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
/// "load-bearing input"). Field names mirror `core::ssg::JudgmentInputs` exactly (and, for the
/// overlap, `core::method::LOAD_BEARING_JUDGMENT_INPUTS`) so the Story-2.6 engine mapping can map
/// them straight across. The four growth/option fields below were added in Story 2.2 to close
/// issue #14 — without them an FR6 growth judgment and the §4 option (c)/(d) inputs were silently
/// lost on save/reload. They are `#[serde(default)]` optionals, so the change is additive and
/// forward- AND backward-compatible (no `SCHEMA_VERSION` bump — see the contract forward-compat
/// policy in `lib.rs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Judgment {
    /// Judged estimated high EPS over the forecast horizon (method spec §4).
    #[serde(default)]
    pub estimated_high_eps: Option<Money>,
    /// Judged estimated low EPS over the forecast horizon (method spec §4).
    #[serde(default)]
    pub estimated_low_eps: Option<Money>,
    /// Judged future sales growth, percent per year (FR6). Stored as the percent value itself
    /// (e.g. `"12.5"`). Added in Story 2.2 (issue #14).
    #[serde(default)]
    pub projected_sales_growth_pct: Option<Money>,
    /// Judged future EPS growth, percent per year (FR6). Added in Story 2.2 (issue #14).
    #[serde(default)]
    pub projected_eps_growth_pct: Option<Money>,
    /// Judged future average high P/E (method spec §4).
    #[serde(default)]
    pub judged_avg_high_pe: Option<Money>,
    /// Judged future average low P/E (method spec §4).
    #[serde(default)]
    pub judged_avg_low_pe: Option<Money>,
    /// Which §4 forecast-low option the user selected.
    pub forecast_low_option: ForecastLowOption,
    /// §4 forecast-low option (c) input: a recent severe market low. Added in Story 2.2 (issue #14).
    #[serde(default)]
    pub recent_severe_low: Option<Money>,
    /// The security's current price — the zone/verdict anchor.
    #[serde(default)]
    pub current_price: Option<Money>,
    /// Trailing-twelve-months EPS (Issue #113) — the current-P/E denominator (spec §3/§9:
    /// `current_price / TTM EPS`). A **current market fact** (like `current_price`), populated by a
    /// provider fetch; `None` when unknown → current P/E stays honestly unknown. Additive
    /// `#[serde(default)]` optional — no `SCHEMA_VERSION` bump (contract forward-compat policy).
    #[serde(default)]
    pub ttm_eps: Option<Money>,
    /// §4 option (d) numerator + §5 present-yield input: the present full-year dividend per share.
    /// Added in Story 2.2 (issue #14).
    #[serde(default)]
    pub present_full_year_dividend: Option<Money>,
}

/// A durable stock study (one row of the journal). Carries the `schema_version` it was written under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Study {
    /// The study's stable identity (preserved across export/import).
    pub id: Uuid,
    /// The journal this study belongs to (rebound on import).
    pub journal_id: Uuid,
    /// The studied security's ticker symbol.
    pub security_ticker: String,
    /// ISO-4217-style native currency of the security (calculations run in this currency).
    pub native_currency: String,
    /// The historical per-year input cells.
    #[serde(default)]
    pub years: Vec<YearData>,
    /// The user's judgment snapshot.
    pub judgment: Judgment,
    /// First-class decision rationale (FR49) — the "why".
    #[serde(default)]
    pub rationale: Option<String>,
    /// When the study was created (RFC3339 UTC).
    pub created_at: Timestamp,
    /// The [`SCHEMA_VERSION`] the study was written under.
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
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: None,
            forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
            recent_severe_low: None,
            current_price: None,
            present_full_year_dividend: None,
            ttm_eps: None,
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
