//! Story 7.3 — the « Examen rapide » rail: a fetch for a ticker with no study (kept in the
//! session, nothing written), or the open study's own years; the screen's figures pushed once;
//! the reader's objective re-words the two rate conclusions; « Exporter PDF » through the native
//! picker; « Créer l'étude » writes the study from the SAME fetched financials (no second fetch).

use std::cell::Cell;
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

/// The examination of the moment (session only). `Clone`: a criblage row keeps its own and hands
/// a copy to the screen on « Ouvrir l'examen ».
#[derive(Clone)]
pub(crate) struct QuickScreenSession {
    pub(crate) ticker: String,
    pub(crate) currency: String,
    pub(crate) name: String,
    pub(crate) source: String,
    pub(crate) from_study: bool,
    /// Opened from the watchlist's criblage → « Retour » lands on Liste de suivi, and no
    /// « Créer l'étude » (a watched ticker carries no currency to create it in).
    pub(crate) from_watchlist: bool,
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
    q.set_from_watchlist(session.from_watchlist);
    q.set_sales_lines(strings(&view.sales.lines));
    q.set_sales_years(strings(&view.sales.years));
    q.set_sales_rate(view.sales.rate.into());
    q.set_sales_span(view.sales.span_years.into());
    q.set_sales_nonpositive_base(view.sales.nonpositive_base);
    q.set_sales_unavailable(view.sales.unavailable);
    q.set_eps_lines(strings(&view.eps.lines));
    q.set_eps_years(strings(&view.eps.years));
    q.set_eps_rate(view.eps.rate.into());
    q.set_eps_span(view.eps.span_years.into());
    q.set_eps_nonpositive_base(view.eps.nonpositive_base);
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
    q.set_pe_basis(view.pe_basis.into());
    q.set_pe_years(view.pe_years.into());
    q.set_record_years(view.record_years.into());
    q.set_present_price(view.present_price.into());
    q.set_present_eps(view.present_eps.into());
    q.set_present_pe(view.present_pe.into());
    q.set_high_five_years_ago(view.high_five_years_ago.into());
    q.set_high_basis(view.high_basis.into());
    q.set_high_year(view.high_year.into());
    q.set_price_vs_high_pct(view.price_vs_high_pct.into());
    q.set_price_vs_high(view.price_vs_high.into());
    q.set_years_sold_as_high(view.years_sold_as_high.into());
    q.set_pe_position(view.pe_position.into());
    q.set_sold_basis(view.sold_basis.into());
    q.set_sold_of(view.sold_of.into());
    q.set_pe_absent(view.pe_absent.into());
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

/// The examination of fetched financials. Issue #109's rule, as the study apply path: a year
/// without `sales` is the provider's price-only row for the fiscal year in progress — not an
/// analysis year (its EPS would read 0 and its high would count as « sold as high »).
pub(crate) fn session_from_fetch(
    ticker: &str,
    currency: &str,
    fetched: FetchedFinancials,
    effective: ProviderChoice,
) -> QuickScreenSession {
    let years: Vec<CanonicalYear> = fetched
        .canonical
        .years
        .iter()
        .filter(|y| y.sales.is_some())
        .cloned()
        .collect();
    let outputs = examine(&years, fetched.latest_price, fetched.ttm_eps);
    QuickScreenSession {
        ticker: ticker.to_uppercase(),
        currency: currency.to_string(),
        name: String::new(),
        source: state::MSG_QUICK_SOURCE_PROVIDER.replace("{provider}", effective.display_name()),
        from_study: false,
        from_watchlist: false,
        outputs,
        fetched: Some(fetched),
    }
}

/// The examination of a saved study's own years (no fetch); `Err` = its series did not normalize.
pub(crate) fn session_from_study(
    study: &steadyinvest_contract::Study,
) -> Result<QuickScreenSession, &'static str> {
    let frame = build_frame(study).map_err(|_| state::MSG_NORMALIZE_FAILED)?;
    let outputs = examine(
        &frame.series,
        study.judgment.current_price.map(|m| m.as_decimal()),
        study.judgment.ttm_eps.map(|m| m.as_decimal()),
    );
    Ok(QuickScreenSession {
        ticker: study.security_ticker.clone(),
        currency: study.native_currency.clone(),
        name: study.company_name.clone().unwrap_or_default(),
        source: state::MSG_QUICK_SOURCE_STUDY.to_string(),
        from_study: true,
        from_watchlist: false,
        outputs,
        fetched: None,
    })
}

/// Push `session` to the screen, keep it as the examination of the moment, open the screen. A
/// new examination starts with blank reader's answers (G1 review: the reasons and answers typed for
/// one company never carry over to the next one's screen or PDF); the objective stays — it is the
/// reader's bar for the session (spec Q1).
pub(crate) fn show(
    ui: &MainWindow,
    state: &JournalState,
    format: NumberFormat,
    slot: &Rc<std::cell::RefCell<Option<QuickScreenSession>>>,
    session: QuickScreenSession,
) {
    let q = ui.global::<QuickScreen>();
    q.set_reasons(SharedString::new());
    q.set_factors_continue(SharedString::new());
    q.set_pe_notes(SharedString::new());
    q.set_eps_will_meet(SharedString::new());
    push(ui, &session, &today(state), format);
    *slot.borrow_mut() = Some(session);
    ui.global::<Studies>().set_screen_open(true);
}

/// Supersede any examination fetch in flight: its result will be dropped (another examination
/// is now the one of the moment).
pub(crate) fn supersede_request(ui: &MainWindow, request: &Cell<u64>) {
    request.set(request.get() + 1);
    ui.global::<QuickScreen>().set_fetching(false);
}

/// A worker examination result, with the identity and the currency of its request.
pub(crate) struct FetchedExamination {
    pub(crate) request_id: u64,
    pub(crate) ticker: String,
    pub(crate) currency: String,
    pub(crate) result: Result<FetchedFinancials, steadyinvest_ingestion::IngestionError>,
    pub(crate) effective: ProviderChoice,
}

/// Whether fetched financials hold an analysis year — a year WITH sales (issue #109: a price-only
/// row is the fiscal year in progress, which [`session_from_fetch`] drops).
pub(crate) fn has_analysis_years(fetched: &FetchedFinancials) -> bool {
    fetched.canonical.years.iter().any(|y| y.sales.is_some())
}

/// The worker's examination result (called from the fetch outcome handler). Only the LATEST
/// request is shown — a superseded one is dropped (G1 review: keyed by request identity, never by
/// « whatever arrives last »), and its currency is the one picked when it was asked.
pub(crate) fn on_fetched(
    ui: &MainWindow,
    state: &JournalState,
    format: NumberFormat,
    slot: &Rc<std::cell::RefCell<Option<QuickScreenSession>>>,
    request: &Cell<u64>,
    outcome: FetchedExamination,
) {
    if outcome.request_id != request.get() {
        tracing::info!(ticker = %outcome.ticker, "superseded quick screen result dropped");
        return;
    }
    let q = ui.global::<QuickScreen>();
    q.set_fetching(false);
    match outcome.result {
        Ok(fetched) if !has_analysis_years(&fetched) => {
            crate::wiring::dialog::refuse(ui, state::MSG_PROVIDER_NO_DATA);
        }
        Ok(fetched) => {
            let session = session_from_fetch(
                &outcome.ticker,
                &outcome.currency,
                fetched,
                outcome.effective,
            );
            show(ui, state, format, slot, session);
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
        quick_screen_request: request,
        fetch_tx,
        ..
    } = s;
    {
        // « Examiner » on Études: the fundamentals fetch through the configured chain. The Rust
        // side holds the button's guards too — Enter in the symbol field reaches here directly.
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let request = Rc::clone(request);
        let fetch_tx = fetch_tx.clone();
        ui.global::<QuickScreen>().on_examine(move |ticker, currency| {
            let ui = ui_weak.unwrap();
            if ui.global::<QuickScreen>().get_fetching() {
                return; // one examination fetch at a time (each costs provider quota)
            }
            let ticker = ticker.trim().to_uppercase();
            if ticker.is_empty() {
                crate::wiring::dialog::refuse(&ui, state::MSG_QUICK_BLANK_TICKER);
                return;
            }
            let currency = currency.trim().to_uppercase();
            if currency.is_empty() {
                crate::wiring::dialog::refuse(&ui, state::MSG_QUICK_BLANK_CURRENCY);
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
            q.set_pick_currency(currency.as_str().into());
            q.set_fetching(true);
            let request_id = request.get() + 1;
            request.set(request_id);
            let primary = config.borrow().preferred_provider;
            tracing::info!(ticker = %ticker, provider = primary.wire(), "quick screen requested");
            if fetch_tx
                .send(crate::fetch::WorkerJob::QuickScreen(
                    crate::fetch::QuickScreenRequest {
                        request_id,
                        currency,
                        request: crate::fetch::FetchRequest {
                            study_id: uuid::Uuid::nil(),
                            ticker,
                            chain,
                            primary,
                        },
                    },
                ))
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
        let request = Rc::clone(request);
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
            let session = match session_from_study(&study) {
                Ok(session) => session,
                Err(message) => {
                    crate::wiring::dialog::refuse(&ui, message);
                    return;
                }
            };
            let format = config.borrow().number_format;
            supersede_request(&ui, &request);
            show(&ui, &journal_state.borrow(), format, &slot, session);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let slot = Rc::clone(slot);
        ui.global::<QuickScreen>().on_close(move || {
            let ui = ui_weak.unwrap();
            ui.global::<Studies>().set_screen_open(false);
            let (from_study, from_watchlist) = slot
                .borrow()
                .as_ref()
                .map_or((false, false), |s| (s.from_study, s.from_watchlist));
            if from_watchlist {
                // Back where the row was opened: Liste de suivi (its criblage card still shown).
                ui.set_current_screen(1);
                crate::wiring::watchlist::refresh_watchlist(&ui, &journal_state.borrow());
            } else if !from_study {
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
            // The study exists either way: the examination is spent, the study opens — with the
            // success notice only when its data really was written (G1 review).
            *slot.borrow_mut() = None;
            refresh_studies(&ui, &journal_state.borrow());
            ui.global::<Studies>().set_screen_open(false);
            // Either way an outcome on the study's notice slot — not a refusal dialog: the study
            // WAS created (a « refusé » title would misstate it).
            let notice = match applied {
                Ok(_) => state::MSG_QUICK_SCREEN_STUDY_CREATED.to_string(),
                Err(message) => state::MSG_QUICK_SCREEN_STUDY_EMPTY.replace("{cause}", &message),
            };
            ui.global::<Studies>().set_notice(notice.into());
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
