//! Story 7.3 (PR 2) — the watchlist « criblage »: « Examiner la liste » examines every watched
//! ticker in the watchlist's order — a studied one from its saved years (no fetch), the others
//! through the configured chain, one paced worker job per row (the 6.9 pacing). The worker latches
//! the run's stop flag on the first quota reply, so the rows behind it read « non examiné (limite
//! d'usage) »; any other failure reads « indisponible » on its row only. « Ouvrir l'examen » opens
//! the row's full examination (kept whole — no second fetch). Nothing is written.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use steadyinvest_ingestion::{FetchedFinancials, IngestionError};

use crate::provider::ProviderChoice;
use crate::state::{self, JournalState};
use crate::viewmodel::format::NumberFormat;
use crate::viewmodel::screening::{RowState, failed_state, screening_row_view};
use crate::wiring::Session;
use crate::wiring::fetch::resolve_chain;
use crate::wiring::quick_screen::{
    QuickScreenSession, session_from_fetch, session_from_study, show,
};
use crate::{MainWindow, ScreeningRow, Watchlist};

/// One watched ticker in the run.
pub(crate) struct ScreeningEntry {
    ticker: String,
    has_study: bool,
    state: RowState<Box<QuickScreenSession>>,
}

/// The run of the moment: its rows (the watchlist's order at launch), its identity, its latch.
pub(crate) struct ScreeningSession {
    batch: u64,
    rows: Vec<ScreeningEntry>,
    stop: Arc<AtomicBool>,
}

impl ScreeningSession {
    fn running(&self) -> bool {
        self.rows
            .iter()
            .any(|r| matches!(r.state, RowState::Pending))
    }
}

/// Re-render the « Criblage » card from the run.
fn push(ui: &MainWindow, session: &ScreeningSession, format: NumberFormat) {
    let rows: Vec<ScreeningRow> = session
        .rows
        .iter()
        .map(|r| {
            let state = match &r.state {
                RowState::Pending => RowState::Pending,
                RowState::Examined(s) => RowState::Examined(&s.outputs),
                RowState::Unavailable => RowState::Unavailable,
                RowState::NotExamined => RowState::NotExamined,
            };
            let v = screening_row_view(&r.ticker, r.has_study, &state, format);
            ScreeningRow {
                ticker: v.ticker.into(),
                state: v.state.into(),
                years: v.years.into(),
                sales_rate: v.sales_rate.into(),
                eps_rate: v.eps_rate.into(),
                eps_vs_sales: v.eps_vs_sales.into(),
                pe_position: v.pe_position.into(),
                price_vs_high: v.price_vs_high.into(),
                has_study: v.has_study,
            }
        })
        .collect();
    let w = ui.global::<Watchlist>();
    let done = session
        .rows
        .iter()
        .filter(|r| !matches!(r.state, RowState::Pending))
        .count();
    w.set_screening_rows(ModelRc::new(VecModel::from(rows)));
    w.set_screening_shown(true);
    w.set_screening_running(session.running());
    w.set_screening_done(done as i32);
    w.set_screening_total(session.rows.len() as i32);
    w.set_screening_quota(session.stop.load(Ordering::Relaxed));
}

/// A row's fetch result (called from the fetch outcome handler). A stale batch is ignored.
#[allow(clippy::too_many_arguments)]
pub(crate) fn on_fetched(
    ui: &MainWindow,
    format: NumberFormat,
    slot: &Rc<RefCell<Option<ScreeningSession>>>,
    batch: u64,
    index: usize,
    result: Option<Result<FetchedFinancials, IngestionError>>,
    effective: ProviderChoice,
) {
    let mut guard = slot.borrow_mut();
    let Some(session) = guard.as_mut().filter(|s| s.batch == batch) else {
        return;
    };
    let Some(row) = session.rows.get_mut(index) else {
        return;
    };
    row.state = match result {
        // Skipped behind the quota stop.
        None => RowState::NotExamined,
        Some(Ok(fetched)) if fetched.canonical.years.is_empty() => RowState::Unavailable,
        Some(Ok(fetched)) => {
            // A watched ticker carries no currency: the examination names none (and offers no
            // « Créer l'étude » — Études' « Examiner un titre » asks for one).
            let mut s = session_from_fetch(&row.ticker, "", fetched, effective);
            s.from_watchlist = true;
            RowState::Examined(Box::new(s))
        }
        Some(Err(error)) => failed_state(&error),
    };
    push(ui, session, format);
}

/// Build the run's rows: a studied ticker (linked, else the newest same-ticker study) examined at
/// once from its saved years; the others `Pending` — or `Unavailable` when no provider can fetch.
fn plan(state: &JournalState, can_fetch: bool) -> Vec<ScreeningEntry> {
    state
        .list_watch_items()
        .into_iter()
        .map(|w| {
            let study = w
                .study_id
                .or_else(|| state.study_id_for_ticker(&w.security_ticker))
                .and_then(|id| state.get_study(id));
            let state = match &study {
                Some(study) => match session_from_study(study) {
                    Ok(mut s) => {
                        s.from_watchlist = true;
                        RowState::Examined(Box::new(s))
                    }
                    Err(_) => RowState::Unavailable,
                },
                None if can_fetch => RowState::Pending,
                None => RowState::Unavailable,
            };
            ScreeningEntry {
                ticker: w.security_ticker.to_uppercase(),
                has_study: study.is_some(),
                state,
            }
        })
        .collect()
}

pub(crate) fn wire_screening(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        quick_screen,
        screening,
        fetch_tx,
        ..
    } = s;
    {
        // « Examiner la liste ».
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let slot = Rc::clone(screening);
        let fetch_tx = fetch_tx.clone();
        ui.global::<Watchlist>().on_screen_list(move || {
            let ui = ui_weak.unwrap();
            let format = config.borrow().number_format;
            let primary = config.borrow().preferred_provider;
            let chain = if primary == ProviderChoice::None {
                Vec::new()
            } else {
                resolve_chain(
                    &config.borrow(),
                    steadyinvest_ingestion::FieldKind::Fundamentals,
                )
            };
            let rows = plan(&journal_state.borrow(), !chain.is_empty());
            // A run in flight is superseded: its queued rows drain unfetched, its late results
            // are ignored (another batch number).
            let batch = match slot.borrow().as_ref() {
                Some(previous) => {
                    previous.stop.store(true, Ordering::Relaxed);
                    previous.batch + 1
                }
                None => 1,
            };
            let stop = Arc::new(AtomicBool::new(false));
            let unfetchable = chain.is_empty()
                && rows
                    .iter()
                    .any(|r| matches!(r.state, RowState::Unavailable) && !r.has_study);
            let mut session = ScreeningSession { batch, rows, stop };
            for (index, row) in session.rows.iter_mut().enumerate() {
                if !matches!(row.state, RowState::Pending) {
                    continue;
                }
                let sent = fetch_tx.send(crate::fetch::WorkerJob::Screening(
                    crate::fetch::ScreeningRequest {
                        batch,
                        index,
                        request: crate::fetch::FetchRequest {
                            study_id: uuid::Uuid::nil(),
                            ticker: row.ticker.clone(),
                            chain: chain.clone(),
                            primary,
                        },
                        stop: Arc::clone(&session.stop),
                    },
                ));
                if sent.is_err() {
                    row.state = RowState::Unavailable;
                }
            }
            tracing::info!(
                batch,
                rows = session.rows.len(),
                "watchlist screening requested"
            );
            push(&ui, &session, format);
            *slot.borrow_mut() = Some(session);
            if unfetchable {
                // The unstudied rows could not be fetched: say why once (the studied ones stand).
                let cause = if primary == ProviderChoice::None {
                    state::MSG_PROVIDER_NONE
                } else {
                    state::MSG_PROVIDER_NO_KEY
                };
                crate::wiring::dialog::refuse(&ui, cause);
            }
        });
    }
    {
        // « Ouvrir l'examen » on a row: its examination, whole, on the examination screen.
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let slot = Rc::clone(screening);
        let quick_screen = Rc::clone(quick_screen);
        ui.global::<Watchlist>().on_open_screening(move |index| {
            let ui = ui_weak.unwrap();
            let session = slot
                .borrow()
                .as_ref()
                .and_then(|s| s.rows.get(usize::try_from(index).ok()?))
                .and_then(|r| match &r.state {
                    RowState::Examined(s) => Some((**s).clone()),
                    _ => None,
                });
            let Some(session) = session else { return };
            let format = config.borrow().number_format;
            show(&ui, &journal_state.borrow(), format, &quick_screen, session);
            // The examination screen lives under Études.
            ui.set_current_screen(0);
        });
    }
    {
        // « Fermer le criblage »: the card goes, the run's queued rows drain unfetched.
        let ui_weak = ui.as_weak();
        let slot = Rc::clone(screening);
        ui.global::<Watchlist>().on_close_screening(move || {
            let ui = ui_weak.unwrap();
            if let Some(previous) = slot.borrow_mut().take() {
                previous.stop.store(true, Ordering::Relaxed);
            }
            let w = ui.global::<Watchlist>();
            w.set_screening_shown(false);
            w.set_screening_running(false);
            w.set_screening_rows(ModelRc::new(VecModel::from(Vec::<ScreeningRow>::new())));
            w.set_notice(SharedString::new());
        });
    }
}
