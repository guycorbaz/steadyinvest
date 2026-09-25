//! Story 7.2 — the « Revue » screen: push the composed state read into the `Review` global on
//! activation (issue #94 — never stale on arrival), and the FR53 export through the native `rfd`
//! save picker (the 5.6 rail). Formatting happens HERE (the one float→string boundary of the app);
//! the state read carries engine values, the report carries its own labels.

use rust_decimal::Decimal;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use steadyinvest_core::rounding::DisplayField;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::state::{self, JournalState, PortfolioReviewFacts, ReviewStudy};
use crate::viewmodel::engine::fmt_ud;
use crate::viewmodel::format::{NumberFormat, format_scaled};
use crate::wiring::Session;
use crate::wiring::holdings::{HoldingFreshnessMap, concentration_murmur};
use crate::{MainWindow, Review, ReviewDueRow, ReviewPositionRow, ReviewShareRow, Studies};

fn pct(v: Option<Decimal>, format: NumberFormat) -> SharedString {
    v.map(|d| format_scaled(d, DisplayField::Percent, format))
        .unwrap_or_default()
        .into()
}

fn amount(v: Option<Decimal>, currency: &str, format: NumberFormat) -> SharedString {
    v.map(|d| {
        format!(
            "{} {currency}",
            format_scaled(d, DisplayField::LargeMonetary, format)
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
/// band: the spec's sector rule). Currencies never murmur (no call site).
fn sector_murmur(share: Option<Decimal>, threshold: Decimal) -> bool {
    share.is_some_and(|s| s > Decimal::ZERO && s >= threshold)
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
/// the stop's own currency, the bank holding the lot (no prose).
fn stops_text<'a>(
    stops: impl Iterator<Item = &'a state::StopFact>,
    format: NumberFormat,
) -> String {
    stops
        .map(|s| {
            format!(
                "{} {} ({})",
                format_scaled(s.level, DisplayField::Price, format),
                s.currency,
                s.bank
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn rates_note(rates: &[steadyinvest_persistence::FxRateItem]) -> SharedString {
    rates
        .iter()
        .map(|r| {
            format!(
                "{} → {} {} ({}, {})",
                r.base_currency, r.quote_currency, r.rate, r.rate_date, r.source
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
        .into()
}

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
/// keyed by uppercased ticker — the register's rule).
pub(crate) fn push_review(
    ui: &MainWindow,
    state: &JournalState,
    freshness: &HoldingFreshnessMap,
    config: &AppConfig,
) {
    let review = ui.global::<Review>();
    // The export outcome belongs to the render it reported on: every (re)push clears it — a
    // dossier switch or a later arrival never shows a stale « exportée » (the F4 slot is empty
    // again for the next outcome).
    review.set_notice(SharedString::new());
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
    review.set_rates(rates_note(&f.rates_used));
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
    // The size shares' denominator is the 6.7 global: an absent share names ITS pairs.
    let size_row = |key: &str, share: Option<Decimal>, target: &str| {
        share_row(
            key.into(),
            None,
            share,
            config_pct(target, format),
            blocking_pairs(share.is_some(), &[], &d.missing_pairs),
            false,
        )
    };
    // The 6.7 read failed: the size and concentration blocks are « indisponible » — their
    // models are EMPTIED (never stale rows the export would print).
    let size_rows = if d.unavailable {
        Vec::new()
    } else {
        vec![
            size_row("small", d.small.share_pct, &t_small),
            size_row("medium", d.medium.share_pct, &t_medium),
            size_row("large", d.large.share_pct, &t_large),
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
                        None,
                        r.share_pct,
                        String::new(),
                        blocking_pairs(r.share_pct.is_some(), own, &global),
                        sector_murmur(r.share_pct, threshold),
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
                        None,
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
    review.set_global_missing(if c.global.is_none() {
        c.missing_pairs.join(" · ").into()
    } else {
        SharedString::new()
    });
    // Absent without a nameable pair (a failed bank read, an overflow): stated plainly.
    review.set_global_unavailable(c.global.is_none() && c.missing_pairs.is_empty());
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
                stop_breached_levels: stops_text(p.stops.iter().filter(|s| s.breached), format)
                    .into(),
                trigger: p.trigger.into(),
                mixed_links: p.mixed_links.join(" · ").into(),
                ..Default::default()
            };
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
                    row.price = s
                        .current_price
                        .map(|d| {
                            format!(
                                "{} {}",
                                format_scaled(d, DisplayField::Price, format),
                                p.currency
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
        unclassified: shares(r.get_unclassified_rows()),
        diversification_unavailable: r.get_concentration_unavailable(),
        sector_lines: shares(r.get_sector_rows()),
        sectors_unavailable: r.get_sectors_unavailable(),
        currency_lines: shares(r.get_currency_rows()),
        currencies_unavailable: r.get_currencies_unavailable(),
        bank_lines: shares(r.get_bank_rows()),
        global_invested: r.get_global_invested().to_string(),
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
                mixed_links: p.mixed_links.to_string(),
                stop: p.stop.to_string(),
                stop_breached: p.stop_breached,
                stop_breached_levels: p.stop_breached_levels.to_string(),
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
    let Session { journal_state, .. } = s;
    {
        // « Exporter PDF » — render from the pushed rows, native save picker, outcome in the slot.
        let ui_weak = ui.as_weak();
        let journal_state = std::rc::Rc::clone(journal_state);
        ui.global::<Review>().on_export_pdf(move || {
            let ui = ui_weak.unwrap();
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
        // before the study opens over it.
        let ui_weak = ui.as_weak();
        ui.global::<Review>().on_open_study(move |id| {
            let ui = ui_weak.unwrap();
            if Uuid::parse_str(&id).is_ok() {
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
        assert!(!sector_murmur(Some(d("45")), t), "no approaching band");
        assert!(sector_murmur(Some(d("50")), t));
        assert!(sector_murmur(Some(d("100")), t));
        assert!(!sector_murmur(None, t));
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
