//! Replacement-candidate surfacing (Story 6.8, FR48) — pure reads, journal-wide.
//!
//! PRD Journey 4: selling is not an end — on a sell (or a sell/stop trigger) the app surfaces
//! WATCHLIST candidates nearest to or inside their §4 Buy zone, each with its upside/downside
//! and two neutral re-concentration FACTS (already-held share, currency exposure), and lets the
//! user open the candidate's study. The app states facts; it never says "buy this one" (FR13).
//!
//! Conversion happens only here and in the sibling consolidation reads (FR28 — the
//! `core::risk::fx` pin); every share is checked arithmetic over the same global total the
//! 6.6/6.7 reads use, and an ABSENT fact never flags, never passes as zero.

use rust_decimal::Decimal;
use steadyinvest_core::risk::share_pct;
use steadyinvest_core::ssg::UpsideDownside;
use uuid::Uuid;

use super::JournalState;
use crate::viewmodel::engine;

/// One held currency's share of the TOTAL invested capital (Story 6.8, AC2) — the FR48
/// currency-exposure fact. `share_pct` is absent when the currency's bucket (or the global)
/// could not convert; `missing_pair` names the pair when that is the cause.
pub struct CurrencyShare {
    pub currency: String,
    pub share_pct: Option<Decimal>,
    pub missing_pair: Option<String>,
}

/// The journal-wide per-currency exposure: one row per HELD currency (deterministic order),
/// plus whether the checked global total was formed and is positive (shares meaningful).
pub struct CurrencyExposure {
    pub rows: Vec<CurrencyShare>,
    pub global_positive: bool,
    /// The rates actually used (deduplicated per pair) — the FR28 footnote: every converted
    /// figure's rate stays inspectable (date + source) wherever the shares render.
    pub rates_used: Vec<steadyinvest_persistence::FxRateItem>,
}

impl CurrencyExposure {
    /// The exposure fact for `currency`: its held share, or an HONEST zero when the journal
    /// holds nothing in it and the total is known — `(None, None)` when the total is absent
    /// (an absent fact never flags, never passes as zero).
    pub fn share_for(&self, currency: &str) -> (Option<Decimal>, Option<String>) {
        match self.rows.iter().find(|r| r.currency == currency) {
            Some(row) => (row.share_pct, row.missing_pair.clone()),
            None => (self.global_positive.then_some(Decimal::ZERO), None),
        }
    }
}

/// Which facts a candidate could state (Story 6.8, AC1) — honest buckets, never dropped rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateData {
    /// The study resolved and the §4 band exists — zone/distance/UD facts are stated.
    Ok,
    /// The study resolved but the zone facts are unavailable: no current price, a degenerate
    /// band, a price below the band (the §4 zone is undefined there, by method), or a study
    /// that does not normalize.
    Insufficient,
    /// No saved study matches the watch item (neither its link nor the ticker).
    NoStudy,
    /// The study lookup FAILED (issue #95) — a study may well exist, so « aucune étude »
    /// would be factually wrong; the UI says « étude indisponible ».
    StudyUnavailable,
}

/// One watchlist candidate with its neutral facts (Story 6.8). Exact `Decimal`s — the wiring
/// formats for display via the locale path.
pub struct ReplacementCandidate {
    /// The ticker as watched (display); joins are done uppercased.
    pub ticker: String,
    pub study_id: Option<Uuid>,
    pub data: CandidateData,
    /// "buy" | "neutral" | "sell" | "" — the engine zone key (crossed to `Labels` nouns).
    pub zone_key: String,
    pub in_buy_zone: bool,
    /// Issue #48 (FR35): the price sits BELOW the recorded forecast band — a distinct neutral
    /// fact; mutually exclusive with a defined zone (undefined outside the band, by method).
    pub below_band: bool,
    /// `(price − buy_top) / buy_top × 100` when the price sits ABOVE the buy zone — the
    /// relative distance candidates are ranked by. Absent inside the zone or without a band.
    pub distance_above_buy_pct: Option<Decimal>,
    /// The §4 U/D ratio when it IS a ratio — `Undefined`/`Unknown` are absences, never 0.
    pub ud_ratio: Option<Decimal>,
    /// The study's native currency (uppercased); `None` without a study.
    pub currency: Option<String>,
    /// The candidate's ALREADY-HELD share of the total invested capital (Story 6.7 rows) —
    /// present only when the ticker is currently held and its share could be stated.
    pub held_share_pct: Option<Decimal>,
    /// The share of the total invested capital already denominated in the study's currency.
    pub currency_share_pct: Option<Decimal>,
    /// Names the missing pair when the currency share is absent for that reason.
    pub currency_missing_pair: Option<String>,
}

impl JournalState {
    /// The journal-wide per-currency invested exposure (Story 6.8, AC2): ALL active holdings
    /// of EVERY portfolio grouped by effective currency (the 6.2 rule, via the shared
    /// [`car_buckets_by_currency`] fold), each bucket's invested converted at the LATEST stored
    /// rate (identity for the reference — never a self-rate lookup), divided by the checked
    /// global. A missing pair absents the affected share AND the global (named), never a
    /// partial total. `None` = the view could not be built at all (no journal / a failed
    /// read — an absence, never an empty-looking zero state).
    pub fn journal_currency_exposure(&self, reference_currency: &str) -> Option<CurrencyExposure> {
        let journal = self.journal.as_ref()?;
        let portfolios = journal.list_portfolios().ok()?;
        let mut all_holdings = Vec::new();
        for portfolio in portfolios {
            match journal.list_holdings(portfolio.id) {
                Ok(mut holdings) => all_holdings.append(&mut holdings),
                Err(_) => return None,
            }
        }
        // The shared per-currency fold (6.2/6.6): (currency, car, invested), native, FX-free.
        // Each currency appears exactly ONCE here (the fold groups), so one lookup per bucket
        // is already the minimum — no memo needed (2026-07-03 review).
        let buckets = super::holdings::car_buckets_by_currency(&all_holdings, reference_currency);
        let mut rates_used: Vec<steadyinvest_persistence::FxRateItem> = Vec::new();
        let mut converted: Vec<(String, Option<Decimal>, Option<String>)> = Vec::new();
        for (currency, _car, invested) in &buckets {
            let (amount, missing) = if currency == reference_currency {
                (Some(*invested), None)
            } else {
                let row = journal
                    .latest_fx_rate(currency, reference_currency, None)
                    .ok()
                    .flatten();
                let rate = row
                    .as_ref()
                    .and_then(|r| Decimal::from_str_exact(&r.rate).ok())
                    // A nonpositive stored rate must not convert to a confident zero
                    // (6.6 review) — folded into the named refusal.
                    .filter(|r| r.is_sign_positive() && !r.is_zero());
                match rate {
                    Some(rate) => {
                        if let Some(row) = row {
                            // The FR28 footnote: every rate used, once per pair.
                            if !rates_used.iter().any(|u| {
                                u.base_currency == row.base_currency
                                    && u.quote_currency == row.quote_currency
                            }) {
                                rates_used.push(row);
                            }
                        }
                        (steadyinvest_core::risk::convert(*invested, rate), None)
                    }
                    None => (None, Some(format!("{currency} → {reference_currency}"))),
                }
            };
            converted.push((currency.clone(), amount, missing));
        }
        let global = converted
            .iter()
            .try_fold(Decimal::ZERO, |acc, (_, amount, _)| {
                acc.checked_add((*amount)?)
            });
        let rows = converted
            .into_iter()
            .map(|(currency, amount, missing_pair)| CurrencyShare {
                currency,
                share_pct: amount.zip(global).and_then(|(a, g)| share_pct(a, g)),
                missing_pair,
            })
            .collect();
        Some(CurrencyExposure {
            rows,
            global_positive: global.is_some_and(|g| g > Decimal::ZERO),
            rates_used,
        })
    }

    /// The FR48 replacement candidates (Story 6.8, AC1): one per watch item, in the PINNED
    /// order — in-buy-zone first (watchlist position tiebreak), then ascending relative
    /// distance above the buy zone, then « données insuffisantes », then « aucune étude ».
    /// One `build_snapshot` per resolved study (the `confront.rs` off-form precedent — the
    /// watchlist already pays this per refresh). A pure read; deterministic throughout.
    /// `None` = the watchlist itself could not be read (an absence, never an empty-looking
    /// « liste vide » — the 6.6/6.7 IO rule).
    pub fn replacement_candidates(
        &self,
        reference_currency: &str,
    ) -> Option<Vec<ReplacementCandidate>> {
        // The held-share facts ride the 6.7 read; the size boundaries only shape the size-mix
        // slots (discarded here — per-ticker shares are bounds-independent), so the pinned
        // defaults are passed rather than threading display config into a state read.
        let (small_max, medium_max) = (
            Decimal::from_str_exact(crate::config::DEFAULT_SIZE_SMALL_MAX).unwrap_or(Decimal::ONE),
            Decimal::from_str_exact(crate::config::DEFAULT_SIZE_MEDIUM_MAX).unwrap_or(Decimal::TWO),
        );
        let diversification =
            self.journal_diversification(reference_currency, small_max, medium_max);
        let exposure = self.journal_currency_exposure(reference_currency);

        let items = self.journal.as_ref()?.list_watch_items().ok()?;
        let mut candidates: Vec<(usize, ReplacementCandidate)> = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            // The 4.1 link rule: the explicit study link first, else the case-insensitive
            // most-recent same-ticker study. Issue #95 tri-state: a read FAILURE anywhere in
            // the chain is « étude indisponible », never « aucune étude ».
            let study: Result<Option<_>, String> = (|| {
                if let Some(sid) = item.study_id
                    && let Some(study) = self.try_get_study(sid)?
                {
                    return Ok(Some(study));
                }
                match self.try_study_id_for_ticker(&item.security_ticker)? {
                    Some(sid) => self.try_get_study(sid),
                    None => Ok(None),
                }
            })();
            let no_study_candidate = |data: CandidateData| ReplacementCandidate {
                ticker: item.security_ticker.clone(),
                study_id: None,
                data,
                zone_key: String::new(),
                in_buy_zone: false,
                below_band: false,
                distance_above_buy_pct: None,
                ud_ratio: None,
                currency: None,
                held_share_pct: None,
                currency_share_pct: None,
                currency_missing_pair: None,
            };
            let candidate = match study {
                Err(_) => no_study_candidate(CandidateData::StudyUnavailable),
                Ok(None) => no_study_candidate(CandidateData::NoStudy),
                Ok(Some(study)) => {
                    let snapshot = engine::build_snapshot(&study).ok();
                    let price = engine::money_dec(study.judgment.current_price);
                    let (zone, distance, ud) = match &snapshot {
                        Some(snapshot) => {
                            let rr = &snapshot.outputs().risk_reward;
                            // Relative distance above the buy zone — checked ops only (the
                            // render-path rule): (price − buy_top)/buy_top × 100.
                            let distance =
                                rr.zones.as_ref().zip(price).and_then(|(zones, price)| {
                                    (price > zones.buy_top && zones.buy_top > Decimal::ZERO)
                                        .then(|| {
                                            price
                                                .checked_sub(zones.buy_top)?
                                                .checked_div(zones.buy_top)?
                                                .checked_mul(Decimal::ONE_HUNDRED)
                                        })
                                        .flatten()
                                });
                            let ud = match rr.upside_downside {
                                UpsideDownside::Ratio(ratio) => Some(ratio),
                                UpsideDownside::Undefined | UpsideDownside::Unknown => None,
                            };
                            (rr.present_price_zone, distance, ud)
                        }
                        None => (None, None, None),
                    };
                    let in_buy_zone = zone == Some(steadyinvest_core::ssg::Zone::Buy);
                    // Issue #48 (FR35): a price BELOW the recorded band is a statable neutral
                    // fact of its own — the line reads « sous la bande de prévision », never
                    // « données insuffisantes ». `data` (and thus the AC1 pinned RANK) is
                    // untouched: the §4 zone facts genuinely are unavailable there by method;
                    // only the label becomes honest.
                    let below_band = snapshot.as_ref().zip(price).is_some_and(|(s, p)| {
                        s.outputs()
                            .risk_reward
                            .forecast_low
                            .is_some_and(|low| p < low)
                    });
                    // A KNOWN zone is a statable fact even when the distance is not (a
                    // degenerate/nonpositive buy_top) — never « données insuffisantes » beside
                    // a stated H/B (2026-07-03 review); the UI renders the zone noun then.
                    let data = if in_buy_zone || distance.is_some() || zone.is_some() {
                        CandidateData::Ok
                    } else {
                        CandidateData::Insufficient
                    };
                    let currency = study.native_currency.to_uppercase();
                    let held_share_pct = diversification
                        .rows
                        .iter()
                        .find(|r| r.ticker == item.security_ticker.to_uppercase())
                        .and_then(|r| r.share_pct);
                    let (currency_share_pct, currency_missing_pair) = exposure
                        .as_ref()
                        .map(|e| e.share_for(&currency))
                        .unwrap_or((None, None));
                    ReplacementCandidate {
                        ticker: item.security_ticker.clone(),
                        study_id: Some(study.id),
                        data,
                        zone_key: engine::zone_key(zone).to_string(),
                        in_buy_zone,
                        below_band,
                        distance_above_buy_pct: distance,
                        ud_ratio: ud,
                        currency: Some(currency),
                        held_share_pct,
                        currency_share_pct,
                        currency_missing_pair,
                    }
                }
            };
            candidates.push((index, candidate));
        }
        // The pinned order (AC1). Rank: 0 in-zone · 1 above-with-distance · 2 insufficient ·
        // 3 no-study (issue #95: a failed lookup shares this last rank); within a rank the
        // watchlist position (then distance for rank 1).
        let rank = |c: &ReplacementCandidate| -> u8 {
            if c.in_buy_zone {
                0
            } else if c.distance_above_buy_pct.is_some() {
                1
            } else if c.data == CandidateData::Insufficient {
                2
            } else {
                3
            }
        };
        candidates.sort_by(|(ia, a), (ib, b)| {
            rank(a).cmp(&rank(b)).then_with(|| {
                match (a.distance_above_buy_pct, b.distance_above_buy_pct) {
                    (Some(da), Some(db)) => da.cmp(&db).then(ia.cmp(ib)),
                    _ => ia.cmp(ib),
                }
            })
        });
        Some(candidates.into_iter().map(|(_, c)| c).collect())
    }
}
