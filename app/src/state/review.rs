//! Story 7.2 — the portfolio health review: ONE composed, read-only view of the whole dossier
//! (every bank, every currency) — the diversification reads of Epic 6 (size mix, sector and
//! currency exposure, per-bank consolidation, per-security concentration), one engine snapshot per
//! linked study (verdict state, zone, U/D, relative value, quality flags), the FR51 history for the
//! annual-review due date, and counts. Facts only (FR13): nothing is ranked, scored or
//! recommended, and nothing is computed that the existing reads / the engine do not already
//! compute. Absence honesty (#95): a read failure is `Err` (« indisponible »), never an empty
//! section; a missing FX pair is named on the figure it blocks.

use rust_decimal::Decimal;
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
    /// read (G1 review), and the study can still be opened to repair it.
    NotComputable(Uuid),
    /// A linked study and its snapshot facts.
    Linked(ReviewStudyFacts),
}

/// The engine-snapshot facts of one linked study, as the engine states them (no formatting here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewStudyFacts {
    pub study_id: Uuid,
    pub company_name: Option<String>,
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

/// One held ticker, aggregated across banks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewPosition {
    pub ticker: String,
    /// The banks (portfolio names) holding it, in portfolio order.
    pub banks: Vec<String>,
    /// The position's currency (the first bank's; every bank's row for a ticker shares the
    /// study, hence the currency, since #218 — a legacy mix is shown as its first currency).
    pub currency: String,
    /// Invested in the reference currency (the 6.7 concentration row) — `None` when a pair is
    /// missing (named in `missing_pairs`) or the total could not form.
    pub invested: Option<Decimal>,
    pub share_pct: Option<Decimal>,
    pub missing_pairs: Vec<String>,
    pub study: ReviewStudy,
    /// The trailing-stop levels of EVERY held lot of the ticker that carries one (distinct, in
    /// lot order), in the position's currency — empty when no lot has a stop (G1 review: a stop
    /// on another bank's lot was ignored).
    pub stop_levels: Vec<Decimal>,
    /// Any lot's stop is reached by the study's present price (`core::risk::stop_breached`).
    pub stop_breached: bool,
    /// `"stop"` | `"sell"` | `""` — the neutral trigger (core::risk), as the register shows it.
    pub trigger: &'static str,
}

/// A study due for its review, with EVERY reason that applies (G1 review — the first reason hid
/// the others), in the fixed order `"age"` (older than the cadence), `"withheld"` (a
/// load-bearing input missing), `"low_confidence"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueStudy {
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
    pub linked: usize,
    pub full: usize,
    pub provisional: usize,
    pub withheld: usize,
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

/// Every reason a linked study is due for its review, in the fixed order age · withheld · low
/// confidence — all of them, never only the first (G1 review).
fn due_reasons(due_for_review: bool, verdict: &str, low_confidence: bool) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if due_for_review {
        reasons.push("age");
    }
    if verdict == "withheld" {
        reasons.push("withheld");
    }
    if low_confidence {
        reasons.push("low_confidence");
    }
    reasons
}

/// The trailing stop across EVERY held lot of a ticker (every bank): the distinct levels, in lot
/// order, and whether any of them is breached by the present price. An unknown price breaches
/// nothing (the register's rule); an unparsable stored level is skipped (unreachable through the
/// validated write path).
fn stops_across_lots<'a>(
    levels: impl IntoIterator<Item = Option<&'a str>>,
    price: Option<Decimal>,
) -> (Vec<Decimal>, bool) {
    let mut distinct: Vec<Decimal> = Vec::new();
    for level in levels
        .into_iter()
        .flatten()
        .filter_map(|s| Decimal::from_str_exact(s).ok())
    {
        if !distinct.contains(&level) {
            distinct.push(level);
        }
    }
    let breached = price.is_some_and(|p| {
        distinct
            .iter()
            .any(|level| steadyinvest_core::risk::stop_breached(*level, p))
    });
    (distinct, breached)
}

impl JournalState {
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
        let mut positions = Vec::new();
        let mut due = Vec::new();
        let mut counts = ReviewCounts::default();
        for ticker in tickers {
            let held: Vec<_> = holdings
                .iter()
                .filter(|h| h.security_ticker.to_uppercase() == ticker)
                .collect();
            let first = held[0];
            // The displayed currency is the effective one (the 6.2 coalescing); the study MATCH
            // below uses the lot's DECLARED currency exactly as the register does — a legacy
            // `None` row matches ticker-only there (G1 review: the two surfaces disagreed).
            let currency = super::effective_currency(first, reference_currency);
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
            // The study in the position's currency (#81 / #218); a read failure is its own state.
            let (study, current_price, in_sell_zone) = match self
                .try_matched_study_in_currency(&ticker, first.currency.as_deref())
            {
                Err(_) => (ReviewStudy::Unavailable, None, false),
                Ok(None) => {
                    // The other-currency cause, read FALLIBLY (#95): a failed lookup is
                    // « indisponible », never a cause-less « aucune étude » that may be false.
                    let other = self
                        .try_study_id_for_ticker(&ticker)
                        .and_then(|id| match id {
                            Some(id) => self.try_get_study(id),
                            None => Ok(None),
                        });
                    match other {
                        Err(_) => (ReviewStudy::Unavailable, None, false),
                        Ok(other) => (
                            ReviewStudy::None {
                                other_currency: other.map(|s| s.native_currency.to_uppercase()),
                            },
                            None,
                            false,
                        ),
                    }
                }
                // The study is already read: its snapshot is built directly (THE engine call), so
                // a normalize failure is its own state — never worded as a read failure.
                Ok(Some(s)) => match engine::build_snapshot(&s) {
                    Err(_) => (ReviewStudy::NotComputable(s.id), None, false),
                    Ok(snapshot) => {
                        let outputs = snapshot.outputs();
                        let price = s.judgment.current_price.map(|m| m.as_decimal());
                        // The zone / verdict keys of every other surface (viewmodel::engine).
                        let zone = engine::zone_position_key(&outputs.risk_reward, price);
                        let verdict = engine::verdict_state(snapshot.verdict());
                        // The annual-review clock: the latest FR51 snapshot, else creation —
                        // unknown when the history read failed.
                        let last_saved = last_saved_day(
                            self.try_list_study_history(s.id)
                                .map(|h| h.into_iter().map(|e| e.created_at.0).collect::<Vec<_>>()),
                            &s.created_at.0,
                        );
                        let due_for_review = match (threshold.as_deref(), last_saved.as_deref()) {
                            (Some(t), Some(saved)) => saved < t,
                            _ => false,
                        };
                        let facts = ReviewStudyFacts {
                            study_id: s.id,
                            company_name: s.company_name.clone(),
                            verdict,
                            low_confidence: outputs.low_confidence,
                            zone,
                            current_price: price,
                            upside_downside: outputs.risk_reward.upside_downside,
                            relative_value_pct: outputs.valuation.relative_value_pct,
                            quality_flags: outputs.quality_flags.clone(),
                            last_saved,
                            due_for_review,
                        };
                        (ReviewStudy::Linked(facts), price, zone == "sell")
                    }
                },
            };
            // The trailing stop across EVERY held lot (every bank — never the first lot alone)
            // and the neutral trigger (core::risk).
            let (stop_levels, stop_breached) = stops_across_lots(
                held.iter().map(|h| h.trailing_stop_level.as_deref()),
                current_price,
            );
            let trigger = match steadyinvest_core::risk::trigger_state(stop_breached, in_sell_zone)
            {
                Some(steadyinvest_core::risk::TriggerKind::Stop) => "stop",
                Some(steadyinvest_core::risk::TriggerKind::Sell) => "sell",
                None => "",
            };
            // Counts + the due list.
            counts.positions += 1;
            if let ReviewStudy::Linked(f) = &study {
                counts.linked += 1;
                match f.verdict {
                    "full" => counts.full += 1,
                    "provisional" => counts.provisional += 1,
                    _ => counts.withheld += 1,
                }
                if !f.quality_flags.is_empty() {
                    counts.flagged += 1;
                }
                if f.zone == "sell" || f.zone == "above" {
                    counts.high_zone += 1;
                }
                let reasons = due_reasons(f.due_for_review, f.verdict, f.low_confidence);
                if !reasons.is_empty() {
                    due.push(DueStudy {
                        ticker: ticker.clone(),
                        study_id: f.study_id,
                        last_saved: f.last_saved.clone(),
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
                stop_levels,
                stop_breached,
                trigger,
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
            due_reasons(true, "withheld", true),
            vec!["age", "withheld", "low_confidence"]
        );
        assert_eq!(
            due_reasons(false, "provisional", true),
            vec!["low_confidence"]
        );
        assert!(due_reasons(false, "full", false).is_empty());
    }

    #[test]
    fn the_stop_is_read_on_every_lot() {
        let d = |s: &str| Decimal::from_str_exact(s).unwrap();
        // The first lot has no stop; the second one's is breached.
        let (levels, breached) = stops_across_lots([None, Some("63"), Some("63")], Some(d("60")));
        assert_eq!(levels, vec![d("63")], "distinct levels");
        assert!(breached);
        let (levels, breached) = stops_across_lots([Some("50"), Some("63")], Some(d("70")));
        assert_eq!(levels, vec![d("50"), d("63")]);
        assert!(!breached);
        // An unknown price breaches nothing.
        assert!(!stops_across_lots([Some("63")], None).1);
    }
}
