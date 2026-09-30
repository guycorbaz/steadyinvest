//! The frozen decision-time verdict (Story 8.8, FR68, arch A13 / D11).
//!
//! When the owner validates a study on a FULL verdict, the verdict of that moment is stored in the
//! study blob — its facts, its figures, the `inputs_hash` and `method_version` of the engine's
//! `FullVerdict`, the inputs it was computed from, and when. It is never recomputed: the current
//! verdict stays live (never persisted) and the app compares the two. Built ONLY by
//! `report::form::freeze` from a `FullVerdict` (this crate has no engine dependency).
//!
//! Zones and their bounds are stored with NEUTRAL names (`low` / `middle` / `high` — the MCP
//! `computed` convention, FR13): the raw
//! blob is readable through MCP, where no buy / sell word may appear; the app writes them with its
//! runtime zone labels.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::money::Money;
use crate::provenance::Timestamp;

/// The price zone at the freeze, as a neutral code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrozenZone {
    /// The lowest third of the forecast band.
    Low,
    /// The middle third.
    Middle,
    /// The upper third.
    High,
}

/// One criterion of the verdict at the freeze.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrozenCriterion {
    Met,
    Unmet,
    /// Unmet because its inputs were insufficient (`unknown` — the MCP spelling).
    #[serde(rename = "unknown")]
    UnmetByInsufficiency,
}

/// The upside/downside ratio at the freeze.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "value")]
pub enum FrozenUpsideDownside {
    Ratio(Money),
    /// No downside: the ratio is undefined.
    Undefined,
    Unknown,
}

/// The forecast band's thirds at the freeze (neutral names — the MCP `computed.zones` spelling).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenZoneBounds {
    pub forecast_low: Money,
    pub low_zone_top: Money,
    pub middle_zone_top: Money,
    pub forecast_high: Money,
}

/// The verdict the owner validated (Story 8.8). Every figure is the engine's exact value at the
/// freeze, stored as a string decimal; `None` where the engine had none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenVerdict {
    /// When the owner validated the study (RFC 3339 UTC).
    pub frozen_at: Timestamp,
    /// The engine's method version then (e.g. `ssg-1.2.0`).
    pub method_version: String,
    /// The engine's inputs digest then (SHA-256 hex).
    pub inputs_hash: String,
    /// The verdict: a quality-value candidate or not.
    pub quality_value_candidate: bool,
    #[serde(default)]
    pub present_zone: Option<FrozenZone>,
    pub ud_at_or_above_target: FrozenCriterion,
    pub relative_value_below_ceiling: FrozenCriterion,
    pub present_price_in_low_zone: FrozenCriterion,
    pub appreciation_at_or_above_double: FrozenCriterion,
    pub upside_downside: FrozenUpsideDownside,
    #[serde(default)]
    pub relative_value_pct: Option<Money>,
    #[serde(default)]
    pub projected_appreciation_pct: Option<Money>,
    /// The « Potentiel à 5 ans » figure the study header shows.
    #[serde(default)]
    pub five_year_potential_pct: Option<Money>,
    #[serde(default)]
    pub zones: Option<FrozenZoneBounds>,
    /// Every input the `inputs_hash` covers, keyed `y{year}.{field}`, `j.{field}`, `q.{field}` →
    /// its canonical value (« absent » when none).
    #[serde(default)]
    pub inputs: BTreeMap<String, String>,
}
