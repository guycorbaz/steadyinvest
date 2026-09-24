//! Story 7.2 — the portfolio health review: ONE composed, read-only view of the whole dossier
//! (every bank, every currency) — the diversification reads of Epic 6 (size mix, sector and
//! currency exposure, per-bank consolidation, per-security concentration), one engine snapshot per
//! linked study (verdict state, zone, U/D, relative value, quality flags), the FR51 history for the
//! annual-review due date, and counts. Facts only (FR13): nothing is ranked, scored or
//! recommended, and nothing is computed that the existing reads / the engine do not already
//! compute. Absence honesty (#95): a read failure is `Err` (« indisponible »), never an empty
//! section; a missing FX pair is named on the figure it blocks.

use rust_decimal::Decimal;
use steadyinvest_core::ssg::{QualityFlagKey, UpsideDownside, Zone};
use steadyinvest_core::verdict::Verdict;
use steadyinvest_persistence::FxRateItem;
use uuid::Uuid;

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
    /// creation) — the annual-review clock.
    pub last_saved: String,
    /// `last_saved` is older than [`REVIEW_CADENCE_MONTHS`].
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
    /// The trailing stop as a level in the position's currency, when set (the first bank's).
    pub stop_level: Option<Decimal>,
    pub stop_breached: bool,
    /// `"stop"` | `"sell"` | `""` — the neutral trigger (core::risk), as the register shows it.
    pub trigger: &'static str,
}

/// A study due for its review, with the reason: `"age"` (older than the cadence), `"withheld"`
/// (a load-bearing input missing) or `"low_confidence"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueStudy {
    pub ticker: String,
    pub study_id: Uuid,
    pub last_saved: String,
    pub reason: &'static str,
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

/// The present price's position against the §4 band: the zone inside it, the honest « below »
/// / « above » outside it (the register's 2026-07-12 rule), `""` without a band or a price.
fn zone_position(
    r: &steadyinvest_core::ssg::RiskRewardOutputs,
    price: Option<Decimal>,
) -> &'static str {
    if let Some(zone) = r.present_price_zone {
        return match zone {
            Zone::Buy => "buy",
            Zone::Neutral => "neutral",
            Zone::Sell => "sell",
        };
    }
    match (r.zones.as_ref(), price) {
        (Some(b), Some(p)) if p < b.forecast_low => "below",
        (Some(b), Some(p)) if p > b.forecast_high => "above",
        _ => "",
    }
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
            let (study, current_price, in_sell_zone) =
                match self.try_matched_study_in_currency(&ticker, Some(&currency)) {
                    Err(_) => (ReviewStudy::Unavailable, None, false),
                    Ok(None) => {
                        let other = self
                            .study_id_for_ticker(&ticker)
                            .and_then(|id| self.get_study(id))
                            .map(|s| s.native_currency.to_uppercase());
                        (
                            ReviewStudy::None {
                                other_currency: other,
                            },
                            None,
                            false,
                        )
                    }
                    Ok(Some(s)) => match self.snapshot_for(s.id) {
                        Err(_) => (ReviewStudy::Unavailable, None, false),
                        Ok(snapshot) => {
                            let outputs = snapshot.outputs();
                            let price = s.judgment.current_price.map(|m| m.as_decimal());
                            let zone = zone_position(&outputs.risk_reward, price);
                            let verdict = match snapshot.verdict() {
                                Verdict::Full(_) => "full",
                                Verdict::Provisional(_) => "provisional",
                                Verdict::Withheld(_) => "withheld",
                            };
                            // The annual-review clock: the latest FR51 snapshot, else creation.
                            let last_saved: String = self
                                .try_list_study_history(s.id)
                                .ok()
                                .and_then(|h| h.into_iter().map(|e| e.created_at.0).max())
                                .unwrap_or_else(|| s.created_at.0.clone())
                                .chars()
                                .take(10)
                                .collect();
                            let due_for_review = threshold
                                .as_deref()
                                .is_some_and(|t| last_saved.as_str() < t);
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
            // The trailing stop (the first bank's row) and the neutral trigger (core::risk).
            let stop_level = first
                .trailing_stop_level
                .as_deref()
                .and_then(|s| Decimal::from_str_exact(s).ok());
            let stop_breached = match (stop_level, current_price) {
                (Some(level), Some(price)) => steadyinvest_core::risk::stop_breached(level, price),
                _ => false,
            };
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
                let reason = if f.due_for_review {
                    Some("age")
                } else if f.verdict == "withheld" {
                    Some("withheld")
                } else if f.low_confidence {
                    Some("low_confidence")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    due.push(DueStudy {
                        ticker: ticker.clone(),
                        study_id: f.study_id,
                        last_saved: f.last_saved.clone(),
                        reason,
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
                stop_level,
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
}
