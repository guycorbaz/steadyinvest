//! Story 7.2 — the portfolio health review: ONE composed, read-only view of the whole dossier
//! (every bank, every currency) — the diversification reads of Epic 6 (size mix, sector and
//! currency exposure, per-bank consolidation, per-security concentration), one engine snapshot per
//! linked study (verdict state, zone, U/D, relative value, quality flags), the FR51 history for the
//! annual-review due date, and counts. Facts only (FR13): nothing is ranked, scored or
//! recommended, and nothing is computed that the existing reads / the engine do not already
//! compute. Absence honesty (#95): a read failure is `Err` (« indisponible »), never an empty
//! section; a missing FX pair is named on the figure it blocks.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use steadyinvest_contract::Study;
use steadyinvest_core::ssg::{QualityFlagKey, UpsideDownside};
use steadyinvest_persistence::FxRateItem;
use uuid::Uuid;

use crate::viewmodel::engine;

use super::fx::JournalConsolidation;
use super::{
    CurrencyExposure, JournalDiversification, JournalState, MSG_NO_JOURNAL, SectorExposure,
};

/// The annual-review cadence (Story 3.6): a linked study whose last effective save is older than
/// this many months is « à revoir ». Fixed in this story (spec Q2).
pub const REVIEW_CADENCE_MONTHS: u32 = 12;

/// What the review knows about a held ticker's study.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewStudy {
    /// No study matches the ticker in the position's currency; `other_currency` names a
    /// same-ticker study in another currency when one exists (the CHF-vs-USD trap).
    None { other_currency: Option<String> },
    /// The study read FAILED (#95) — « indisponible », never « aucune étude ».
    Unavailable,
    /// The study was READ, but its data does not normalize (a structural input error — the
    /// engine suspends its computation, `MSG_NORMALIZE_FAILED`): a different fact from a failed
    /// read (G1 review). The study is still a study — counted, listed « à revoir », openable —
    /// and its present price still reads the stop (the register does).
    NotComputable(NotComputableFacts),
    /// A linked study and its snapshot facts.
    Linked(ReviewStudyFacts),
}

/// What is known of a study the engine cannot compute: its identity, its entered present price
/// and its annual-review clock (none of these need the engine).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotComputableFacts {
    pub study_id: Uuid,
    pub current_price: Option<Decimal>,
    /// See [`ReviewStudyFacts::last_saved`].
    pub last_saved: Option<String>,
    pub due_for_review: bool,
}

/// The engine-snapshot facts of one linked study, as the engine states them (no formatting here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewStudyFacts {
    pub study_id: Uuid,
    pub company_name: Option<String>,
    /// The study's own currency — the unit of `current_price` (never the position's: a legacy
    /// lot links ticker-only and may carry no currency of its own — G1 final review).
    pub currency: String,
    /// `"full"` | `"provisional"` | `"withheld"` (the verdict integrity state, Story 2.7).
    pub verdict: &'static str,
    pub low_confidence: bool,
    /// `"buy"` | `"neutral"` | `"sell"` inside the band; `"below"` / `"above"` outside it
    /// (2026-07-12 honest states); `""` without a band or a price.
    pub zone: &'static str,
    pub current_price: Option<Decimal>,
    pub upside_downside: UpsideDownside,
    pub relative_value_pct: Option<Decimal>,
    pub quality_flags: Vec<QualityFlagKey>,
    /// The `YYYY-MM-DD` of the last effective save (the latest FR51 snapshot, else the study's
    /// creation) — the annual-review clock. `None` when the FR51 history read FAILED: the date
    /// is unknown, never replaced by the creation date (G1 review — a false « plus de 12 mois »).
    pub last_saved: Option<String>,
    /// `last_saved` is known and older than [`REVIEW_CADENCE_MONTHS`].
    pub due_for_review: bool,
}

/// One held lot's trailing stop, read against ITS OWN matched study (the register's rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopFact {
    pub level: Decimal,
    /// The lot's DECLARED currency — the stop's unit. `None` for a legacy lot that carries no
    /// currency: the stop's unit cannot be established, so it is never labelled with one (not
    /// even the reference currency) and never compared with a price (G1 final review).
    pub currency: Option<String>,
    /// The bank (portfolio name) holding the lot.
    pub bank: String,
    /// The lot's study price reached the level. `false` when the price is unknown, is in
    /// another currency than the stop, or the stop's currency is unknown (never a comparison
    /// across currencies — nor across an unknown one).
    pub breached: bool,
}

/// One lot's trigger, keyed by the HOLDING's identity — so a trigger the user dismissed on the
/// register stays dismissed here (G1 final review).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LotTrigger {
    pub holding_id: Uuid,
    /// `"stop"` | `"sell"`.
    pub kind: &'static str,
}

/// The neutral trigger across a position's lots — the stop takes priority, as per lot in the
/// register — skipping every lot whose trigger is dismissed. `""` when none remains.
pub fn position_trigger(lots: &[LotTrigger], dismissed: impl Fn(Uuid) -> bool) -> &'static str {
    let shown = || lots.iter().filter(|t| !dismissed(t.holding_id));
    if shown().any(|t| t.kind == "stop") {
        "stop"
    } else if shown().any(|t| t.kind == "sell") {
        "sell"
    } else {
        ""
    }
}

/// One held ticker, aggregated across banks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewPosition {
    pub ticker: String,
    /// The banks (portfolio names) holding it, in portfolio order.
    pub banks: Vec<String>,
    /// The position's currency: the effective one of the lot whose study the row shows (see
    /// `study` — by identity, never the first lot's).
    pub currency: String,
    /// Invested in the reference currency (the 6.7 concentration row) — `None` when a pair is
    /// missing (named in `missing_pairs`) or the total could not form.
    pub invested: Option<Decimal>,
    pub share_pct: Option<Decimal>,
    pub missing_pairs: Vec<String>,
    /// The study whose facts the row shows, chosen by IDENTITY (the discriminator rule, G1
    /// final review — never the first lot's): see [`row_link`].
    pub study: ReviewStudy,
    /// When the lots do NOT all link to the same study (a legacy lot matched ticker-only beside
    /// a declared one, lots in two currencies): each distinct link's study currency, in lot
    /// order (`"—"` for a lot without a readable study). Empty when every lot shares the study.
    pub mixed_links: Vec<String>,
    /// Every lot's stop that carries one, in lot order (G1 review: never the first lot alone).
    pub stops: Vec<StopFact>,
    /// Any lot's stop is breached.
    pub stop_breached: bool,
    /// `"stop"` | `"sell"` | `""` — the neutral trigger (core::risk) across the lots, as the
    /// register states it per lot (the stop takes priority), dismissed or not.
    pub trigger: &'static str,
    /// Each lot's trigger with its holding's identity — a surface drops the dismissed ones
    /// ([`position_trigger`]).
    pub lot_triggers: Vec<LotTrigger>,
}

/// A study due for its review, with EVERY reason that applies (G1 review — the first reason hid
/// the others), in the fixed order `"age"` (older than the cadence), `"age_unknown"` (the history
/// read failed), `"withheld"` (a load-bearing input missing), `"low_confidence"`,
/// `"not_computable"` (the data does not normalize).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueStudy {
    /// The ticker — followed by the study's currency (« NESN (USD) ») when the ticker's lots link
    /// to different studies, so two due studies of one ticker are told apart.
    pub ticker: String,
    pub study_id: Uuid,
    /// `None` when the history read failed (the date is unknown — see [`ReviewStudyFacts`]).
    pub last_saved: Option<String>,
    pub reasons: Vec<&'static str>,
}

/// Counts only — a number is a fact; no judgement is derived from them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewCounts {
    pub positions: usize,
    /// Positions with a study — computable or not.
    pub linked: usize,
    pub full: usize,
    pub provisional: usize,
    pub withheld: usize,
    pub not_computable: usize,
    pub flagged: usize,
    pub high_zone: usize,
    pub stop_breached: usize,
    pub due: usize,
}

/// The whole review — every block is an existing read or the engine's snapshot.
pub struct PortfolioReviewFacts {
    pub reference_currency: String,
    /// Today, `YYYY-MM-DD` (the injected clock, ADD15).
    pub today: String,
    pub bank_count: usize,
    pub diversification: JournalDiversification,
    pub sectors: Option<SectorExposure>,
    pub currencies: Option<CurrencyExposure>,
    pub consolidation: JournalConsolidation,
    /// Largest invested share first; absent invested last; ticker tiebreak.
    pub positions: Vec<ReviewPosition>,
    pub due: Vec<DueStudy>,
    pub counts: ReviewCounts,
    /// Every rate any block used, deduplicated per pair — the FR28 footnote.
    pub rates_used: Vec<FxRateItem>,
}

/// One lot's resolved link: the study state, the study's currency and entered price, whether the
/// price sits in the Sell zone, and the link's identity (to tell lots apart).
#[derive(Clone)]
pub(super) struct LotLink {
    pub(super) study: ReviewStudy,
    pub(super) study_currency: Option<String>,
    pub(super) price: Option<Decimal>,
    pub(super) in_sell_zone: bool,
    /// `Some(study id)` for a study, `None` for no / an unreadable study.
    pub(super) identity: Option<Uuid>,
}

/// `today` minus the review cadence, as a `YYYY-MM-DD` string comparable lexicographically
/// (12 months = the same month-day one year earlier; a Feb-29 today compares as Feb-29, which
/// only ever makes a Feb-28 save one day « younger » — immaterial for a yearly cadence).
fn review_threshold(today: &str) -> Option<String> {
    let year: i32 = today.get(0..4)?.parse().ok()?;
    let rest = today.get(4..10)?;
    let years_back = i32::try_from(REVIEW_CADENCE_MONTHS / 12).ok()?;
    Some(format!("{:04}{rest}", year - years_back))
}

/// The last effective save's day: the latest FR51 snapshot, else the study's creation — or
/// `None` when the history read FAILED (an unknown date, never the creation date passed off as
/// the last save: that would state a false « plus de 12 mois »).
fn last_saved_day<E>(history: Result<Vec<String>, E>, created_at: &str) -> Option<String> {
    let latest = history.ok()?.into_iter().max();
    Some(
        latest
            .as_deref()
            .unwrap_or(created_at)
            .chars()
            .take(10)
            .collect(),
    )
}

/// Every reason a study is due for its review, in the fixed order age · age unknown · withheld
/// · low confidence · not computable — all of them, never only the first (G1 review).
/// `verdict` is `None` for a study the engine cannot compute.
fn due_reasons(
    last_saved: Option<&str>,
    due_for_review: bool,
    verdict: Option<&str>,
    low_confidence: bool,
) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if due_for_review {
        reasons.push("age");
    }
    if last_saved.is_none() {
        reasons.push("age_unknown");
    }
    if verdict == Some("withheld") {
        reasons.push("withheld");
    }
    if low_confidence {
        reasons.push("low_confidence");
    }
    if verdict.is_none() {
        reasons.push("not_computable");
    }
    reasons
}

/// One lot's stop against its own study: breached only when the price is known AND in the
/// stop's currency (never a cross-currency comparison — a legacy lot may match a study in
/// another currency ticker-only). A legacy lot with NO declared currency (`lot_currency` is
/// `None`) has a stop of unknown unit: never labelled with a currency it does not carry, never
/// compared (G1 final review). `None` when the lot carries no (parsable) stop.
pub(super) fn lot_stop(
    level: Option<&str>,
    lot_currency: Option<&str>,
    bank: &str,
    study_currency: Option<&str>,
    price: Option<Decimal>,
) -> Option<StopFact> {
    let level = Decimal::from_str_exact(level?).ok()?;
    let same_currency = lot_currency
        .zip(study_currency)
        .is_some_and(|(lot, study)| study.eq_ignore_ascii_case(lot));
    let breached =
        same_currency && price.is_some_and(|p| steadyinvest_core::risk::stop_breached(level, p));
    Some(StopFact {
        level,
        currency: lot_currency.map(str::to_uppercase),
        bank: bank.to_string(),
        breached,
    })
}

/// Which of a position's lot links the row shows — by IDENTITY, never by lot position (the
/// discriminator rule, G1 final review): among the links to a study, the NEWEST study
/// (`study_order` = the studies oldest first, as listed; the ticker's own study — the one every
/// ticker-only match resolves — is the newest); an unlisted identity ranks below, ties broken by
/// id. Without any study, a failed read wins over « aucune étude » (a row never states an
/// absence one of its lots could not establish). `links` is never empty (a position has a lot).
pub(super) fn row_link(links: &[&LotLink], study_order: &[Uuid]) -> usize {
    let rank = |id: Uuid| study_order.iter().position(|s| *s == id);
    links
        .iter()
        .enumerate()
        .filter_map(|(i, l)| l.identity.map(|id| (i, id)))
        .max_by(|(_, a), (_, b)| rank(*a).cmp(&rank(*b)).then_with(|| a.cmp(b)))
        .map(|(i, _)| i)
        .or_else(|| {
            links
                .iter()
                .position(|l| matches!(l.study, ReviewStudy::Unavailable))
        })
        .unwrap_or(0)
}

/// The lots' distinct links, when there is more than one: each link's study currency, in lot
/// order (`"—"` for a lot without a readable study). Empty when every lot shares one link.
pub(super) fn mixed_links(links: &[&LotLink]) -> Vec<String> {
    let mut seen: Vec<(Option<Uuid>, String)> = Vec::new();
    for l in links {
        let label = match (&l.identity, &l.study_currency) {
            (Some(_), Some(c)) => c.clone(),
            _ => "—".to_string(),
        };
        if !seen
            .iter()
            .any(|(id, lab)| *id == l.identity && *lab == label)
        {
            seen.push((l.identity, label));
        }
    }
    if seen.len() > 1 {
        seen.into_iter().map(|(_, label)| label).collect()
    } else {
        Vec::new()
    }
}

impl JournalState {
    /// The annual-review clock of a study already read: its last save (unknown on a failed
    /// history read) and whether it is older than the cadence.
    fn review_clock(&self, s: &Study, threshold: Option<&str>) -> (Option<String>, bool) {
        let last_saved = last_saved_day(
            self.try_list_study_history(s.id)
                .map(|h| h.into_iter().map(|e| e.created_at.0).collect::<Vec<_>>()),
            &s.created_at.0,
        );
        let due = match (threshold, last_saved.as_deref()) {
            (Some(t), Some(saved)) => saved < t,
            _ => false,
        };
        (last_saved, due)
    }

    /// One lot's link, matched EXACTLY as the register matches it (#81 / #218 — the lot's
    /// DECLARED currency; a legacy `None` lot matches ticker-only); a read failure is its own
    /// state, a normalize failure too.
    pub(super) fn lot_link(
        &self,
        ticker: &str,
        currency: Option<&str>,
        threshold: Option<&str>,
    ) -> LotLink {
        let unlinked = |study| LotLink {
            study,
            study_currency: None,
            price: None,
            in_sell_zone: false,
            identity: None,
        };
        let s = match self.try_matched_study_in_currency(ticker, currency) {
            Err(_) => return unlinked(ReviewStudy::Unavailable),
            Ok(None) => {
                // The other-currency cause, read FALLIBLY (#95): a failed lookup is
                // « indisponible », never a cause-less « aucune étude » that may be false.
                let other = self
                    .try_study_id_for_ticker(ticker)
                    .and_then(|id| match id {
                        Some(id) => self.try_get_study(id),
                        None => Ok(None),
                    });
                return match other {
                    Err(_) => unlinked(ReviewStudy::Unavailable),
                    Ok(other) => unlinked(ReviewStudy::None {
                        other_currency: other.map(|s| s.native_currency.to_uppercase()),
                    }),
                };
            }
            Ok(Some(s)) => s,
        };
        let price = s.judgment.current_price.map(|m| m.as_decimal());
        let (last_saved, due_for_review) = self.review_clock(&s, threshold);
        let study_currency = Some(s.native_currency.to_uppercase());
        // The study is already read: its snapshot is built directly (THE engine call), so a
        // normalize failure is its own state — never worded as a read failure.
        match engine::build_snapshot(&s) {
            Err(_) => LotLink {
                study: ReviewStudy::NotComputable(NotComputableFacts {
                    study_id: s.id,
                    current_price: price,
                    last_saved,
                    due_for_review,
                }),
                study_currency,
                price,
                in_sell_zone: false,
                identity: Some(s.id),
            },
            Ok(snapshot) => {
                let outputs = snapshot.outputs();
                // The zone / verdict keys of every other surface (viewmodel::engine).
                let zone = engine::zone_position_key(&outputs.risk_reward, price);
                LotLink {
                    study: ReviewStudy::Linked(ReviewStudyFacts {
                        study_id: s.id,
                        company_name: s.company_name.clone(),
                        currency: s.native_currency.to_uppercase(),
                        verdict: engine::verdict_state(snapshot.verdict()),
                        low_confidence: outputs.low_confidence,
                        zone,
                        current_price: price,
                        upside_downside: outputs.risk_reward.upside_downside,
                        relative_value_pct: outputs.valuation.relative_value_pct,
                        quality_flags: outputs.quality_flags.clone(),
                        last_saved,
                        due_for_review,
                    }),
                    study_currency,
                    price,
                    in_sell_zone: zone == "sell",
                    identity: Some(s.id),
                }
            }
        }
    }

    /// The portfolio health review (Story 7.2). `Err` on a read FAILURE of the holdings or the
    /// portfolios (the whole view is « indisponible » — no honest partial exists for a roll-up);
    /// a single study's read failure marks THAT row `Unavailable` and the rest stands.
    pub fn portfolio_review(
        &self,
        reference_currency: &str,
        small_max: Decimal,
        medium_max: Decimal,
    ) -> Result<PortfolioReviewFacts, String> {
        let journal = self.journal.as_ref().ok_or(MSG_NO_JOURNAL.to_string())?;
        let portfolios = journal.list_portfolios().map_err(|e| e.to_string())?;
        let holdings: Vec<_> = journal
            .list_all_holdings()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|h| h.sold_at.is_none())
            .collect();
        let today: String = self.clock.now().0.chars().take(10).collect();
        let threshold = review_threshold(&today);

        let diversification =
            self.journal_diversification(reference_currency, small_max, medium_max);
        let sectors = self.journal_sector_exposure(reference_currency);
        let currencies = self.journal_currency_exposure(reference_currency);
        let consolidation = self.journal_capital_at_risk_consolidation(reference_currency);

        // ── One row per held ticker, aggregated across banks (portfolio order for the banks). ──
        let bank_name = |id: Uuid| -> String {
            portfolios
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.clone())
                .unwrap_or_default()
        };
        let mut tickers: Vec<String> = holdings
            .iter()
            .map(|h| h.security_ticker.to_uppercase())
            .collect();
        tickers.sort();
        tickers.dedup();
        // One study resolution per (ticker, declared currency) — lots sharing both share it.
        let mut link_cache: BTreeMap<(String, Option<String>), LotLink> = BTreeMap::new();
        // The studies oldest first — the recency that picks a row's study by identity. A failed
        // listing leaves it empty: the lots' own reads then fail too (« indisponible »), and a
        // tie falls to the id — never to a lot's position.
        let study_order: Vec<Uuid> = self
            .try_list_studies()
            .map(|all| all.into_iter().map(|s| s.id).collect())
            .unwrap_or_default();
        let mut positions = Vec::new();
        let mut due = Vec::new();
        let mut counts = ReviewCounts::default();
        for ticker in tickers {
            let held: Vec<_> = holdings
                .iter()
                .filter(|h| h.security_ticker.to_uppercase() == ticker)
                .collect();
            let first = held[0];
            let mut banks: Vec<String> = portfolios
                .iter()
                .filter(|p| held.iter().any(|h| h.portfolio_id == p.id))
                .map(|p| p.name.clone())
                .collect();
            if banks.is_empty() {
                banks.push(bank_name(first.portfolio_id));
            }
            let conc = diversification.rows.iter().find(|r| r.ticker == ticker);
            let (invested, share_pct, missing_pairs) = match conc {
                Some(r) => (r.invested, r.share_pct, r.missing_pairs.clone()),
                None => (None, None, Vec::new()),
            };
            // Each LOT's own link — the register's per-row rule (G1 review: never held[0] for
            // every bank's lot).
            let mut links: Vec<LotLink> = Vec::new();
            for h in &held {
                let declared = h.currency.as_deref().map(str::to_uppercase);
                let link = link_cache
                    .entry((ticker.clone(), declared.clone()))
                    .or_insert_with(|| {
                        self.lot_link(&ticker, declared.as_deref(), threshold.as_deref())
                    })
                    .clone();
                links.push(link);
            }
            let link_refs: Vec<&LotLink> = links.iter().collect();
            let mixed = mixed_links(&link_refs);
            // Every lot's stop against its own study, and each lot's trigger keyed by its
            // holding (the stop takes priority, as per lot in the register). A legacy lot's stop
            // carries no currency: its unit is unknown (G1 final review).
            let mut stops = Vec::new();
            let mut lot_triggers = Vec::new();
            for (h, link) in held.iter().zip(&links) {
                let declared = h.currency.as_deref().map(str::to_uppercase);
                let stop = lot_stop(
                    h.trailing_stop_level.as_deref(),
                    declared.as_deref(),
                    &bank_name(h.portfolio_id),
                    link.study_currency.as_deref(),
                    link.price,
                );
                let breached = stop.as_ref().is_some_and(|s| s.breached);
                let kind = match steadyinvest_core::risk::trigger_state(breached, link.in_sell_zone)
                {
                    Some(steadyinvest_core::risk::TriggerKind::Stop) => Some("stop"),
                    Some(steadyinvest_core::risk::TriggerKind::Sell) => Some("sell"),
                    None => None,
                };
                lot_triggers.extend(kind.map(|kind| LotTrigger {
                    holding_id: h.id,
                    kind,
                }));
                stops.extend(stop);
            }
            let trigger = position_trigger(&lot_triggers, |_| false);
            let stop_breached = stops.iter().any(|s| s.breached);
            // The row's study by IDENTITY (never the first lot's), and the position's currency
            // as the lot of that study declares it.
            let shown = row_link(&link_refs, &study_order);
            let study = links[shown].study.clone();
            let currency = super::effective_currency(held[shown], reference_currency);
            // Counts: the verdict partition follows the row's study (full + provisional +
            // withheld + not computable = linked); the flag / zone counts and the due list read
            // EVERY lot's study (G1 final review — a second lot's study is not invisible).
            counts.positions += 1;
            match &study {
                ReviewStudy::Linked(f) => {
                    counts.linked += 1;
                    match f.verdict {
                        "full" => counts.full += 1,
                        "provisional" => counts.provisional += 1,
                        _ => counts.withheld += 1,
                    }
                }
                ReviewStudy::NotComputable(_) => {
                    counts.linked += 1;
                    counts.not_computable += 1;
                }
                ReviewStudy::None { .. } | ReviewStudy::Unavailable => {}
            }
            let linked_facts = || {
                links.iter().filter_map(|l| match &l.study {
                    ReviewStudy::Linked(f) => Some(f),
                    _ => None,
                })
            };
            if linked_facts().any(|f| !f.quality_flags.is_empty()) {
                counts.flagged += 1;
            }
            if linked_facts().any(|f| f.zone == "sell" || f.zone == "above") {
                counts.high_zone += 1;
            }
            // The due list: every distinct study of the lots, the row's first, each once.
            let mut seen: Vec<Uuid> = Vec::new();
            let order = std::iter::once(&links[shown]).chain(links.iter());
            for link in order {
                let Some(id) = link.identity else { continue };
                if seen.contains(&id) {
                    continue;
                }
                seen.push(id);
                let label = match (&link.study_currency, mixed.is_empty()) {
                    (Some(c), false) => format!("{ticker} ({c})"),
                    _ => ticker.clone(),
                };
                let entry = match &link.study {
                    ReviewStudy::Linked(f) => Some((
                        f.study_id,
                        f.last_saved.clone(),
                        due_reasons(
                            f.last_saved.as_deref(),
                            f.due_for_review,
                            Some(f.verdict),
                            f.low_confidence,
                        ),
                    )),
                    // A study the engine cannot compute is still a study, and always « à
                    // revoir » (its data must be repaired).
                    ReviewStudy::NotComputable(f) => Some((
                        f.study_id,
                        f.last_saved.clone(),
                        due_reasons(f.last_saved.as_deref(), f.due_for_review, None, false),
                    )),
                    ReviewStudy::None { .. } | ReviewStudy::Unavailable => None,
                };
                if let Some((study_id, last_saved, reasons)) = entry
                    && !reasons.is_empty()
                {
                    due.push(DueStudy {
                        ticker: label,
                        study_id,
                        last_saved,
                        reasons,
                    });
                }
            }
            if stop_breached {
                counts.stop_breached += 1;
            }
            positions.push(ReviewPosition {
                ticker,
                banks,
                currency,
                invested,
                share_pct,
                missing_pairs,
                study,
                mixed_links: mixed,
                stops,
                stop_breached,
                trigger,
                lot_triggers,
            });
        }
        counts.due = due.len();
        // Largest invested first; absent invested last; ticker tiebreak (the 6.7 order).
        positions.sort_by(|a, b| match (a.invested, b.invested) {
            (Some(x), Some(y)) => y.cmp(&x).then_with(|| a.ticker.cmp(&b.ticker)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.ticker.cmp(&b.ticker),
        });

        // The FR28 footnote: every rate any block used, once per pair.
        let mut rates_used: Vec<FxRateItem> = Vec::new();
        let mut push_rates = |rates: &[FxRateItem]| {
            for r in rates {
                if !rates_used.iter().any(|u| {
                    u.base_currency == r.base_currency && u.quote_currency == r.quote_currency
                }) {
                    rates_used.push(r.clone());
                }
            }
        };
        push_rates(&diversification.rates_used);
        push_rates(&consolidation.rates_used);
        if let Some(c) = &currencies {
            push_rates(&c.rates_used);
        }

        Ok(PortfolioReviewFacts {
            reference_currency: reference_currency.to_string(),
            today,
            bank_count: portfolios.len(),
            diversification,
            sectors,
            currencies,
            consolidation,
            positions,
            due,
            counts,
            rates_used,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    #[test]
    fn the_review_threshold_is_the_same_day_one_year_earlier() {
        assert_eq!(
            review_threshold("2026-09-24").as_deref(),
            Some("2025-09-24")
        );
        assert_eq!(review_threshold("bad"), None);
        // Lexicographic comparison is the intended clock: a save the day before is due, the
        // same day is not.
        assert!("2025-09-23" < "2025-09-24");
        assert!(("2025-09-24" >= "2025-09-24"));
    }

    #[test]
    fn a_failed_history_read_leaves_the_last_save_unknown() {
        // The latest snapshot wins; an empty history falls back to the creation; a FAILED read
        // is unknown — never the creation date passed off as the last save.
        let created = "2024-03-01T10:00:00Z";
        assert_eq!(
            last_saved_day::<()>(
                Ok(vec![
                    "2025-01-02T00:00:00Z".into(),
                    "2026-02-03T00:00:00Z".into()
                ]),
                created
            )
            .as_deref(),
            Some("2026-02-03")
        );
        assert_eq!(
            last_saved_day::<()>(Ok(Vec::new()), created).as_deref(),
            Some("2024-03-01")
        );
        assert_eq!(last_saved_day(Err("disk"), created), None);
    }

    #[test]
    fn every_due_reason_is_kept_in_order() {
        assert_eq!(
            due_reasons(Some("2024-01-01"), true, Some("withheld"), true),
            vec!["age", "withheld", "low_confidence"]
        );
        assert_eq!(
            due_reasons(Some("2026-01-01"), false, Some("provisional"), true),
            vec!["low_confidence"]
        );
        assert!(due_reasons(Some("2026-01-01"), false, Some("full"), false).is_empty());
        // An unknown last save is its own reason; a study the engine cannot compute too.
        assert_eq!(
            due_reasons(None, false, Some("full"), false),
            vec!["age_unknown"]
        );
        assert_eq!(
            due_reasons(Some("2024-01-01"), true, None, false),
            vec!["age", "not_computable"]
        );
    }

    #[test]
    fn a_lots_stop_is_read_against_its_own_study_in_its_own_currency() {
        let chf = Some("CHF");
        let s = lot_stop(Some("63"), chf, "UBS", Some("CHF"), Some(d("60"))).unwrap();
        assert!(s.breached);
        assert_eq!((s.bank.as_str(), s.currency.as_deref()), ("UBS", chf));
        assert!(
            !lot_stop(Some("63"), chf, "UBS", Some("CHF"), Some(d("70")))
                .unwrap()
                .breached
        );
        // A price in another currency is never compared with the stop.
        assert!(
            !lot_stop(Some("63"), chf, "UBS", Some("USD"), Some(d("10")))
                .unwrap()
                .breached
        );
        // An unknown price breaches nothing; no stop is no fact.
        assert!(
            !lot_stop(Some("63"), chf, "UBS", Some("CHF"), None)
                .unwrap()
                .breached
        );
        assert_eq!(lot_stop(None, chf, "UBS", Some("CHF"), Some(d("1"))), None);
    }

    #[test]
    fn a_legacy_lots_stop_carries_no_currency_and_is_never_compared() {
        // G1 final review: a lot without a declared currency — its stop's unit is unknown. It
        // is neither labelled with the reference currency nor compared, even when the price
        // would reach it in the study's own currency.
        let s = lot_stop(Some("63"), None, "UBS", Some("CHF"), Some(d("1"))).unwrap();
        assert_eq!(s.currency, None);
        assert!(!s.breached);
    }

    fn link(id: Option<u128>, study: ReviewStudy) -> LotLink {
        LotLink {
            study,
            study_currency: None,
            price: None,
            in_sell_zone: false,
            identity: id.map(Uuid::from_u128),
        }
    }

    #[test]
    fn a_rows_study_is_chosen_by_identity_never_by_lot_position() {
        let none = || ReviewStudy::None {
            other_currency: None,
        };
        let old = link(Some(1), ReviewStudy::Unavailable);
        let new = link(Some(2), ReviewStudy::Unavailable);
        let order = [Uuid::from_u128(1), Uuid::from_u128(2)];
        // The newest study wins, whichever lot links it — reordering the lots changes nothing.
        assert_eq!(row_link(&[&old, &new], &order), 1);
        assert_eq!(row_link(&[&new, &old], &order), 0);
        // A lot without a study never hides a lot with one.
        let bare = link(None, none());
        assert_eq!(row_link(&[&bare, &old], &order), 1);
        // Without any study, a failed read wins over « aucune étude ».
        let failed = link(None, ReviewStudy::Unavailable);
        assert_eq!(row_link(&[&bare, &failed], &order), 1);
        assert_eq!(row_link(&[&failed, &bare], &order), 0);
        // An unlisted identity ranks below a listed one; ties fall to the id.
        let stray = link(Some(9), ReviewStudy::Unavailable);
        assert_eq!(row_link(&[&stray, &old], &order), 1);
        assert_eq!(row_link(&[&stray, &link(Some(3), none())], &[]), 0);
    }

    #[test]
    fn a_dismissed_lots_trigger_is_not_shown_the_stop_first() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let lots = [
            LotTrigger {
                holding_id: a,
                kind: "stop",
            },
            LotTrigger {
                holding_id: b,
                kind: "sell",
            },
        ];
        assert_eq!(position_trigger(&lots, |_| false), "stop");
        assert_eq!(position_trigger(&lots, |id| id == a), "sell");
        assert_eq!(position_trigger(&lots, |_| true), "");
        assert_eq!(position_trigger(&[], |_| false), "");
    }

    #[test]
    fn distinct_links_are_named_only_when_the_lots_disagree() {
        let link = |id: Option<u128>, cur: Option<&str>| LotLink {
            study: ReviewStudy::Unavailable,
            study_currency: cur.map(str::to_string),
            price: None,
            in_sell_zone: false,
            identity: id.map(Uuid::from_u128),
        };
        let chf = link(Some(1), Some("CHF"));
        let usd = link(Some(2), Some("USD"));
        let none = link(None, None);
        assert!(mixed_links(&[&chf, &chf]).is_empty(), "one shared study");
        assert_eq!(mixed_links(&[&chf, &usd, &chf]), vec!["CHF", "USD"]);
        assert_eq!(mixed_links(&[&chf, &none]), vec!["CHF", "—"]);
    }
}
