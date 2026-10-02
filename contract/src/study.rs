//! `Study` and `Judgment` — the durable journal types. A `Study` holds the per-year input cells, the
//! user's judgment snapshot, an optional decision rationale, and the `schema_version` it was written
//! under. Field names align with `core::method`'s load-bearing keys so the engine (Story 1.8) can map
//! them directly. New/optional fields use `#[serde(default)]` for forward-compatibility.

use crate::ai::AiOrigin;
use crate::cell::{Cell, Freshness, Source};
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
    /// Which judgment fields were placed by an owner-validated AI draft (Story 8.2b, arch A6):
    /// one [`AiOrigin`] slot per draftable judgment field. **Any** write to a field clears its
    /// slot. Additive and skipped when empty, so a judgment without AI marks serializes
    /// byte-identically to before.
    #[serde(default, skip_serializing_if = "AiPlaced::is_empty")]
    pub ai_placed: AiPlaced,
    /// Where `current_price` came from and when (Guy's on-screen test 2026-10-01, FR11/FR12): the
    /// provider (a fetch or a holdings price refresh) or the owner's typing, the moment it was
    /// written, the provider's trading-session date when known, and its freshness — a failed
    /// refresh flags it stale like the provider cells (FR23). `None` = unknown origin (a price
    /// written before this field existed). Additive and skipped when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_price_origin: Option<PriceOrigin>,
}

/// The origin of the current price (see [`Judgment::current_price_origin`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceOrigin {
    /// `Provider` (fetched) or `Manual` (typed by the owner).
    pub source: Source,
    /// When the price was written (RFC 3339 UTC, the app clock).
    pub at: Timestamp,
    /// The provider's trading-session date of the close (`YYYY-MM-DD`), when it supplied one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_date: Option<String>,
    /// `Stale` after a failed refresh (FR23); a successful refresh or a typed price is `Current`.
    pub freshness: Freshness,
    /// The FR23 **age horizon**, applied at read time (owner decision 2026-10-01): `true` when the
    /// price is older than the configured number of trading days on the day it is read
    /// (`report::price_age::apply_price_age`). **Never serialized** — the age depends on today and
    /// on a setting the owner can change, so it is recomputed on every read and can never be
    /// persisted into the dossier, an export or a backup (the wire format is unchanged).
    #[serde(skip)]
    pub aged: bool,
}

impl PriceOrigin {
    /// Stale for the verdict and the traceability (FR23): flagged by a failed refresh, or older
    /// than the age horizon on the read that produced this copy.
    pub fn is_stale(&self) -> bool {
        self.freshness == Freshness::Stale || self.aged
    }
}

/// The "placed by AI" marks of a [`Judgment`] — exactly the nine draftable judgment fields
/// (`current_price` and `ttm_eps` are provider market facts, not draftable — owner decision D6).
/// Each slot is `Some` from the owner's validation of an AI draft until the next write to that field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPlaced {
    /// Mark on [`Judgment::estimated_high_eps`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_high_eps: Option<AiOrigin>,
    /// Mark on [`Judgment::estimated_low_eps`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_low_eps: Option<AiOrigin>,
    /// Mark on [`Judgment::projected_sales_growth_pct`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projected_sales_growth_pct: Option<AiOrigin>,
    /// Mark on [`Judgment::projected_eps_growth_pct`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projected_eps_growth_pct: Option<AiOrigin>,
    /// Mark on [`Judgment::judged_avg_high_pe`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judged_avg_high_pe: Option<AiOrigin>,
    /// Mark on [`Judgment::judged_avg_low_pe`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judged_avg_low_pe: Option<AiOrigin>,
    /// Mark on [`Judgment::forecast_low_option`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forecast_low_option: Option<AiOrigin>,
    /// Mark on [`Judgment::recent_severe_low`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recent_severe_low: Option<AiOrigin>,
    /// Mark on [`Judgment::present_full_year_dividend`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub present_full_year_dividend: Option<AiOrigin>,
}

impl AiPlaced {
    /// `true` when no field carries a mark (the serialized form then omits the whole object).
    pub fn is_empty(&self) -> bool {
        *self == AiPlaced::default()
    }
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
    /// User-entered company name shown on the study header card (2026-07-12). Free text, optional;
    /// `#[serde(default)]` keeps studies written before this field loadable (they read `None`). A
    /// future fetch may pre-fill it, but it is user-editable by design (no fetch dependency).
    #[serde(default)]
    pub company_name: Option<String>,
    /// The owner's dated notes on the study (Story 8.1, FR78), in insertion order. An additive
    /// `#[serde(default)]` field like `company_name` — a study written before it reads an empty list;
    /// no `SCHEMA_VERSION` bump (owner, 2026-09-27: the app is not in production).
    #[serde(default)]
    pub notes: Vec<Note>,
    /// The verdict frozen when the owner last validated the study (Story 8.8, FR68, A13) — `None`
    /// until then. Additive and skipped when absent, so a study never validated serializes exactly
    /// as before (pins and legacy blobs byte-identical).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_verdict: Option<crate::frozen::FrozenVerdict>,
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
            company_name: None,
            notes: Vec::new(),
            frozen_verdict: None,
            created_at,
            schema_version: SCHEMA_VERSION,
        }
    }
}

/// One dated note of the owner on a study (Story 8.1, FR78). Plain owner text — never
/// posture-scanned (FR13 covers app-generated signals only). A deleted note leaves the study but
/// stays readable in the study history (FR51, owner decision O6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    /// The note's stable identity (history diffs key on it, never on position).
    pub id: Uuid,
    /// The note text, trimmed; never empty.
    pub text: String,
    /// When the note was added (RFC3339 UTC).
    pub created_at: Timestamp,
    /// When the note text last changed (equals `created_at` until the first edit).
    pub updated_at: Timestamp,
    /// The AI origin of a note validated from an AI draft (Epic 8, Story 8.5b). Always `None` in
    /// Story 8.1.
    #[serde(default)]
    pub ai_origin: Option<AiOrigin>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_judgment() -> Judgment {
        Judgment {
            ai_placed: Default::default(),
            current_price_origin: None,
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
    fn study_with_notes_round_trips() {
        let mut s = Study::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "AAPL",
            "USD",
            empty_judgment(),
            Timestamp("2026-06-09T00:00:00Z".to_string()),
        );
        s.notes.push(Note {
            id: Uuid::from_u128(3),
            text: "Marge en hausse".to_string(),
            created_at: Timestamp("2026-09-27T08:00:00Z".to_string()),
            updated_at: Timestamp("2026-09-27T09:00:00Z".to_string()),
            ai_origin: None,
        });
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Study>(&json).unwrap(), s);
    }

    #[test]
    fn a_study_written_before_notes_reads_an_empty_list() {
        let s = Study::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "AAPL",
            "USD",
            empty_judgment(),
            Timestamp("2026-06-09T00:00:00Z".to_string()),
        );
        let mut v = serde_json::to_value(&s).unwrap();
        v.as_object_mut().unwrap().remove("notes");
        assert_eq!(
            serde_json::from_value::<Study>(v).unwrap().notes,
            Vec::new()
        );
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
