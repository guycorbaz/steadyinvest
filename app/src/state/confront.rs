//! Reopen & confront (Story 5.1 — FR50/ADD13): the strictly read-only [`ConfrontView`] — the
//! study's recorded §4 projection band (re-derived deterministically from the **frozen** stored
//! judgment by the pure SSG engine; the study persists inputs only, and `build_snapshot` reproduces
//! the decision-time bounds bit-for-bit) overlaid on the security's **actual** close trajectory
//! from the price-history cache — plus the cache writer the price refreshes hook into. Confront
//! writes nothing and bumps no `logical_version`; the cache is local, reconstructible, and excluded
//! from the export snapshot.

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::viewmodel::engine;

use super::JournalState;

/// The read-only confront view (Story 5.1, FR50/ADD13): the study's **recorded projection band**
/// (forecast high/low over the horizon, anchored at the decision) + the security's **actual** close
/// trajectory since the decision, for the overlay. `available` is false (neutral empty state) when
/// there is no cached post-decision close or the study has no forecast band.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfrontView {
    pub available: bool,
    /// The decision date (`study.created_at`, `YYYY-MM-DD`) the band is anchored at.
    pub decision_date: String,
    /// The recorded §4 forecast band bounds (read-only from the stored judgment — no verdict recompute).
    pub forecast_high: Option<Decimal>,
    pub forecast_low: Option<Decimal>,
    /// The projection horizon (`core::method::FORECAST_HORIZON_YEARS`).
    pub horizon_years: u32,
    /// The actual close trajectory since the decision, oldest-first: `(date, close)`.
    pub actual: Vec<(String, Decimal)>,
}

impl JournalState {
    /// Build the read-only **confront** view for a saved study (Story 5.1, FR50/ADD13): its recorded
    /// projection band overlaid on the security's actual close trajectory since the decision. Strictly
    /// read-only — reads the stored `Study` + the price-history cache and renders; it writes nothing
    /// and bumps no `logical_version`. The §4 forecast band is a **deterministic rebuild from the
    /// frozen stored judgment** (the SSG engine is pure — `build_snapshot` reproduces the decision-time
    /// bounds bit-for-bit, and `forecast_high/low` are invariant to `current_price`); the study persists
    /// only judgment inputs, so re-deriving is the faithful — and only — way to recover the recorded
    /// band, not a re-decision. `available` is false (neutral empty state) when there is no cached
    /// post-decision close or the study has no forecast band.
    pub fn confront(&self, study_id: Uuid) -> ConfrontView {
        let empty = |decision_date: String| ConfrontView {
            available: false,
            decision_date,
            forecast_high: None,
            forecast_low: None,
            horizon_years: steadyinvest_core::method::FORECAST_HORIZON_YEARS,
            actual: Vec::new(),
        };
        let Some(study) = self.get_study(study_id) else {
            return empty(String::new());
        };
        let decision_date: String = study.created_at.0.chars().take(10).collect();

        // Recorded projection band — read-only snapshot from the stored judgment (no recompute of the
        // verdict, no mutation): the §4 forecast bounds the study implied at the decision.
        let (forecast_high, forecast_low) = engine::build_snapshot(&study)
            .ok()
            .map(|s| {
                let rr = &s.outputs().risk_reward;
                (rr.forecast_high, rr.forecast_low)
            })
            .unwrap_or((None, None));

        // Actual trajectory since the decision, oldest-first, from the price-history cache.
        let actual: Vec<(String, Decimal)> = self
            .journal
            .as_ref()
            .and_then(|j| j.closes_since(&study.security_ticker, &decision_date).ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(date, close)| Decimal::from_str_exact(&close).ok().map(|d| (date, d)))
            .collect();

        let available = !actual.is_empty() && forecast_high.is_some() && forecast_low.is_some();
        ConfrontView {
            available,
            decision_date,
            forecast_high,
            forecast_low,
            horizon_years: steadyinvest_core::method::FORECAST_HORIZON_YEARS,
            actual,
        }
    }

    /// Append a close to the price-history cache (Story 5.1): `(ticker, today, price)` from a refresh.
    /// One point per ticker/day (dedup by the unique index); the close is the canonical decimal TEXT.
    /// A no-op when no journal is open. `source = "provider"` (v1; the per-provider tag is a later
    /// refinement). The confront overlay reads this back via `closes_since`.
    ///
    /// KNOWN LIMITATION (tracked as issue #72): the close is keyed by the **refresh date**, not
    /// the provider's real EOD session date — `fetch_latest_price` returns only the price (Decimal), and
    /// carrying the session date would expand the `MarketDataProvider` surface, which Story 5.1's AC4
    /// forbids ("no new provider surface"). So a weekend/holiday refresh stamps the prior session's
    /// close under today, and a same-day re-fetch is first-wins (`INSERT OR IGNORE`). The trajectory's
    /// x-axis is therefore ordinal ("nth recorded close"), good enough for the MVP confront overlay.
    pub(crate) fn cache_close(&mut self, ticker: &str, price: Decimal) {
        let now = self.now();
        let date: String = now.0.chars().take(10).collect(); // YYYY-MM-DD prefix
        let close = price.normalize().to_string();
        if let Some(journal) = self.journal.as_mut() {
            let _ = journal.upsert_closes(ticker, &[(&date, &close, "provider")], &now);
        }
    }
}
