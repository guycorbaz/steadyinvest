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
use crate::viewmodel::quick_screen::{
    QuickScreenHeader, meets_key, quick_screen_view, respell_objective,
};
use crate::wiring::Session;
use crate::wiring::fetch::resolve_chain;
use crate::wiring::list_notice;
use crate::wiring::studies::refresh_studies;
use crate::wiring::study_notice::{self, Source};
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
    q.set_sales_rate_nonpositive(view.sales.rate_nonpositive);
    q.set_eps_lines(strings(&view.eps.lines));
    q.set_eps_years(strings(&view.eps.years));
    q.set_eps_rate(view.eps.rate.into());
    q.set_eps_span(view.eps.span_years.into());
    q.set_eps_nonpositive_base(view.eps.nonpositive_base);
    q.set_eps_unavailable(view.eps.unavailable);
    q.set_eps_rate_nonpositive(view.eps.rate_nonpositive);
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

/// Re-render the examination of the moment in `format` (a number-format change from `old` — G1
/// final review, re-render completeness). The reader's objective is re-spelled into the new
/// format (a valid « 7,5 % » never turns « non lu » by the switch); the other fields stay; the
/// notice slot keeps its message.
pub(crate) fn rerender(
    ui: &MainWindow,
    state: &JournalState,
    slot: &std::cell::RefCell<Option<QuickScreenSession>>,
    old: NumberFormat,
    format: NumberFormat,
) {
    let q = ui.global::<QuickScreen>();
    let objective = respell_objective(&q.get_objective(), old, format);
    q.set_objective(objective.into());
    if let Some(session) = slot.borrow().as_ref() {
        let notice = ui.global::<QuickScreen>().get_notice();
        push(ui, session, &today(state), format);
        ui.global::<QuickScreen>().set_notice(notice);
    }
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

/// A kept « Examiner » outcome (G1 final review): the result, or the failure, of a request whose
/// answer arrived while the studies list was not on screen — named on the list's « Examiner un
/// titre » card, never laid as a screen or a dialog over what the reader had open.
pub(crate) enum KeptExamination {
    Result(Box<QuickScreenSession>),
    Failure { ticker: String, message: String },
}

/// The kept slot (session-only; one outcome at most).
pub(crate) type KeptSlot = std::cell::RefCell<Option<KeptExamination>>;

/// What changes the kept slot — its whole lifecycle (G1 final review).
pub(crate) enum KeptEvent {
    /// A new « Examiner » request is sent: the older outcome is superseded by the reader's choice.
    NewRequest,
    /// An examination is shown (a result on the list, one from a study or the criblage, or the
    /// kept one opened): whatever was kept is older than what is now on screen — dropped, so
    /// « Ouvrir l'examen » can never replace a fresher examination.
    Shown,
    /// A newer outcome is kept: it replaces the previous one explicitly.
    Kept(KeptExamination),
    /// « Compris » on a kept failure, or a dossier change.
    Cleared,
}

/// PURE: the kept slot after `event`.
pub(crate) fn kept_after(event: KeptEvent) -> Option<KeptExamination> {
    match event {
        KeptEvent::Kept(kept) => Some(kept),
        KeptEvent::NewRequest | KeptEvent::Shown | KeptEvent::Cleared => None,
    }
}

/// Apply `event` to the kept slot and mirror it on the card (`ready-ticker` / `ready-failure`) —
/// the one path that writes either.
pub(crate) fn update_kept(ui: &MainWindow, kept: &KeptSlot, event: KeptEvent) {
    let next = kept_after(event);
    let q = ui.global::<QuickScreen>();
    let (ticker, failure) = match &next {
        Some(KeptExamination::Result(s)) => (s.ticker.clone(), String::new()),
        Some(KeptExamination::Failure { ticker, message }) => (ticker.clone(), message.clone()),
        None => (String::new(), String::new()),
    };
    q.set_ready_ticker(ticker.into());
    q.set_ready_failure(failure.into());
    *kept.borrow_mut() = next;
}

/// Push `session` to the screen, keep it as the examination of the moment, open the screen. A
/// new examination starts with blank reader's answers (G1 review: the reasons and answers typed for
/// one company never carry over to the next one's screen or PDF); the objective stays — it is the
/// reader's bar for the session (spec Q1). A kept « Examiner » outcome is older than this one: it
/// goes ([`KeptEvent::Shown`]).
pub(crate) fn show(
    ui: &MainWindow,
    state: &JournalState,
    format: NumberFormat,
    slot: &Rc<std::cell::RefCell<Option<QuickScreenSession>>>,
    kept: &KeptSlot,
    session: QuickScreenSession,
) {
    update_kept(ui, kept, KeptEvent::Shown);
    let q = ui.global::<QuickScreen>();
    q.set_reasons(SharedString::new());
    q.set_factors_continue(SharedString::new());
    q.set_pe_notes(SharedString::new());
    q.set_eps_will_meet(SharedString::new());
    push(ui, &session, &today(state), format);
    *slot.borrow_mut() = Some(session);
    ui.global::<Studies>().set_screen_open(true);
}

/// Supersede any examination fetch in flight: its result will be dropped — the dossier-switch
/// reset only (G1 final review: opening another examination no longer cancels an « Examiner » in
/// silence; its result lands in the kept slot instead, see [`lands_now`]).
pub(crate) fn supersede_request(ui: &MainWindow, request: &Cell<u64>) {
    request.set(request.get() + 1);
    ui.global::<QuickScreen>().set_fetching(false);
}

/// How the examination screen is being closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseVia {
    /// « Retour » on the screen: back where the examination was opened.
    Back,
    /// « Études » on the nav rail — or a programmatic route to Études (`close_studies_overlays`):
    /// the reader chose the destination, and that route re-derives what it shows.
    NavRail,
}

/// Where the reader lands once the examination screen closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AfterClose {
    /// Liste de suivi, re-rendered (its criblage card still shown).
    Watchlist,
    /// The studies list, re-rendered.
    StudiesList,
    /// Nothing to do: the open study underneath shows again, or the nav rail already handles it.
    Stay,
}

/// PURE: the « Retour » destination follows the examination's origin; the nav rail's close keeps
/// the reader's choice (Études) whatever the origin.
pub(crate) fn after_close(from_study: bool, from_watchlist: bool, via: CloseVia) -> AfterClose {
    match via {
        CloseVia::NavRail => AfterClose::Stay,
        CloseVia::Back if from_watchlist => AfterClose::Watchlist,
        CloseVia::Back if from_study => AfterClose::Stay,
        CloseVia::Back => AfterClose::StudiesList,
    }
}

/// Close the examination screen — the ONE close path, for « Retour » and for the nav rail (G1 G:
/// the rail no longer writes `screen-open` behind this handler's back). The examination of the
/// moment stays in the slot (a re-open always replaces it through [`show`]).
pub(crate) fn close_screen(
    ui: &MainWindow,
    state: &JournalState,
    slot: &Rc<std::cell::RefCell<Option<QuickScreenSession>>>,
    via: CloseVia,
) {
    ui.global::<Studies>().set_screen_open(false);
    let (from_study, from_watchlist) = slot
        .borrow()
        .as_ref()
        .map_or((false, false), |s| (s.from_study, s.from_watchlist));
    match after_close(from_study, from_watchlist, via) {
        AfterClose::Watchlist => {
            ui.set_current_screen(1);
            crate::wiring::watchlist::refresh_watchlist(ui, state);
        }
        AfterClose::StudiesList => refresh_studies(ui, state),
        AfterClose::Stay => {}
    }
}

/// End the examination of the moment — the dossier-switch reset (G1 G): a fetch in flight is
/// superseded (its result, asked in the previous dossier, is dropped), the session is emptied
/// (« Créer l'étude » could otherwise write the previous dossier's fetch into the new one), the
/// screen closes and its notice goes.
pub(crate) fn clear_examination(
    ui: &MainWindow,
    slot: &std::cell::RefCell<Option<QuickScreenSession>>,
    kept: &KeptSlot,
    request: &Cell<u64>,
) {
    supersede_request(ui, request);
    *slot.borrow_mut() = None;
    // A kept « Examiner » outcome was asked in the previous dossier: it goes too.
    update_kept(ui, kept, KeptEvent::Cleared);
    ui.global::<Studies>().set_screen_open(false);
    ui.global::<QuickScreen>().set_notice(SharedString::new());
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

/// Where a (current) « Examiner » outcome goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Landing {
    /// The result opens on the list.
    Show,
    /// The failure is refused in the dialog (the list is on screen, nothing is covered).
    Refuse,
    /// Kept and named on the list's card — result or failure alike (G1 final review: never a
    /// screen, nor a dialog, over an open study, comparison or examination).
    Keep,
}

/// PURE: the landing of an outcome — `ok` = a result with analysis years.
pub(crate) fn landing(lands_now: bool, ok: bool) -> Landing {
    match (lands_now, ok) {
        (true, true) => Landing::Show,
        (true, false) => Landing::Refuse,
        (false, _) => Landing::Keep,
    }
}

/// PURE (G1 final review): an « Examiner » result opens at once only when the studies LIST is what
/// the reader has on screen — Études, with no study, comparison or examination open. Anywhere
/// else it is kept (never lost when the reader comes back, never laid over what they have open).
pub(crate) fn lands_now(
    current_screen: i32,
    study_open: bool,
    compare_open: bool,
    screen_open: bool,
) -> bool {
    current_screen == 0 && !study_open && !compare_open && !screen_open
}

/// The worker's examination result (called from the fetch outcome handler). Only the LATEST
/// request is shown — a superseded one is dropped (G1 review: keyed by request identity, never by
/// « whatever arrives last »), and its currency is the one picked when it was asked. It opens at
/// once on the studies list; elsewhere it is kept in `ready` and named on the list's
/// « Examiner un titre » card ([`lands_now`]).
pub(crate) fn on_fetched(
    ui: &MainWindow,
    state: &JournalState,
    format: NumberFormat,
    slot: &Rc<std::cell::RefCell<Option<QuickScreenSession>>>,
    kept: &KeptSlot,
    request: &Cell<u64>,
    outcome: FetchedExamination,
) {
    if outcome.request_id != request.get() {
        tracing::info!(ticker = %outcome.ticker, "superseded quick screen result dropped");
        return;
    }
    ui.global::<QuickScreen>().set_fetching(false);
    let studies = ui.global::<Studies>();
    let now = lands_now(
        ui.get_current_screen(),
        studies.get_study_open(),
        studies.get_compare_open(),
        studies.get_screen_open(),
    );
    // The result, or the failure's own cause.
    let result = match outcome.result {
        Ok(fetched) if !has_analysis_years(&fetched) => Err(state::MSG_PROVIDER_NO_DATA),
        Ok(fetched) => Ok(fetched),
        Err(error) => Err(state::provider_failure_notice(&error)),
    };
    match (landing(now, result.is_ok()), result) {
        (Landing::Show, Ok(fetched)) => {
            let session = session_from_fetch(
                &outcome.ticker,
                &outcome.currency,
                fetched,
                outcome.effective,
            );
            show(ui, state, format, slot, kept, session);
        }
        (Landing::Keep, Ok(fetched)) => {
            tracing::info!(ticker = %outcome.ticker, "quick screen result kept for the list");
            let session = session_from_fetch(
                &outcome.ticker,
                &outcome.currency,
                fetched,
                outcome.effective,
            );
            update_kept(
                ui,
                kept,
                KeptEvent::Kept(KeptExamination::Result(Box::new(session))),
            );
        }
        (Landing::Keep, Err(message)) => {
            tracing::info!(ticker = %outcome.ticker, "quick screen failure kept for the list");
            let ticker = outcome.ticker.to_uppercase();
            let message = message.to_string();
            update_kept(
                ui,
                kept,
                KeptEvent::Kept(KeptExamination::Failure { ticker, message }),
            );
        }
        (_, Err(message)) => crate::wiring::dialog::refuse(ui, message),
        // `landing` never refuses a result.
        (Landing::Refuse, Ok(_)) => {}
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
        quick_screen_ready: kept,
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
        let kept = Rc::clone(kept);
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
            // A new request supersedes a kept outcome (the reader asked for another one).
            update_kept(&ui, &kept, KeptEvent::NewRequest);
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
                        .replace("{cause}", state::MSG_FETCH_WORKER_GONE),
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
        let kept = Rc::clone(kept);
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
            // An « Examiner » fetch in flight is NOT cancelled (G1 final review): its result is
            // kept for the list ([`lands_now`]), never dropped in silence.
            show(&ui, &journal_state.borrow(), format, &slot, &kept, session);
        });
    }
    {
        // « Ouvrir l'examen » on the list's card: the kept « Examiner » result (G1 final review).
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let slot = Rc::clone(slot);
        let kept = Rc::clone(kept);
        ui.global::<QuickScreen>().on_open_ready(move || {
            let ui = ui_weak.unwrap();
            // Only over the list (the card's home): never over an open examination, whose typed
            // answers `show` would blank.
            let studies = ui.global::<Studies>();
            if !lands_now(
                ui.get_current_screen(),
                studies.get_study_open(),
                studies.get_compare_open(),
                studies.get_screen_open(),
            ) {
                return;
            }
            let session = match kept.borrow_mut().take() {
                Some(KeptExamination::Result(session)) => *session,
                other => {
                    *kept.borrow_mut() = other;
                    return;
                }
            };
            let format = config.borrow().number_format;
            // `show` drops the (now taken) kept slot and clears the card.
            show(&ui, &journal_state.borrow(), format, &slot, &kept, session);
        });
    }
    {
        // « Compris » on a kept failure.
        let ui_weak = ui.as_weak();
        let kept = Rc::clone(kept);
        ui.global::<QuickScreen>().on_dismiss_ready(move || {
            update_kept(&ui_weak.unwrap(), &kept, KeptEvent::Cleared);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let slot = Rc::clone(slot);
        ui.global::<QuickScreen>().on_close(move || {
            let ui = ui_weak.unwrap();
            close_screen(&ui, &journal_state.borrow(), &slot, CloseVia::Back);
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
        let current_study = Rc::clone(current_study);
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
            // WAS created (a « refusé » title would misstate it). G1 J: the OPEN study's slot,
            // written AFTER the open (which empties it) so it lands on the study it names.
            ui.global::<Studies>()
                .invoke_open_study(id.to_string().into());
            // G1 J review: only when the open really happened (this study, by id, on screen) —
            // otherwise the list's slot, where the new study's row is.
            let opened = ui.global::<Studies>().get_study_open()
                && current_study.borrow().as_deref() == Some(id.to_string().as_str());
            match applied {
                // The list's slot through its F4 owner (G1 P): never over a sibling's failure.
                Ok(_) if !opened => list_notice::show(
                    &ui,
                    list_notice::Source::QuickScreen,
                    state::MSG_QUICK_SCREEN_STUDY_CREATED,
                ),
                Ok(_) => {
                    study_notice::outcome(&ui, Source::Fetch, state::MSG_QUICK_SCREEN_STUDY_CREATED)
                }
                Err(message) => {
                    let notice = state::MSG_QUICK_SCREEN_STUDY_EMPTY.replace("{cause}", &message);
                    if opened {
                        study_notice::fail(&ui, Source::Fetch, &notice);
                    } else {
                        list_notice::fail(&ui, list_notice::Source::QuickScreen, &notice);
                    }
                }
            }
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
                // Named in French, the OS cause logged — never appended in English (G1 final M4,
                // 2026-09-26: no raw third-party text in a save-failure refusal).
                Err(error) => {
                    tracing::warn!(%error, "export write failed");
                    crate::wiring::dialog::refuse(&ui, state::MSG_EXPORT_WRITE_FAILED)
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept_failure() -> KeptExamination {
        KeptExamination::Failure {
            ticker: "NESN.SW".into(),
            message: "m".into(),
        }
    }

    #[test]
    fn the_kept_outcome_lives_until_a_newer_one_or_a_shown_examination() {
        // Kept, then replaced explicitly by a newer kept outcome.
        assert!(kept_after(KeptEvent::Kept(kept_failure())).is_some());
        // A new « Examiner » request, any shown examination (fresher than the kept one — so
        // « Ouvrir l'examen » never replaces it), « Compris » or a dossier change: gone.
        assert!(kept_after(KeptEvent::NewRequest).is_none());
        assert!(kept_after(KeptEvent::Shown).is_none());
        assert!(kept_after(KeptEvent::Cleared).is_none());
    }

    #[test]
    fn a_current_outcome_lands_over_the_list_or_is_kept_never_laid_over() {
        assert_eq!(landing(true, true), Landing::Show);
        assert_eq!(landing(true, false), Landing::Refuse);
        // A failure with a study / the comparison / an examination open: kept for the card,
        // never a dialog over what the reader has open.
        assert_eq!(landing(false, false), Landing::Keep);
        assert_eq!(landing(false, true), Landing::Keep);
    }

    #[test]
    fn an_examiner_result_opens_at_once_only_over_the_studies_list() {
        // The list itself, on screen: it opens.
        assert!(lands_now(0, false, false, false));
        // Another destination, or something open over the list: kept, never laid over it.
        assert!(!lands_now(2, false, false, false), "on Portefeuille");
        assert!(!lands_now(0, true, false, false), "a study is open");
        assert!(!lands_now(0, false, true, false), "the comparison is open");
        assert!(
            !lands_now(0, false, false, true),
            "another examination is open (e.g. from the criblage)"
        );
    }

    #[test]
    fn back_follows_the_origin_and_the_nav_rail_keeps_the_readers_choice() {
        // « Retour »: back where the examination was opened.
        assert_eq!(
            after_close(false, true, CloseVia::Back),
            AfterClose::Watchlist
        );
        assert_eq!(after_close(true, false, CloseVia::Back), AfterClose::Stay);
        assert_eq!(
            after_close(false, false, CloseVia::Back),
            AfterClose::StudiesList
        );
        // The nav rail's « Études »: never sent to Liste de suivi, whatever the origin (its own
        // arrival re-derives the studies list).
        for (from_study, from_watchlist) in [(false, false), (true, false), (false, true)] {
            assert_eq!(
                after_close(from_study, from_watchlist, CloseVia::NavRail),
                AfterClose::Stay
            );
        }
    }
}
