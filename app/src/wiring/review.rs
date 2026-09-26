//! Story 7.2 — the « Revue » screen: push the composed state read into the `Review` global on
//! activation (issue #94 — never stale on arrival), and the FR53 export through the native `rfd`
//! save picker (the 5.6 rail). Formatting happens HERE (the one float→string boundary of the app);
//! the state read carries engine values, the report carries its own labels.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use rust_decimal::Decimal;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use steadyinvest_core::rounding::DisplayField;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::state::{self, JournalState, PortfolioReviewFacts, ReviewStudy, StopUncompared};
use crate::viewmodel::engine::fmt_ud;
use crate::viewmodel::format::{NumberFormat, format_amount, format_scaled};
use crate::wiring::Session;
use crate::wiring::holdings::{HoldingFreshnessMap, concentration_murmur};
use crate::{MainWindow, Review, ReviewDueRow, ReviewPositionRow, ReviewShareRow, Studies};

fn pct(v: Option<Decimal>, format: NumberFormat) -> SharedString {
    v.map(|d| format_scaled(d, DisplayField::Percent, format))
        .unwrap_or_default()
        .into()
}

/// An invested amount in the reference currency. G1 final review: the SAME precision as the
/// Portefeuille screen for the same figure (its consolidation and concentration amounts are
/// `DisplayField::Price`: rounded to at most two decimals, trailing zeros not padded — « 1 234,5 »
/// stays « 1 234,5 », as there) — a total read on one screen and checked on the other must
/// agree digit for digit; the checklist's locale rule (§4) asks for one spelling per figure.
fn amount(v: Option<Decimal>, currency: &str, format: NumberFormat) -> SharedString {
    v.map(|d| {
        format!(
            "{} {currency}",
            format_scaled(d, DisplayField::Price, format)
        )
    })
    .unwrap_or_default()
    .into()
}

/// A config percent (threshold, size target) spelled through the locale path — it renders BESIDE
/// locale-formatted shares (the 6.7 P4 locale rule: never a raw config string in the same
/// sentence as a formatted figure). An unparsable value is shown as stored (the accessor
/// validated it; unreachable in practice).
fn config_pct(raw: &str, format: NumberFormat) -> String {
    Decimal::from_str_exact(raw)
        .map(|d| format_scaled(d, DisplayField::Percent, format))
        .unwrap_or_else(|_| raw.to_string())
}

/// The concentration threshold as a decimal — the config's validated value, else ITS default
/// ([`crate::config::DEFAULT_CONCENTRATION_THRESHOLD_PCT`]), never a literal of this module.
fn threshold_pct(raw: &str) -> Decimal {
    Decimal::from_str_exact(raw)
        .or_else(|_| Decimal::from_str_exact(crate::config::DEFAULT_CONCENTRATION_THRESHOLD_PCT))
        .unwrap_or_default()
}

/// Decision 6 (G1 review) — a sector murmurs only AT or OVER the threshold (no « approaching »
/// band: the spec's sector rule). Currencies never murmur (no call site). The « non renseigné »
/// bucket never murmurs: it is an absence, not a sector (an absent fact never flags — G1
/// on-screen check).
fn sector_murmur(sector: Option<&str>, share: Option<Decimal>, threshold: Decimal) -> bool {
    sector.is_some_and(|s| !s.trim().is_empty())
        && share.is_some_and(|s| s > Decimal::ZERO && s >= threshold)
}

/// The pair(s) a row names when its share could not be stated: its OWN pair(s) when it has any,
/// else the pairs that absent the block's denominator (a share blocked by a missing global pair
/// names it — G1 review). `""` when the share is present or no pair is to blame (then the row
/// reads a plain « indisponible »). Every pair is named, never only the first.
fn blocking_pairs(share_present: bool, own: &[String], global: &[String]) -> String {
    if share_present {
        String::new()
    } else if !own.is_empty() {
        own.join(" · ")
    } else {
        global.join(" · ")
    }
}

/// The union of the rows' own missing pairs (deduplicated, sorted) — the pairs that absent an
/// exposure block's global total (the 6.8 reads carry them per row only).
fn union_pairs<'a>(pairs: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    let mut all: Vec<String> = pairs.into_iter().cloned().collect();
    all.sort();
    all.dedup();
    all
}

/// Stops as data: « 63,00 CHF (UBS) · 50,00 CHF (Swissquote) » — level through the locale path,
/// the stop's own currency, the bank holding the lot (no prose). A legacy lot's stop has no
/// currency to state (G1 final review): « 63,00 (UBS) », never a borrowed one.
fn stops_text<'a>(
    stops: impl Iterator<Item = &'a state::StopFact>,
    format: NumberFormat,
) -> String {
    stops
        .map(|s| {
            let level = format_scaled(s.level, DisplayField::Price, format);
            match &s.currency {
                Some(currency) => format!("{level} {currency} ({})", s.bank),
                None => format!("{level} ({})", s.bank),
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The FR28 footnote: pair, rate, then "(date, source)" — pure data, the rate spelled in the
/// user's number format (G1 final review: never the stored « 0.8 » beside « 1 234,50 »).
fn rates_note(
    rates: &[steadyinvest_persistence::FxRateItem],
    format: NumberFormat,
) -> SharedString {
    rates
        .iter()
        .map(|r| {
            format!(
                "{} → {} {} ({}, {})",
                r.base_currency,
                r.quote_currency,
                format_amount(&r.rate, format),
                r.rate_date,
                r.source
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
        .into()
}

/// The global total's named absence (G1 final review — every cause, never only the pairs):
/// the pair(s) that absent it, and whether ANOTHER cause absents it too (a bank that could not
/// consolidate for a non-pair reason, or — no pair at all to blame — a failed read / overflow).
fn global_absence(
    global_absent: bool,
    missing_pairs: &[String],
    any_bank_unavailable: bool,
) -> (String, bool) {
    if !global_absent {
        return (String::new(), false);
    }
    (
        missing_pairs.join(" · "),
        any_bank_unavailable || missing_pairs.is_empty(),
    )
}

/// The coalescing latch of the async re-push (G3 review: a 40-ticker price batch re-composed
/// the whole review 40 times). The first request of a burst schedules ONE re-push; the ones
/// that land before it fires ride along. Pure — the timer is the caller's.
#[derive(Default)]
pub(crate) struct RepushLatch {
    pending: std::cell::Cell<bool>,
}

impl RepushLatch {
    /// A re-push is wanted: `true` when the caller must schedule it (none is pending yet).
    pub(crate) fn request(&self) -> bool {
        !self.pending.replace(true)
    }

    /// The scheduled re-push runs: the latch opens for the next burst.
    pub(crate) fn fire(&self) {
        self.pending.set(false);
    }
}

thread_local! {
    /// The UI thread's one latch (the review is one surface).
    static REPUSH: RepushLatch = RepushLatch::default();
}

/// Re-push the review when it is the screen on display — the re-render every async mutation
/// owes a shown surface (checklist §7): a price, FX or study result landing while « Revue » is
/// open (G1 final review). Coalesced (G3 review): a burst of results re-pushes ONCE, on the
/// next event-loop turn (a zero-delay single-shot timer). Off-screen, the arrival re-render
/// covers it.
pub(crate) fn request_review_refresh(
    ui: &MainWindow,
    journal_state: &Rc<RefCell<JournalState>>,
    freshness: &Rc<RefCell<HoldingFreshnessMap>>,
    dismissed: &Rc<RefCell<HashSet<String>>>,
    config: &Rc<RefCell<AppConfig>>,
) {
    if ui.get_current_screen() != REVIEW_SCREEN || !REPUSH.with(RepushLatch::request) {
        return;
    }
    let ui_weak = ui.as_weak();
    let journal_state = Rc::clone(journal_state);
    let freshness = Rc::clone(freshness);
    let dismissed = Rc::clone(dismissed);
    let config = Rc::clone(config);
    slint::Timer::single_shot(std::time::Duration::ZERO, move || {
        REPUSH.with(RepushLatch::fire);
        let Some(ui) = ui_weak.upgrade() else { return };
        // Still on display? (the user may have left in the meantime — the arrival re-renders.)
        if ui.get_current_screen() == REVIEW_SCREEN {
            push_review(
                &ui,
                &journal_state.borrow(),
                &freshness.borrow(),
                &dismissed.borrow(),
                &config.borrow(),
            );
        }
    });
}

/// Empty the export-outcome slot — on arrival and at the start of an export (the F4 slot is
/// free again for the next outcome; a stale « exportée » never survives a navigation).
pub(crate) fn clear_notice(ui: &MainWindow) {
    ui.global::<Review>().set_notice(SharedString::new());
}

/// The « Revue » screen's index (`MainWindow.current-screen`).
const REVIEW_SCREEN: i32 = 3;

/// Compose the review facts from the state + the config's size table (the same parse the
/// Portefeuille block uses), or `None` (« indisponible ») on a read failure.
fn facts(state: &JournalState, config: &AppConfig) -> Option<PortfolioReviewFacts> {
    let reference = config.reference_currency_or_default();
    let (small, medium) = config.size_bounds_or_default();
    let dec = |s: &str, default: &str| {
        Decimal::from_str_exact(s)
            .or_else(|_| Decimal::from_str_exact(default))
            .unwrap_or_default()
    };
    state
        .portfolio_review(
            &reference,
            dec(&small, crate::config::DEFAULT_SIZE_SMALL_MAX),
            dec(&medium, crate::config::DEFAULT_SIZE_MEDIUM_MAX),
        )
        .ok()
}

fn rows_model(rows: Vec<ReviewShareRow>) -> ModelRc<ReviewShareRow> {
    ModelRc::new(VecModel::from(rows))
}

/// Push the review into the `Review` global. Freshness comes from the session map (transient,
/// keyed by uppercased ticker — the register's rule); `dismissed` is the register's set of
/// dismissed triggers (holding ids), honoured here too (G1 final review).
pub(crate) fn push_review(
    ui: &MainWindow,
    state: &JournalState,
    freshness: &HoldingFreshnessMap,
    dismissed: &HashSet<String>,
    config: &AppConfig,
) {
    let review = ui.global::<Review>();
    // The export notice is NOT cleared here (G3 review): an async re-push (a price landing)
    // must keep « Revue exportée : chemin ». It is cleared on arrival ([`clear_notice`], the
    // screen-activated arm), at the start of an export, and on a dossier switch.
    let format = config.number_format;
    let Some(f) = facts(state, config) else {
        review.set_unavailable(true);
        return;
    };
    review.set_unavailable(false);
    let reference = f.reference_currency.clone();
    review.set_dossier(
        state
            .path()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_default()
            .into(),
    );
    review.set_date(f.today.clone().into());
    review.set_reference_currency(reference.clone().into());
    review.set_bank_count(f.bank_count as i32);
    review.set_rates(rates_note(&f.rates_used, format));
    let threshold_raw = config.concentration_threshold_pct_or_default();
    let threshold = threshold_pct(&threshold_raw);
    review.set_threshold(config_pct(&threshold_raw, format).into());

    // A share row: `missing` names the pair(s) blocking the figure, `reason` keys an
    // unclassified size row — two fields, never one string to split.
    let share_row = |label: String,
                     amount_v: Option<Decimal>,
                     share: Option<Decimal>,
                     target: String,
                     missing: String,
                     flagged: bool| ReviewShareRow {
        label: label.into(),
        amount: amount(amount_v, &reference, format),
        share: pct(share, format),
        target: target.into(),
        missing: missing.into(),
        reason: SharedString::new(),
        flagged,
    };
    let (t_small, t_medium, t_large) = config.size_targets_or_default();
    let d = &f.diversification;
    // The size shares' denominator is the 6.7 global: an absent share names ITS pairs. Owner
    // decision (Guy, 2026-09-26): the « Montant » is the class's invested capital in the
    // reference currency, as the bank and concentration blocks state theirs.
    let size_row = |key: &str, slot: &state::SizeMixSlot, target: &str| {
        let share = slot.share_pct;
        share_row(
            key.into(),
            slot.invested,
            share,
            config_pct(target, format),
            blocking_pairs(share.is_some(), &[], &d.missing_pairs),
            false,
        )
    };
    // The 6.7 read failed: the size and concentration blocks are « indisponible » — their
    // models are EMPTIED (never stale rows the export would print). A dossier without any
    // position has nothing to classify: one « aucune position classée » statement, never three
    // « indisponible » classes (G1 final review — an empty dossier is not a failed read).
    // Keyed by the review's own positions (G3 review), not by the 6.7 read's row list.
    let size_empty = !d.unavailable && f.positions.is_empty();
    review.set_size_empty(size_empty);
    let size_rows = if d.unavailable || size_empty {
        Vec::new()
    } else {
        vec![
            size_row("small", &d.small, &t_small),
            size_row("medium", &d.medium, &t_medium),
            size_row("large", &d.large, &t_large),
        ]
    };
    review.set_size_rows(rows_model(size_rows));
    let unclassified: Vec<ReviewShareRow> = d
        .unclassified
        .iter()
        .map(|u| {
            let (reason, missing) = match &u.reason {
                state::UnclassifiedReason::NoStudy => ("no_study", String::new()),
                state::UnclassifiedReason::StudyUnavailable => ("study_unavailable", String::new()),
                state::UnclassifiedReason::NoSales => ("no_sales", String::new()),
                state::UnclassifiedReason::Unconvertible => ("unconvertible", String::new()),
                state::UnclassifiedReason::MissingRate(pair) => ("missing_rate", pair.clone()),
            };
            ReviewShareRow {
                reason: reason.into(),
                ..share_row(u.ticker.clone(), None, None, String::new(), missing, false)
            }
        })
        .collect();
    review.set_unclassified_rows(rows_model(unclassified));

    match &f.sectors {
        Some(e) => {
            review.set_sectors_unavailable(false);
            let global = union_pairs(e.rows.iter().flat_map(|r| &r.missing_pairs));
            let rows: Vec<ReviewShareRow> = e
                .rows
                .iter()
                .map(|r| {
                    // Every pair of the sector's own currencies (G1 review — not the first).
                    let own = &r.missing_pairs;
                    share_row(
                        r.sector.clone().unwrap_or_default(),
                        r.amount,
                        r.share_pct,
                        String::new(),
                        blocking_pairs(r.share_pct.is_some(), own, &global),
                        sector_murmur(r.sector.as_deref(), r.share_pct, threshold),
                    )
                })
                .collect();
            review.set_sector_rows(rows_model(rows));
        }
        None => {
            review.set_sectors_unavailable(true);
            review.set_sector_rows(rows_model(Vec::new()));
        }
    }
    match &f.currencies {
        Some(e) => {
            review.set_currencies_unavailable(false);
            let global = union_pairs(e.rows.iter().flat_map(|r| &r.missing_pair));
            let rows: Vec<ReviewShareRow> = e
                .rows
                .iter()
                .map(|r| {
                    let own: Vec<String> = r.missing_pair.iter().cloned().collect();
                    // Decision 6: a currency share never murmurs (an all-CHF dossier is 100 %
                    // CHF by construction — not a concentration fact).
                    share_row(
                        r.currency.clone(),
                        r.amount,
                        r.share_pct,
                        String::new(),
                        blocking_pairs(r.share_pct.is_some(), &own, &global),
                        false,
                    )
                })
                .collect();
            review.set_currency_rows(rows_model(rows));
        }
        None => {
            review.set_currencies_unavailable(true);
            review.set_currency_rows(rows_model(Vec::new()));
        }
    }
    let c = &f.consolidation;
    let global_total = c.global.map(|(_, total)| total);
    let bank_rows: Vec<ReviewShareRow> = c
        .banks
        .iter()
        .map(|b| {
            let total = b.converted.map(|(_, t)| t);
            let share = total.zip(global_total).and_then(|(t, g)| {
                if g > Decimal::ZERO {
                    t.checked_div(g)
                        .and_then(|q| q.checked_mul(Decimal::ONE_HUNDRED))
                } else {
                    None
                }
            });
            // A bank that could not consolidate reads plainly « indisponible » — with its pairs
            // when they are the cause (all of them); never an unclassified-study reason.
            let missing = if total.is_none() {
                b.missing_pairs.join(" · ")
            } else {
                String::new()
            };
            share_row(b.name.clone(), total, share, String::new(), missing, false)
        })
        .collect();
    review.set_bank_rows(rows_model(bank_rows));
    review.set_global_invested(amount(global_total, &reference, format));
    // Owner decision (Guy, 2026-09-26): the global total is 100 % of itself — stated when the
    // total is known and positive (a zero or absent total has no share to state).
    review.set_global_share(pct(
        global_total
            .filter(|g| *g > Decimal::ZERO)
            .map(|_| Decimal::ONE_HUNDRED),
        format,
    ));
    // Every cause of an absent total: its pairs, and — alone or beside them — a non-pair cause
    // (a failed bank read, an overflow), stated plainly.
    let (global_missing, global_other) = global_absence(
        c.global.is_none(),
        &c.missing_pairs,
        c.banks.iter().any(|b| b.unavailable),
    );
    review.set_global_missing(global_missing.into());
    review.set_global_unavailable(global_other);
    review.set_concentration_unavailable(d.unavailable);
    review.set_concentration_missing(if d.global_invested.is_none() && !d.unavailable {
        d.missing_pairs.join(" · ").into()
    } else {
        SharedString::new()
    });
    // The Portefeuille rule: the shares' denominator absent (or not positive) with no pair to
    // name still states itself (« Parts indisponibles. »).
    review.set_concentration_global_absent(
        !d.unavailable
            && d.missing_pairs.is_empty()
            && !d.rows.is_empty()
            && d.global_invested.is_none_or(|g| g <= Decimal::ZERO),
    );
    let conc_rows: Vec<ReviewShareRow> = d
        .rows
        .iter()
        .map(|r| {
            share_row(
                r.ticker.clone(),
                r.invested,
                r.share_pct,
                String::new(),
                r.missing_pairs.join(" · "),
                concentration_murmur(r.share_pct, !r.missing_pairs.is_empty(), threshold),
            )
        })
        .collect();
    review.set_concentration_rows(rows_model(conc_rows));

    let positions: Vec<ReviewPositionRow> = f
        .positions
        .iter()
        .map(|p| {
            let fresh = freshness.get(&p.ticker);
            let mut row = ReviewPositionRow {
                ticker: p.ticker.clone().into(),
                banks: p.banks.join(", ").into(),
                currency: p.currency.clone().into(),
                invested: amount(p.invested, &reference, format),
                share: pct(p.share_pct, format),
                missing: p.missing_pairs.join(" · ").into(),
                stale: fresh.is_some_and(|x| x.stale),
                as_of: fresh
                    .and_then(|x| x.as_of.clone())
                    .unwrap_or_default()
                    .into(),
                // Every lot's stop (every bank), each in its own currency and named by its bank;
                // the breached ones listed apart — which level, which bank (G1 review).
                stop: stops_text(p.stops.iter(), format).into(),
                stop_breached: p.stop_breached,
                // Named apart only when SOME levels are breached — when every level is, the stop
                // line already lists them (no « 80 CHF (UBS) — sous le seuil : 80 CHF (UBS) »).
                stop_breached_levels: if p.stops.iter().all(|s| s.breached) {
                    String::new()
                } else {
                    stops_text(p.stops.iter().filter(|s| s.breached), format)
                }
                .into(),
                // D5: a legacy lot (presumed in the reference currency) whose study is in another
                // currency is not compared: named apart, both facts on the screen (absence honesty).
                stop_no_currency: stops_text(
                    p.stops.iter().filter(|s| {
                        matches!(s.uncompared, Some(StopUncompared::NoCurrency { .. }))
                    }),
                    format,
                )
                .into(),
                // D5: the study currency the legacy lot's stop is not compared against.
                stop_no_currency_study: {
                    let mut currencies: Vec<String> = Vec::new();
                    for s in &p.stops {
                        if let Some(StopUncompared::NoCurrency { study_currency }) = &s.uncompared
                            && !currencies.contains(study_currency)
                        {
                            currencies.push(study_currency.clone());
                        }
                    }
                    currencies.join(" · ").into()
                },
                // …and a lot whose study could not be read (G3 review): its cause named too.
                stop_unreadable: stops_text(
                    p.stops
                        .iter()
                        .filter(|s| s.uncompared == Some(StopUncompared::StudyUnreadable)),
                    format,
                )
                .into(),
                // A trigger the register dismissed stays dismissed here (keyed by holding id).
                trigger: state::position_trigger(&p.lot_triggers, |id| {
                    dismissed.contains(&id.to_string())
                })
                .into(),
                ..Default::default()
            };
            // The lots link to different studies: every fact of the band — the read studies'
            // currencies, a lot without a study, an unreadable one, and what the OTHER studies
            // carry (signals, high zone) so the row hides nothing (G3 review).
            if let Some(m) = &p.mixed {
                row.mixed = true;
                row.mixed_links = m.currencies.join(" · ").into();
                row.mixed_no_study = m.no_study;
                row.mixed_unreadable = m.unreadable;
                row.other_flagged = m
                    .other_flagged
                    .iter()
                    .map(|(currency, n)| format!("{currency} ({n})"))
                    .collect::<Vec<_>>()
                    .join(" · ")
                    .into();
                row.other_high_zone = m.other_high_zone.join(" · ").into();
            }
            match &p.study {
                ReviewStudy::None { other_currency } => {
                    row.study = "none".into();
                    row.other_currency = other_currency.clone().unwrap_or_default().into();
                }
                ReviewStudy::Unavailable => row.study = "unavailable".into(),
                ReviewStudy::NotComputable(s) => {
                    // Still openable — the study screen is where its data gets repaired; its
                    // annual-review clock is known without the engine.
                    row.study = "not_computable".into();
                    row.study_id = s.study_id.to_string().into();
                    row.last_saved = s.last_saved.clone().unwrap_or_default().into();
                    row.last_saved_unknown = s.last_saved.is_none();
                    row.due = s.due_for_review;
                }
                ReviewStudy::Linked(s) => {
                    row.study = s.verdict.into();
                    row.linked = true;
                    row.study_id = s.study_id.to_string().into();
                    row.name = s.company_name.clone().unwrap_or_default().into();
                    row.low_confidence = s.low_confidence;
                    row.zone = s.zone.into();
                    // The study's price in the STUDY's currency — never the position's (a legacy
                    // lot's effective currency is only the reference fallback — G1 final review).
                    row.price = s
                        .current_price
                        .map(|d| {
                            format!(
                                "{} {}",
                                format_scaled(d, DisplayField::Price, format),
                                s.currency
                            )
                        })
                        .unwrap_or_default()
                        .into();
                    row.ud = fmt_ud(&s.upside_downside, format).into();
                    row.relative = s
                        .relative_value_pct
                        .map(|d| format!("{} %", format_scaled(d, DisplayField::Percent, format)))
                        .unwrap_or_default()
                        .into();
                    row.flag_count = s.quality_flags.len() as i32;
                    row.flags = s
                        .quality_flags
                        .iter()
                        .map(|k| state::quality_flag_label(*k))
                        .collect::<Vec<_>>()
                        .join(" · ")
                        .into();
                    row.last_saved = s.last_saved.clone().unwrap_or_default().into();
                    row.last_saved_unknown = s.last_saved.is_none();
                    row.due = s.due_for_review;
                }
            }
            row
        })
        .collect();
    review.set_positions(ModelRc::new(VecModel::from(positions)));
    let due: Vec<ReviewDueRow> = f
        .due
        .iter()
        .map(|d| ReviewDueRow {
            ticker: d.ticker.clone().into(),
            study_id: d.study_id.to_string().into(),
            date: d.last_saved.clone().unwrap_or_default().into(),
            date_unknown: d.last_saved.is_none(),
            age: d.reasons.contains(&"age"),
            age_unknown: d.reasons.contains(&"age_unknown"),
            withheld: d.reasons.contains(&"withheld"),
            low_confidence: d.reasons.contains(&"low_confidence"),
            not_computable: d.reasons.contains(&"not_computable"),
        })
        .collect();
    review.set_due(ModelRc::new(VecModel::from(due)));
    let k = &f.counts;
    review.set_count_positions(k.positions as i32);
    review.set_count_linked(k.linked as i32);
    review.set_count_full(k.full as i32);
    review.set_count_provisional(k.provisional as i32);
    review.set_count_withheld(k.withheld as i32);
    review.set_count_not_computable(k.not_computable as i32);
    review.set_count_flagged(k.flagged as i32);
    review.set_count_high_zone(k.high_zone as i32);
    review.set_count_stop_breached(k.stop_breached as i32);
    review.set_count_due(k.due as i32);
}

/// The report's value, built from the SAME pushed rows (one formatting path, no drift) —
/// including every named absence the screen states (unavailable blocks, missing pairs, the
/// other-currency cause), so the PDF never says less than the screen.
fn report_value(ui: &MainWindow) -> steadyinvest_report::PortfolioReview {
    use steadyinvest_report::{DueLine, PortfolioReview, ReviewLine, ShareLine};
    let r = ui.global::<Review>();
    let shares = |m: ModelRc<ReviewShareRow>| -> Vec<ShareLine> {
        (0..m.row_count())
            .filter_map(|i| m.row_data(i))
            .map(|s| ShareLine {
                label: s.label.to_string(),
                amount: s.amount.to_string(),
                share: s.share.to_string(),
                target: s.target.to_string(),
                missing: s.missing.to_string(),
                reason: s.reason.to_string(),
                flagged: s.flagged,
            })
            .collect()
    };
    let positions = r.get_positions();
    let due = r.get_due();
    PortfolioReview {
        dossier: r.get_dossier().to_string(),
        date: r.get_date().to_string(),
        reference_currency: r.get_reference_currency().to_string(),
        bank_count: r.get_bank_count().to_string(),
        position_count: r.get_count_positions().to_string(),
        linked_count: r.get_count_linked().to_string(),
        rates: r
            .get_rates()
            .split(" · ")
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        size_lines: shares(r.get_size_rows()),
        size_empty: r.get_size_empty(),
        unclassified: shares(r.get_unclassified_rows()),
        diversification_unavailable: r.get_concentration_unavailable(),
        sector_lines: shares(r.get_sector_rows()),
        sectors_unavailable: r.get_sectors_unavailable(),
        currency_lines: shares(r.get_currency_rows()),
        currencies_unavailable: r.get_currencies_unavailable(),
        bank_lines: shares(r.get_bank_rows()),
        global_invested: r.get_global_invested().to_string(),
        global_share: r.get_global_share().to_string(),
        global_missing: r.get_global_missing().to_string(),
        global_unavailable: r.get_global_unavailable(),
        concentration: shares(r.get_concentration_rows()),
        concentration_missing: r.get_concentration_missing().to_string(),
        concentration_global_absent: r.get_concentration_global_absent(),
        concentration_threshold: format!("{} %", r.get_threshold()),
        positions: (0..positions.row_count())
            .filter_map(|i| positions.row_data(i))
            .map(|p| ReviewLine {
                ticker: p.ticker.to_string(),
                name: p.name.to_string(),
                banks: p.banks.to_string(),
                invested: p.invested.to_string(),
                share: p.share.to_string(),
                currency: p.currency.to_string(),
                missing: p.missing.to_string(),
                study: p.study.to_string(),
                other_currency: p.other_currency.to_string(),
                low_confidence: p.low_confidence,
                zone: p.zone.to_string(),
                ud: p.ud.to_string(),
                relative: p.relative.to_string(),
                flags: p.flags.to_string(),
                flag_count: usize::try_from(p.flag_count).unwrap_or_default(),
                data_state: if p.stale {
                    "stale".into()
                } else if !p.as_of.is_empty() {
                    "fresh".into()
                } else {
                    String::new()
                },
                as_of: p.as_of.to_string(),
                price: p.price.to_string(),
                last_saved: p.last_saved.to_string(),
                last_saved_unknown: p.last_saved_unknown,
                mixed: p.mixed,
                mixed_links: p.mixed_links.to_string(),
                mixed_no_study: p.mixed_no_study,
                mixed_unreadable: p.mixed_unreadable,
                other_flagged: p.other_flagged.to_string(),
                other_high_zone: p.other_high_zone.to_string(),
                stop: p.stop.to_string(),
                stop_breached: p.stop_breached,
                stop_breached_levels: p.stop_breached_levels.to_string(),
                stop_no_currency: p.stop_no_currency.to_string(),
                stop_no_currency_study: p.stop_no_currency_study.to_string(),
                stop_unreadable: p.stop_unreadable.to_string(),
                trigger: p.trigger.to_string(),
            })
            .collect(),
        due: (0..due.row_count())
            .filter_map(|i| due.row_data(i))
            .map(|d| DueLine {
                ticker: d.ticker.to_string(),
                date: d.date.to_string(),
                date_unknown: d.date_unknown,
                reasons: [
                    (d.age, "age"),
                    (d.age_unknown, "age_unknown"),
                    (d.withheld, "withheld"),
                    (d.low_confidence, "low_confidence"),
                    (d.not_computable, "not_computable"),
                ]
                .into_iter()
                .filter(|(on, _)| *on)
                .map(|(_, key)| key.to_string())
                .collect(),
            })
            .collect(),
        counts: vec![
            ("positions".into(), r.get_count_positions().to_string()),
            ("linked".into(), r.get_count_linked().to_string()),
            ("full".into(), r.get_count_full().to_string()),
            ("provisional".into(), r.get_count_provisional().to_string()),
            ("withheld".into(), r.get_count_withheld().to_string()),
            (
                "not_computable".into(),
                r.get_count_not_computable().to_string(),
            ),
            ("flagged".into(), r.get_count_flagged().to_string()),
            ("high_zone".into(), r.get_count_high_zone().to_string()),
            (
                "stop_breached".into(),
                r.get_count_stop_breached().to_string(),
            ),
            ("due".into(), r.get_count_due().to_string()),
        ],
    }
}

pub(crate) fn wire_review(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        quick_screen,
        config,
        holding_freshness,
        holding_dismissed,
        ..
    } = s;
    {
        // « Exporter PDF » — re-push first, then render from the pushed rows (one formatting
        // path): the PDF states the dossier as it is NOW, never rows an async price / FX result
        // has since outdated (G1 final review). Native save picker, outcome in the slot.
        let ui_weak = ui.as_weak();
        let journal_state = std::rc::Rc::clone(journal_state);
        let config = std::rc::Rc::clone(config);
        let holding_freshness = std::rc::Rc::clone(holding_freshness);
        let holding_dismissed = std::rc::Rc::clone(holding_dismissed);
        ui.global::<Review>().on_export_pdf(move || {
            let ui = ui_weak.unwrap();
            // A new export: the previous outcome leaves the slot (G3 review — the re-push no
            // longer clears it).
            clear_notice(&ui);
            push_review(
                &ui,
                &journal_state.borrow(),
                &holding_freshness.borrow(),
                &holding_dismissed.borrow(),
                &config.borrow(),
            );
            if ui.global::<Review>().get_unavailable() {
                // The dossier could not be read just now: the screen says so; nothing to export.
                return;
            }
            let value = report_value(&ui);
            let bytes = steadyinvest_report::render_portfolio_review(&value);
            let mut dialog = rfd::FileDialog::new()
                .set_title("Exporter la revue en PDF")
                .add_filter("PDF", &["pdf"])
                .set_file_name(format!("revue-{}.pdf", value.date));
            if let Some(dir) = journal_state
                .borrow()
                .backups_dir()
                .and_then(|d| d.parent().map(|p| p.to_path_buf()))
                .filter(|d| d.is_dir())
            {
                dialog = dialog.set_directory(dir);
            }
            let Some(path) = dialog.save_file() else {
                return;
            };
            let review = ui.global::<Review>();
            match std::fs::write(&path, bytes) {
                Ok(()) => review.set_notice(
                    format!("{} {}", state::MSG_REVIEW_EXPORTED, path.display()).into(),
                ),
                Err(e) => {
                    crate::wiring::dialog::refuse(&ui, &format!("{} {e}", state::MSG_SAVE_FAILED))
                }
            }
        });
    }
    {
        // « Ouvrir l'étude » — the one open rail (Études + invoke_open_study), with the arrival
        // re-render every navigation owes its destination (#94): Études re-derives its list
        // before the study opens over it. G1 G review: a comparison or an examination left open
        // over Études would hide the study — they close first, through their own close paths.
        let ui_weak = ui.as_weak();
        let journal_state = std::rc::Rc::clone(journal_state);
        let quick_screen = std::rc::Rc::clone(quick_screen);
        ui.global::<Review>().on_open_study(move |id| {
            let ui = ui_weak.unwrap();
            if Uuid::parse_str(&id).is_ok() {
                crate::wiring::close_studies_overlays(&ui, &journal_state.borrow(), &quick_screen);
                ui.set_current_screen(0);
                ui.invoke_screen_activated(0);
                ui.global::<Studies>().invoke_open_study(id);
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        ui.global::<Review>().on_go_to_portfolio(move || {
            let ui = ui_weak.unwrap();
            ui.set_current_screen(2);
            ui.invoke_screen_activated(2);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    #[test]
    fn the_concentration_murmur_is_portefeuilles_core_band() {
        // Decision 6: identical to Portefeuille BY CONSTRUCTION — both surfaces call the one
        // `wiring::holdings::concentration_murmur` (the core band from 10 points below the
        // threshold, on a present positive share no missing pair blocks); the zero case below
        // holds on Portefeuille too (its `share > 0` guard, G1 review).
        let t = d("50");
        for share in ["39.9", "40", "45", "50", "65.9"] {
            assert_eq!(
                concentration_murmur(Some(d(share)), false, t),
                steadyinvest_core::risk::concentration_flagged(d(share), t),
                "parity with Portefeuille at {share} %"
            );
        }
        assert!(
            !concentration_murmur(Some(d("40")), true, t),
            "a blocked row"
        );
        assert!(!concentration_murmur(None, false, t), "an absent share");
        assert!(
            !concentration_murmur(Some(Decimal::ZERO), false, d("5")),
            "a zero share never murmurs, even under a ≤ 10 threshold"
        );
    }

    #[test]
    fn a_sector_murmurs_only_at_or_over_the_threshold() {
        let t = d("50");
        let tech = Some("Technology");
        assert!(
            !sector_murmur(tech, Some(d("45")), t),
            "no approaching band"
        );
        assert!(sector_murmur(tech, Some(d("50")), t));
        assert!(sector_murmur(tech, Some(d("100")), t));
        assert!(!sector_murmur(tech, None, t));
        // « non renseigné » is an absence, never a murmur.
        assert!(!sector_murmur(None, Some(d("60")), t));
        assert!(!sector_murmur(Some("  "), Some(d("60")), t));
    }

    #[test]
    fn an_unparsable_threshold_falls_back_through_the_config_default() {
        assert_eq!(threshold_pct("40"), d("40"));
        assert_eq!(
            threshold_pct("n/a"),
            d(crate::config::DEFAULT_CONCENTRATION_THRESHOLD_PCT)
        );
    }

    #[test]
    fn a_blocked_share_names_every_pair_its_own_first_else_the_globals() {
        let own = vec!["EUR → CHF".to_string(), "USD → CHF".to_string()];
        let global = vec!["GBP → CHF".to_string()];
        assert_eq!(
            blocking_pairs(false, &own, &global),
            "EUR → CHF · USD → CHF"
        );
        assert_eq!(
            blocking_pairs(false, &[], &global),
            "GBP → CHF",
            "a share blocked by the denominator names the denominator's pair"
        );
        assert_eq!(blocking_pairs(true, &own, &global), "", "a present share");
        assert_eq!(
            blocking_pairs(false, &[], &[]),
            "",
            "plain « indisponible »"
        );
        let a = "EUR → CHF".to_string();
        let b = "USD → CHF".to_string();
        assert_eq!(union_pairs([&b, &a, &b]), vec![a.clone(), b.clone()]);
    }

    #[test]
    fn a_burst_of_async_results_schedules_one_re_push() {
        let latch = RepushLatch::default();
        assert!(latch.request(), "the first result of a burst schedules");
        assert!(!latch.request(), "the next ones ride along");
        assert!(!latch.request());
        latch.fire();
        assert!(
            latch.request(),
            "after the re-push, a new burst schedules again"
        );
    }

    #[test]
    fn the_global_totals_absence_names_every_cause() {
        let pairs = vec!["EUR → CHF".to_string()];
        assert_eq!(global_absence(false, &pairs, true), (String::new(), false));
        // Pairs alone.
        assert_eq!(
            global_absence(true, &pairs, false),
            ("EUR → CHF".to_string(), false)
        );
        // Pairs AND a bank that could not consolidate for another reason — both named.
        assert_eq!(
            global_absence(true, &pairs, true),
            ("EUR → CHF".to_string(), true)
        );
        // No pair to blame: a plain « indisponible ».
        assert_eq!(global_absence(true, &[], false), (String::new(), true));
    }

    #[test]
    fn amounts_rates_and_stops_are_spelled_as_elsewhere() {
        let comma = NumberFormat::Comma;
        // Portefeuille's precision for an invested amount: up to two decimals (never rounded to the unit).
        assert_eq!(
            amount(Some(d("1234.5")), "CHF", comma).as_str(),
            format!(
                "{} CHF",
                format_scaled(d("1234.5"), DisplayField::Price, comma)
            )
        );
        assert!(
            amount(Some(d("1234.56")), "CHF", comma).contains(",56"),
            "the cents are no longer rounded away"
        );
        // The FX footnote's rate in the user's number format.
        let rate = steadyinvest_persistence::FxRateItem {
            rate: "0.8".into(),
            ..fx_item()
        };
        assert!(rates_note(&[rate], comma).contains(" 0,8 ("));
        // A legacy lot's stop is not given a currency it does not carry.
        let stop = |currency: Option<&str>| state::StopFact {
            level: d("63"),
            currency: currency.map(str::to_string),
            bank: "UBS".into(),
            breached: false,
            uncompared: None,
        };
        assert_eq!(
            stops_text([stop(None), stop(Some("CHF"))].iter(), comma),
            "63 (UBS) · 63 CHF (UBS)"
        );
    }

    fn fx_item() -> steadyinvest_persistence::FxRateItem {
        steadyinvest_persistence::FxRateItem {
            id: Uuid::nil(),
            base_currency: "USD".into(),
            quote_currency: "CHF".into(),
            rate: "1".into(),
            rate_date: "2026-09-23".into(),
            source: "manual".into(),
            created_at: steadyinvest_contract::Timestamp(String::new()),
        }
    }

    #[test]
    fn config_percents_go_through_the_locale_path() {
        // The comma locale spells the config's canonical "12.5" with its own separator — the
        // same spelling as the shares beside it.
        let comma = NumberFormat::Comma;
        assert_eq!(
            config_pct("12.5", comma),
            format_scaled(d("12.5"), DisplayField::Percent, comma)
        );
        assert!(!config_pct("12.5", comma).contains('.'));
        assert_eq!(config_pct("oops", comma), "oops");
    }
}
