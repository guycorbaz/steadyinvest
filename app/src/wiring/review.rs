//! Story 7.2 — the « Revue » screen: push the composed state read into the `Review` global on
//! activation (issue #94 — never stale on arrival), and the FR53 export through the native `rfd`
//! save picker (the 5.6 rail). Formatting happens HERE (the one float→string boundary of the app);
//! the state read carries engine values, the report carries its own labels.

use std::cell::RefCell;
use std::rc::Rc;

use rust_decimal::Decimal;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use steadyinvest_core::rounding::DisplayField;
use steadyinvest_core::ssg::UpsideDownside;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::state::{self, JournalState, PortfolioReviewFacts, ReviewStudy};
use crate::viewmodel::format::{NumberFormat, format_scaled};
use crate::wiring::Session;
use crate::wiring::holdings::HoldingFreshnessMap;
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

fn ud(u: &UpsideDownside, format: NumberFormat) -> SharedString {
    match u {
        UpsideDownside::Ratio(d) => format!("{}:1", format_scaled(*d, DisplayField::Ratio, format)),
        _ => String::new(),
    }
    .into()
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

/// Push the review into the `Review` global. Freshness comes from the session map (transient,
/// keyed by uppercased ticker — the register's rule).
pub(crate) fn push_review(
    ui: &MainWindow,
    state: &JournalState,
    freshness: &HoldingFreshnessMap,
    config: &AppConfig,
) {
    let review = ui.global::<Review>();
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
    review.set_threshold(config.concentration_threshold_pct_or_default().into());

    // `note` is "" / "missing_rate:<pair>" / a reason key — split into the row's two fields.
    let share_row = |label: String,
                     amount_v: Option<Decimal>,
                     share: Option<Decimal>,
                     target: &str,
                     note: String,
                     flagged: bool| {
        let (missing, reason) = match note.split_once(':') {
            Some(("missing_rate", pair)) => (pair.to_string(), String::new()),
            _ => (String::new(), note.clone()),
        };
        ReviewShareRow {
            label: label.into(),
            amount: amount(amount_v, &reference, format),
            share: pct(share, format),
            target: target.into(),
            missing: missing.into(),
            reason: reason.into(),
            flagged,
        }
    };
    let (t_small, t_medium, t_large) = config.size_targets_or_default();
    let d = &f.diversification;
    let size_rows = vec![
        share_row(
            "small".into(),
            None,
            d.small.share_pct,
            &t_small,
            String::new(),
            false,
        ),
        share_row(
            "medium".into(),
            None,
            d.medium.share_pct,
            &t_medium,
            String::new(),
            false,
        ),
        share_row(
            "large".into(),
            None,
            d.large.share_pct,
            &t_large,
            String::new(),
            false,
        ),
    ];
    review.set_size_rows(ModelRc::new(VecModel::from(size_rows)));
    let unclassified: Vec<ReviewShareRow> = d
        .unclassified
        .iter()
        .map(|u| {
            let note = match &u.reason {
                state::UnclassifiedReason::NoStudy => "no_study".to_string(),
                state::UnclassifiedReason::StudyUnavailable => "study_unavailable".to_string(),
                state::UnclassifiedReason::NoSales => "no_sales".to_string(),
                state::UnclassifiedReason::Unconvertible => "unconvertible".to_string(),
                state::UnclassifiedReason::MissingRate(pair) => format!("missing_rate:{pair}"),
            };
            let mut row = share_row(u.ticker.clone(), None, None, "", note, false);
            if !row.missing.is_empty() {
                row.reason = "missing_rate".into(); // a reason-only row that also names its pair
            }
            row
        })
        .collect();
    review.set_unclassified_rows(ModelRc::new(VecModel::from(unclassified)));
    let threshold = Decimal::from_str_exact(&config.concentration_threshold_pct_or_default())
        .unwrap_or(Decimal::from(50));
    let flagged = |share: Option<Decimal>| {
        share.is_some_and(|s| s > Decimal::ZERO && s >= threshold - Decimal::from(5))
    };

    match &f.sectors {
        Some(e) => {
            review.set_sectors_unavailable(false);
            let rows: Vec<ReviewShareRow> = e
                .rows
                .iter()
                .map(|r| {
                    share_row(
                        r.sector.clone().unwrap_or_default(),
                        None,
                        r.share_pct,
                        "",
                        r.missing_pair
                            .as_ref()
                            .map(|p| format!("missing_rate:{p}"))
                            .unwrap_or_default(),
                        flagged(r.share_pct),
                    )
                })
                .collect();
            review.set_sector_rows(ModelRc::new(VecModel::from(rows)));
        }
        None => review.set_sectors_unavailable(true),
    }
    match &f.currencies {
        Some(e) => {
            review.set_currencies_unavailable(false);
            let rows: Vec<ReviewShareRow> = e
                .rows
                .iter()
                .map(|r| {
                    share_row(
                        r.currency.clone(),
                        None,
                        r.share_pct,
                        "",
                        r.missing_pair
                            .as_ref()
                            .map(|p| format!("missing_rate:{p}"))
                            .unwrap_or_default(),
                        flagged(r.share_pct),
                    )
                })
                .collect();
            review.set_currency_rows(ModelRc::new(VecModel::from(rows)));
        }
        None => review.set_currencies_unavailable(true),
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
            let note = b
                .missing_pairs
                .first()
                .map(|p| format!("missing_rate:{p}"))
                .unwrap_or_else(|| {
                    if b.unavailable {
                        "study_unavailable".to_string()
                    } else {
                        String::new()
                    }
                });
            share_row(b.name.clone(), total, share, "", note, false)
        })
        .collect();
    review.set_bank_rows(ModelRc::new(VecModel::from(bank_rows)));
    review.set_global_invested(amount(global_total, &reference, format));
    review.set_global_missing(if c.global.is_none() {
        c.missing_pairs.join(" · ").into()
    } else {
        SharedString::new()
    });
    review.set_concentration_unavailable(d.unavailable);
    review.set_concentration_missing(if d.global_invested.is_none() && !d.unavailable {
        d.missing_pairs.join(" · ").into()
    } else {
        SharedString::new()
    });
    let conc_rows: Vec<ReviewShareRow> = d
        .rows
        .iter()
        .map(|r| {
            share_row(
                r.ticker.clone(),
                r.invested,
                r.share_pct,
                "",
                r.missing_pairs
                    .first()
                    .map(|p| format!("missing_rate:{p}"))
                    .unwrap_or_default(),
                flagged(r.share_pct),
            )
        })
        .collect();
    review.set_concentration_rows(ModelRc::new(VecModel::from(conc_rows)));

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
                stop: p
                    .stop_level
                    .map(|l| format_scaled(l, DisplayField::Price, format))
                    .unwrap_or_default()
                    .into(),
                stop_breached: p.stop_breached,
                trigger: p.trigger.into(),
                ..Default::default()
            };
            match &p.study {
                ReviewStudy::None { other_currency } => {
                    row.study = "none".into();
                    row.other_currency = other_currency.clone().unwrap_or_default().into();
                }
                ReviewStudy::Unavailable => row.study = "unavailable".into(),
                ReviewStudy::Linked(s) => {
                    row.study = s.verdict.into();
                    row.study_id = s.study_id.to_string().into();
                    row.name = s.company_name.clone().unwrap_or_default().into();
                    row.low_confidence = s.low_confidence;
                    row.zone = s.zone.into();
                    row.price = s
                        .current_price
                        .map(|d| format_scaled(d, DisplayField::Price, format))
                        .unwrap_or_default()
                        .into();
                    row.ud = ud(&s.upside_downside, format);
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
                    row.last_saved = s.last_saved.clone().into();
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
            date: d.last_saved.clone().into(),
            reason: d.reason.into(),
        })
        .collect();
    review.set_due(ModelRc::new(VecModel::from(due)));
    let k = &f.counts;
    review.set_count_positions(k.positions as i32);
    review.set_count_linked(k.linked as i32);
    review.set_count_full(k.full as i32);
    review.set_count_provisional(k.provisional as i32);
    review.set_count_withheld(k.withheld as i32);
    review.set_count_flagged(k.flagged as i32);
    review.set_count_high_zone(k.high_zone as i32);
    review.set_count_stop_breached(k.stop_breached as i32);
    review.set_count_due(k.due as i32);
}

/// The report's value, built from the SAME pushed rows (one formatting path, no drift).
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
                // The report's one-string note convention: a missing pair carries its prefix.
                note: if !s.missing.is_empty() {
                    format!("missing_rate:{}", s.missing)
                } else {
                    s.reason.to_string()
                },
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
        sector_lines: shares(r.get_sector_rows()),
        currency_lines: shares(r.get_currency_rows()),
        bank_lines: shares(r.get_bank_rows()),
        global_invested: r.get_global_invested().to_string(),
        concentration: shares(r.get_concentration_rows()),
        concentration_threshold: format!("{} %", r.get_threshold()),
        positions: (0..positions.row_count())
            .filter_map(|i| positions.row_data(i))
            .map(|p| ReviewLine {
                ticker: p.ticker.to_string(),
                name: p.name.to_string(),
                banks: p.banks.to_string(),
                invested: p.invested.to_string(),
                share: p.share.to_string(),
                study: p.study.to_string(),
                low_confidence: p.low_confidence,
                zone: p.zone.to_string(),
                ud: p.ud.to_string(),
                relative: p.relative.to_string(),
                flags: p.flags.to_string(),
                data_state: if p.stale {
                    "stale".into()
                } else if !p.as_of.is_empty() {
                    "fresh".into()
                } else {
                    String::new()
                },
                as_of: p.as_of.to_string(),
                stop: if p.stop.is_empty() {
                    String::new()
                } else {
                    format!("{} {}", p.stop, p.currency)
                },
                stop_breached: p.stop_breached,
                trigger: p.trigger.to_string(),
            })
            .collect(),
        due: (0..due.row_count())
            .filter_map(|i| due.row_data(i))
            .map(|d| DueLine {
                ticker: d.ticker.to_string(),
                date: d.date.to_string(),
                reason: d.reason.to_string(),
            })
            .collect(),
        counts: vec![
            ("positions".into(), r.get_count_positions().to_string()),
            ("linked".into(), r.get_count_linked().to_string()),
            ("full".into(), r.get_count_full().to_string()),
            ("provisional".into(), r.get_count_provisional().to_string()),
            ("withheld".into(), r.get_count_withheld().to_string()),
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
        config,
        holding_freshness,
        quick_screen,
        ..
    } = s;
    {
        // « Exporter PDF » — render from the pushed rows, native save picker, outcome in the slot.
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
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
        // « Ouvrir l'étude » — the one open rail (Études + invoke_open_study).
        // G1 G review: a comparison or an examination left open over Études would hide the study
        // — they close first, through their own close paths.
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let quick_screen = Rc::clone(quick_screen);
        ui.global::<Review>().on_open_study(move |id| {
            let ui = ui_weak.unwrap();
            if Uuid::parse_str(&id).is_ok() {
                crate::wiring::close_studies_overlays(&ui, &journal_state.borrow(), &quick_screen);
                ui.set_current_screen(0);
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
    let _ = (
        RefCell::new(()),
        Rc::clone(config),
        Rc::clone(holding_freshness),
    );
}
