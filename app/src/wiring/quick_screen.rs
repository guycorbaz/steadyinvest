//! Story 7.3 — the « Examen rapide » rail: a fetch for a ticker with no study (kept in the
//! session, nothing written), or the open study's own years; the screen's figures pushed once;
//! the reader's objective re-words the two rate conclusions; « Exporter PDF » through the native
//! picker; « Créer l'étude » writes the study from the SAME fetched financials (no second fetch).

use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use steadyinvest_core::checklist::{QuickScreenOutputs, quick_screen};
use steadyinvest_core::normalize::CanonicalYear;
use steadyinvest_ingestion::FetchedFinancials;

use crate::provider::ProviderChoice;
use crate::state::{self, JournalState};
use crate::viewmodel::engine::build_frame;
use crate::viewmodel::format::NumberFormat;
use crate::viewmodel::quick_screen::{QuickScreenHeader, meets_key, quick_screen_view};
use crate::wiring::Session;
use crate::wiring::fetch::resolve_chain;
use crate::wiring::studies::refresh_studies;
use crate::{MainWindow, QuickPriceRow, QuickScreen, Studies};

/// The examination of the moment (session only).
pub(crate) struct QuickScreenSession {
    pub(crate) ticker: String,
    pub(crate) currency: String,
    pub(crate) name: String,
    pub(crate) source: String,
    pub(crate) from_study: bool,
    pub(crate) outputs: QuickScreenOutputs,
    /// The financials a fetch brought — « Créer l'étude » reuses them; `None` from a study.
    pub(crate) fetched: Option<FetchedFinancials>,
}

fn strings(items: &[String]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        items
            .iter()
            .map(|s| SharedString::from(s.as_str()))
            .collect::<Vec<_>>(),
    ))
}

/// Push the session's examination into the `QuickScreen` global (figures + keys; the reader's
/// fields are untouched — they are the reader's).
fn push(ui: &MainWindow, session: &QuickScreenSession, today: &str, format: NumberFormat) {
    let view = quick_screen_view(
        QuickScreenHeader {
            ticker: session.ticker.clone(),
            name: session.name.clone(),
            currency: session.currency.clone(),
            date: today.to_string(),
            source: session.source.clone(),
        },
        &session.outputs,
        format,
    );
    let q = ui.global::<QuickScreen>();
    q.set_ticker(view.ticker.into());
    q.set_name(view.name.into());
    q.set_currency(view.currency.into());
    q.set_date(view.date.into());
    q.set_source(view.source.into());
    q.set_from_study(session.from_study);
    q.set_sales_lines(strings(&view.sales.lines));
    q.set_sales_years(strings(&view.sales.years));
    q.set_sales_rate(view.sales.rate.into());
    q.set_sales_span(view.sales.span_years.into());
    q.set_sales_unavailable(view.sales.unavailable);
    q.set_eps_lines(strings(&view.eps.lines));
    q.set_eps_years(strings(&view.eps.years));
    q.set_eps_rate(view.eps.rate.into());
    q.set_eps_span(view.eps.span_years.into());
    q.set_eps_unavailable(view.eps.unavailable);
    q.set_eps_vs_sales(view.eps_vs_sales.into());
    let rows: Vec<QuickPriceRow> = view
        .price_rows
        .iter()
        .map(|r| QuickPriceRow {
            year: r.year.as_str().into(),
            high: r.high.as_str().into(),
            low: r.low.as_str().into(),
            eps: r.eps.as_str().into(),
            pe_high: r.pe_high.as_str().into(),
            pe_low: r.pe_low.as_str().into(),
        })
        .collect();
    q.set_price_rows(ModelRc::new(VecModel::from(rows)));
    q.set_pe_high_total(view.pe_high_total.into());
    q.set_pe_low_total(view.pe_low_total.into());
    q.set_pe_high_avg(view.pe_high_avg.into());
    q.set_pe_low_avg(view.pe_low_avg.into());
    q.set_pe_avg_of_avgs(view.pe_avg_of_avgs.into());
    q.set_present_price(view.present_price.into());
    q.set_present_eps(view.present_eps.into());
    q.set_present_pe(view.present_pe.into());
    q.set_high_five_years_ago(view.high_five_years_ago.into());
    q.set_price_vs_high_pct(view.price_vs_high_pct.into());
    q.set_years_sold_as_high(view.years_sold_as_high.into());
    q.set_pe_position(view.pe_position.into());
    let objective = q.get_objective().to_string();
    q.set_sales_meets(
        meets_key(session.outputs.sales.compound_rate_pct, &objective, format).into(),
    );
    q.set_eps_meets(meets_key(session.outputs.eps.compound_rate_pct, &objective, format).into());
    q.set_notice(SharedString::new());
}

/// The examination from a canonical series + the present facts.
fn examine(
    years: &[CanonicalYear],
    present_price: Option<rust_decimal::Decimal>,
    ttm_eps: Option<rust_decimal::Decimal>,
) -> QuickScreenOutputs {
    // The present EPS: the TTM figure when known, else the latest annual EPS.
    let present_eps = ttm_eps.or_else(|| years.iter().rev().find_map(|y| y.eps));
    quick_screen(years, present_price, present_eps)
}

fn today(state: &JournalState) -> String {
    state.now().0.chars().take(10).collect()
}

/// The worker's examination result (called from the fetch outcome handler).
pub(crate) fn on_fetched(
    ui: &MainWindow,
    state: &JournalState,
    format: NumberFormat,
    slot: &Rc<std::cell::RefCell<Option<QuickScreenSession>>>,
    ticker: String,
    result: Result<FetchedFinancials, steadyinvest_ingestion::IngestionError>,
    effective: ProviderChoice,
) {
    let q = ui.global::<QuickScreen>();
    q.set_fetching(false);
    match result {
        Ok(fetched) if fetched.canonical.years.is_empty() => {
            crate::wiring::dialog::refuse(ui, state::MSG_PROVIDER_NO_DATA);
        }
        Ok(fetched) => {
            let currency = q.get_pick_currency().trim().to_uppercase();
            // Issue #109's rule, as the study apply path: a year without `sales` is the provider's
            // price-only row for the fiscal year in progress — not an analysis year (its EPS would
            // read 0 and its high would count as « sold as high »). Same window, no drift.
            let years: Vec<CanonicalYear> = fetched
                .canonical
                .years
                .iter()
                .filter(|y| y.sales.is_some())
                .cloned()
                .collect();
            let outputs = examine(&years, fetched.latest_price, fetched.ttm_eps);
            let session = QuickScreenSession {
                ticker: ticker.to_uppercase(),
                currency,
                name: String::new(),
                source: state::MSG_QUICK_SOURCE_PROVIDER
                    .replace("{provider}", effective.display_name()),
                from_study: false,
                outputs,
                fetched: Some(fetched),
            };
            push(ui, &session, &today(state), format);
            *slot.borrow_mut() = Some(session);
            ui.global::<Studies>().set_screen_open(true);
        }
        Err(error) => {
            crate::wiring::dialog::refuse(ui, state::provider_failure_notice(&error));
        }
    }
}

/// The report's value: the pushed figures + the reader's fields as typed.
fn report_value(
    ui: &MainWindow,
    session: &QuickScreenSession,
    today: &str,
    format: NumberFormat,
) -> steadyinvest_report::QuickScreen {
    let q = ui.global::<QuickScreen>();
    let mut view = quick_screen_view(
        QuickScreenHeader {
            ticker: session.ticker.clone(),
            name: session.name.clone(),
            currency: session.currency.clone(),
            date: today.to_string(),
            source: session.source.clone(),
        },
        &session.outputs,
        format,
    );
    view.reasons = q.get_reasons().trim().to_string();
    view.factors_continue = q.get_factors_continue().to_string();
    view.pe_notes = q.get_pe_notes().trim().to_string();
    view.objective = q.get_objective().trim().to_string();
    view.eps_will_meet = q.get_eps_will_meet().to_string();
    view.sales_meets = q.get_sales_meets().to_string();
    view.eps_meets = q.get_eps_meets().to_string();
    view
}

pub(crate) fn wire_quick_screen(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        current_study,
        quick_screen: slot,
        fetch_tx,
        ..
    } = s;
    {
        // « Examiner » on Études: the fundamentals fetch through the configured chain.
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let fetch_tx = fetch_tx.clone();
        ui.global::<QuickScreen>().on_examine(move |ticker, currency| {
            let ui = ui_weak.unwrap();
            let ticker = ticker.trim().to_uppercase();
            if ticker.is_empty() {
                crate::wiring::dialog::refuse(&ui, state::MSG_QUICK_BLANK_TICKER);
                return;
            }
            if config.borrow().preferred_provider == ProviderChoice::None {
                crate::wiring::dialog::refuse(&ui, state::MSG_PROVIDER_NONE);
                return;
            }
            let chain = resolve_chain(
                &config.borrow(),
                steadyinvest_ingestion::FieldKind::Fundamentals,
            );
            if chain.is_empty() {
                crate::wiring::dialog::refuse(&ui, state::MSG_PROVIDER_NO_KEY);
                return;
            }
            let q = ui.global::<QuickScreen>();
            q.set_pick_currency(currency.trim().to_uppercase().into());
            q.set_fetching(true);
            let primary = config.borrow().preferred_provider;
            tracing::info!(ticker = %ticker, provider = primary.wire(), "quick screen requested");
            if fetch_tx
                .send(crate::fetch::WorkerJob::QuickScreen(crate::fetch::FetchRequest {
                    study_id: uuid::Uuid::nil(),
                    ticker,
                    chain,
                    primary,
                }))
                .is_err()
            {
                q.set_fetching(false);
                crate::wiring::dialog::refuse(
                    &ui,
                    &state::MSG_PROVIDER_FAILED
                        .replace("{cause}", "le service de récupération est indisponible"),
                );
            }
        });
    }
    {
        // « Examen rapide » on the open study: its own saved years, no fetch.
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        let slot = Rc::clone(slot);
        ui.global::<QuickScreen>().on_examine_study(move || {
            let ui = ui_weak.unwrap();
            let Some(study) = current_study
                .borrow()
                .as_deref()
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
                .and_then(|id| journal_state.borrow().get_study(id))
            else {
                return;
            };
            let Ok(frame) = build_frame(&study) else {
                crate::wiring::dialog::refuse(&ui, state::MSG_NORMALIZE_FAILED);
                return;
            };
            let outputs = examine(
                &frame.series,
                study.judgment.current_price.map(|m| m.as_decimal()),
                study.judgment.ttm_eps.map(|m| m.as_decimal()),
            );
            let session = QuickScreenSession {
                ticker: study.security_ticker.clone(),
                currency: study.native_currency.clone(),
                name: study.company_name.clone().unwrap_or_default(),
                source: state::MSG_QUICK_SOURCE_STUDY.to_string(),
                from_study: true,
                outputs,
                fetched: None,
            };
            let format = config.borrow().number_format;
            push(&ui, &session, &today(&journal_state.borrow()), format);
            *slot.borrow_mut() = Some(session);
            ui.global::<Studies>().set_screen_open(true);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let slot = Rc::clone(slot);
        ui.global::<QuickScreen>().on_close(move || {
            let ui = ui_weak.unwrap();
            ui.global::<Studies>().set_screen_open(false);
            let from_study = slot.borrow().as_ref().is_some_and(|s| s.from_study);
            if !from_study {
                refresh_studies(&ui, &journal_state.borrow());
            }
        });
    }
    {
        // The reader's objective re-words conclusions 1 and 2.
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let slot = Rc::clone(slot);
        ui.global::<QuickScreen>()
            .on_objective_edited(move |objective| {
                let ui = ui_weak.unwrap();
                let q = ui.global::<QuickScreen>();
                let format = config.borrow().number_format;
                let (sales, eps) = match slot.borrow().as_ref() {
                    Some(s) => (
                        s.outputs.sales.compound_rate_pct,
                        s.outputs.eps.compound_rate_pct,
                    ),
                    None => (None, None),
                };
                q.set_sales_meets(meets_key(sales, &objective, format).into());
                q.set_eps_meets(meets_key(eps, &objective, format).into());
            });
    }
    {
        // « Créer l'étude » from the fetched financials — one write, the study opens on top.
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let slot = Rc::clone(slot);
        ui.global::<QuickScreen>().on_create_study(move || {
            let ui = ui_weak.unwrap();
            let (ticker, currency, fetched) = {
                let guard = slot.borrow();
                let Some(s) = guard.as_ref() else { return };
                let Some(f) = s.fetched.as_ref() else { return };
                (s.ticker.clone(), s.currency.clone(), f.clone())
            };
            let created = journal_state.borrow_mut().create_study(&ticker, &currency);
            let id = match created {
                Ok(id) => id,
                Err(message) => {
                    crate::wiring::dialog::refuse(&ui, &message);
                    return;
                }
            };
            let applied = journal_state
                .borrow_mut()
                .apply_provider_refresh(id, &fetched);
            if let Err(message) = applied {
                crate::wiring::dialog::refuse(&ui, &message);
            }
            refresh_studies(&ui, &journal_state.borrow());
            ui.global::<Studies>().set_screen_open(false);
            ui.global::<Studies>()
                .set_notice(state::MSG_QUICK_SCREEN_STUDY_CREATED.into());
            ui.global::<Studies>()
                .invoke_open_study(id.to_string().into());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let slot = Rc::clone(slot);
        ui.global::<QuickScreen>().on_export_pdf(move || {
            let ui = ui_weak.unwrap();
            let format = config.borrow().number_format;
            let today = today(&journal_state.borrow());
            let value = match slot.borrow().as_ref() {
                Some(s) => report_value(&ui, s, &today, format),
                None => return,
            };
            let bytes = steadyinvest_report::render_quick_screen(&value);
            let mut dialog = rfd::FileDialog::new()
                .set_title("Exporter l'examen rapide en PDF")
                .add_filter("PDF", &["pdf"])
                .set_file_name(format!("examen-{}-{}.pdf", value.ticker, value.date));
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
            match std::fs::write(&path, bytes) {
                Ok(()) => ui.global::<QuickScreen>().set_notice(
                    format!("{} {}", state::MSG_QUICK_SCREEN_EXPORTED, path.display()).into(),
                ),
                Err(e) => {
                    crate::wiring::dialog::refuse(&ui, &format!("{} {e}", state::MSG_SAVE_FAILED))
                }
            }
        });
    }
}
