//! Reopen & confront (Story 5.1 — FR50/ADD13): the strictly read-only [`ConfrontView`] — the
//! study's §4 projection band as DECIDED (the frozen verdict's band, Story 8.8 — project review
//! 2026-10-01: a re-derivation from today's study drifts with every edit, annual update or method
//! change, so it is only the labelled fallback for a study never validated) overlaid on the
//! security's **actual** close trajectory
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
    /// Issue #95: true when the study READ failed — the overlay says « étude indisponible »,
    /// never the factually wrong « pas encore de cours enregistrés » empty state.
    pub unavailable: bool,
    /// The decision date the band is anchored at (`YYYY-MM-DD`): the frozen verdict's `frozen_at`,
    /// else `study.created_at`. A re-validation moves it (the last decision).
    pub decision_date: String,
    /// The recorded §4 forecast band bounds (read-only from the stored judgment — no verdict recompute).
    pub forecast_high: Option<Decimal>,
    pub forecast_low: Option<Decimal>,
    /// The projection horizon (`core::method::FORECAST_HORIZON_YEARS`).
    pub horizon_years: u32,
    /// FR50 (project review 2026-10-01): where the band comes from (said on the overlay — a
    /// re-derived band is never passed off as the one decided).
    pub basis: ConfrontBasis,
    /// The actual close trajectory since the decision, oldest-first: `(date, close)`.
    pub actual: Vec<(String, Decimal)>,
}

/// The source of the confrontation's band (FR50, project review + G3 review 2026-10-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfrontBasis {
    /// The frozen verdict's band — the projection decided, dated the validation day.
    Decided,
    /// A frozen verdict whose band was degenerate (none recorded): re-derived from today's study,
    /// still dated the validation day.
    FrozenWithoutBand,
    /// No frozen verdict: re-derived from today's study, dated its creation.
    #[default]
    Current,
}

impl ConfrontBasis {
    /// The key crossed to Slint (`decided` / `frozen-no-band` / `current`).
    pub fn key(self) -> &'static str {
        match self {
            ConfrontBasis::Decided => "decided",
            ConfrontBasis::FrozenWithoutBand => "frozen-no-band",
            ConfrontBasis::Current => "current",
        }
    }
}

impl JournalState {
    /// Build the read-only **confront** view for a saved study (Story 5.1, FR50/ADD13): its
    /// projection band overlaid on the security's actual close trajectory since the decision.
    /// Strictly read-only — reads the stored `Study` + the price-history cache and renders; it
    /// writes nothing and bumps no `logical_version`. The band is the frozen verdict's (the
    /// projection decided, [`ConfrontBasis::Decided`]); without a recorded band it is re-derived
    /// from today's study and said so ([`ConfrontBasis::FrozenWithoutBand`] /
    /// [`ConfrontBasis::Current`]). `available` is false (neutral empty state) when there is no
    /// cached post-decision close or no band.
    pub fn confront(&self, study_id: Uuid) -> ConfrontView {
        let empty = |unavailable: bool, decision_date: String| ConfrontView {
            available: false,
            unavailable,
            decision_date,
            forecast_high: None,
            forecast_low: None,
            horizon_years: steadyinvest_core::method::FORECAST_HORIZON_YEARS,
            actual: Vec::new(),
            basis: ConfrontBasis::Current,
        };
        // Issue #95 tri-state: a read FAILURE is « étude indisponible », never the « pas encore
        // de cours enregistrés » empty state a true absence would show.
        let study = match self.try_get_study(study_id) {
            Ok(Some(study)) => study,
            Ok(None) => return empty(false, String::new()),
            Err(_) => return empty(true, String::new()),
        };
        // FR50 (project review 2026-10-01): the projection DECIDED — the frozen verdict's band, dated
        // the day the owner validated the study. A re-derivation from today's study drifts with every
        // annual update, judgment move or method change (ssg-1.2.0 #252), so it is only the fallback
        // for a study never validated — and the overlay says so.
        let re_derived = || {
            engine::build_snapshot(&study)
                .ok()
                .map(|s| {
                    let rr = &s.outputs().risk_reward;
                    (rr.forecast_high, rr.forecast_low)
                })
                .unwrap_or((None, None))
        };
        // The decision's day in the owner's zone (owner decision, Guy 2026-10-03: every shown date
        // is local) — shown « Verdict figé le … » and the first session the trajectory reads.
        let zone = self.day_zone();
        let day = |t: &steadyinvest_contract::Timestamp| {
            zone.shown(t, steadyinvest_report::price_age::Day::iso)
        };
        let (decision_date, forecast_high, forecast_low, basis) = match &study.frozen_verdict {
            Some(f) => match &f.zones {
                Some(z) => (
                    day(&f.frozen_at),
                    Some(z.forecast_high.as_decimal()),
                    Some(z.forecast_low.as_decimal()),
                    ConfrontBasis::Decided,
                ),
                // A Full verdict over a degenerate band records no band (G3 review): today's
                // band, still dated — and named — from the validation.
                None => {
                    let (high, low) = re_derived();
                    (
                        day(&f.frozen_at),
                        high,
                        low,
                        ConfrontBasis::FrozenWithoutBand,
                    )
                }
            },
            None => {
                let (high, low) = re_derived();
                (day(&study.created_at), high, low, ConfrontBasis::Current)
            }
        };

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
            unavailable: false,
            decision_date,
            forecast_high,
            forecast_low,
            horizon_years: steadyinvest_core::method::FORECAST_HORIZON_YEARS,
            actual,
            basis,
        }
    }

    /// Append a close to the price-history cache (Story 5.1): `(ticker, session_date, price)` from a
    /// refresh. One point per ticker/session (dedup by the unique index); the close is the canonical
    /// decimal TEXT. A no-op when no journal is open. `source = "provider"` (v1; the per-provider tag
    /// is a later refinement). The confront overlay reads this back via `closes_since`.
    ///
    /// Issue #72: the close is keyed by the provider's **real EOD session date** (`session_date`) when
    /// it supplies one (EODHD `/eod` dates its bars); when the provider omits it (Twelve Data's bare
    /// `/price`) — or hands back a malformed date — it falls back to the clock day. Keying by the
    /// session date stops a weekend/holiday refresh from filing the prior session's close under today,
    /// and keeps a re-fetch of the same session idempotent. Same-day policy stays **first-wins**
    /// (`INSERT OR IGNORE`): once keyed by the true session date, a repeat fetch of a finalized EOD
    /// close is a no-op, so an intraday last-write-wins correction is a deliberate non-goal here.
    pub(crate) fn cache_close(&mut self, ticker: &str, price: Decimal, session_date: Option<&str>) {
        let now = self.now();
        let clock_day: String = now.0.chars().take(10).collect(); // YYYY-MM-DD prefix
        // Accept the provider's session date only when it is a well-formed ISO day; otherwise fall
        // back to the clock day so a malformed provider string never becomes a nonsense cache key.
        let date: &str = session_date
            .map(str::trim)
            .filter(|d| is_iso_date(d))
            .unwrap_or(&clock_day);
        let close = price.normalize().to_string();
        if let Some(journal) = self.journal.as_mut() {
            let _ = journal.upsert_closes(ticker, &[(date, &close, "provider")], &now);
        }
    }
}

/// Whether `s` is a well-formed `YYYY-MM-DD` calendar-shaped day (issue #72): the confront cache keys
/// off it (a lexical compare in `closes_since`), so a provider date that is not this exact shape must
/// be rejected in favour of the clock day. Shape-only (digits + dashes at the right offsets) — the
/// providers return real calendar days; this just guards against a malformed string, not leap years.
fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..].iter().all(u8::is_ascii_digit)
}
