//! Story 7.3 (PR 2) — the watchlist « criblage »: « Examiner la liste » examines every watched
//! ticker in the watchlist's order — a studied one from its saved years (no fetch), the others
//! through the configured chain, one paced worker job per row (the 6.9 pacing). The worker latches
//! the run's stop flag on the first quota reply, so the rows behind it read « non examiné (limite
//! d'usage) »; any other failure reads « indisponible » on its row only. « Ouvrir l'examen » opens
//! the row's full examination (kept whole — no second fetch). Nothing is written.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use slint::{ComponentHandle, ModelRc, VecModel};
use steadyinvest_ingestion::{FetchedFinancials, IngestionError};

use crate::provider::ProviderChoice;
use crate::state::{self, JournalState};
use crate::viewmodel::format::NumberFormat;
use crate::viewmodel::screening::{RowState, failed_state, screening_row_view};
use crate::wiring::Session;
use crate::wiring::fetch::resolve_chain;
use crate::wiring::quick_screen::{
    QuickScreenSession, has_analysis_years, session_from_fetch, session_from_study, show,
    supersede_request,
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

/// One row's fetch outcome: `quota` = this row latched the run's quota stop.
pub(crate) struct RowOutcome {
    pub(crate) result: Result<FetchedFinancials, IngestionError>,
    pub(crate) effective: ProviderChoice,
    pub(crate) quota: bool,
}

/// A row's fetch result (called from the fetch outcome handler): `None` = drained behind the
/// quota stop. A stale batch is ignored, and so is a result for a row that is not waiting for one
/// (G1 review: a studied row, or one already done, is never overwritten).
pub(crate) fn on_fetched(
    ui: &MainWindow,
    format: NumberFormat,
    slot: &Rc<RefCell<Option<ScreeningSession>>>,
    batch: u64,
    index: usize,
    outcome: Option<RowOutcome>,
) {
    let mut guard = slot.borrow_mut();
    let Some(session) = guard.as_mut().filter(|s| s.batch == batch) else {
        return;
    };
    let Some(row) = session
        .rows
        .get_mut(index)
        .filter(|r| matches!(r.state, RowState::Pending))
    else {
        return;
    };
    row.state = match outcome {
        // Skipped behind the quota stop — or the row that latched it (whatever the chain's final
        // error, the cause named is the usage limit, like the rows behind it).
        None
        | Some(RowOutcome {
            result: Err(_),
            quota: true,
            ..
        }) => RowState::NotExamined,
        Some(RowOutcome {
            result: Ok(fetched),
            ..
        }) if !has_analysis_years(&fetched) => RowState::Unavailable,
        Some(RowOutcome {
            result: Ok(fetched),
            effective,
            ..
        }) => {
            // A watched ticker carries no currency: the examination names none (and offers no
            // « Créer l'étude » — Études' « Examiner un titre » asks for one).
            let mut s = session_from_fetch(&row.ticker, "", fetched, effective);
            s.from_watchlist = true;
            RowState::Examined(Box::new(s))
        }
        Some(RowOutcome {
            result: Err(error), ..
        }) => failed_state(&error),
    };
    push(ui, session, format);
}

/// Build the run's rows: a studied ticker (linked, else the newest same-ticker study) examined at
/// once from its saved years; the others `Pending` — or `Unavailable` when no provider can fetch.
/// Issue #95: a watchlist that cannot be read is an `Err` (never « 0 valeur(s) »), and a row whose
/// study lookup fails is `Unavailable` — never fetched as if it had no study (that spends quota).
fn plan(state: &JournalState, can_fetch: bool) -> Result<Vec<ScreeningEntry>, &'static str> {
    let items = state
        .try_list_watch_items()
        .map_err(|_| state::MSG_SCREENING_LIST_UNREADABLE)?;
    Ok(items
        .into_iter()
        .map(|w| {
            let study = match w.study_id {
                Some(id) => Ok(Some(id)),
                None => state.try_study_id_for_ticker(&w.security_ticker),
            }
            .and_then(|id| match id {
                Some(id) => state.try_get_study(id),
                None => Ok(None),
            });
            let (has_study, state) = match &study {
                Ok(Some(study)) => (
                    true,
                    match session_from_study(study) {
                        Ok(mut s) => {
                            s.from_watchlist = true;
                            RowState::Examined(Box::new(s))
                        }
                        Err(_) => RowState::Unavailable,
                    },
                ),
                Ok(None) if can_fetch => (false, RowState::Pending),
                Ok(None) | Err(_) => (false, RowState::Unavailable),
            };
            ScreeningEntry {
                ticker: w.security_ticker.to_uppercase(),
                has_study,
                state,
            }
        })
        .collect())
}

pub(crate) fn wire_screening(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        quick_screen,
        quick_screen_request,
        screening,
        fetch_tx,
        ..
    } = s;
    {
        // « Examiner la liste ». The batch number comes from a counter that outlives the run
        // (G1 review): after « Fermer le criblage » empties the slot, the next run still gets a NEW
        // number, so a late outcome of the closed run can never match it.
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let slot = Rc::clone(screening);
        let fetch_tx = fetch_tx.clone();
        let last_batch = Rc::new(Cell::new(0u64));
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
            let rows = match plan(&journal_state.borrow(), !chain.is_empty()) {
                Ok(rows) => rows,
                Err(message) => {
                    crate::wiring::dialog::refuse(&ui, message);
                    return;
                }
            };
            // A run in flight is superseded: its queued rows drain unfetched, its late results
            // are ignored (another batch number).
            if let Some(previous) = slot.borrow().as_ref() {
                previous.stop.store(true, Ordering::Relaxed);
            }
            let batch = last_batch.get() + 1;
            last_batch.set(batch);
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
        let request = Rc::clone(quick_screen_request);
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
            supersede_request(&ui, &request);
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
            close_screening(&ui_weak.unwrap(), &slot);
        });
    }
}

/// Stop the run of the moment (its queued rows drain unfetched, its late results find no
/// session) and empty the slot. `true` when there was a run. The batch counter is NOT here: it
/// outlives the run (it lives in `wire_screening`), so the next run still gets a new number.
fn stop_run(slot: &RefCell<Option<ScreeningSession>>) -> bool {
    match slot.borrow_mut().take() {
        Some(previous) => {
            previous.stop.store(true, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// End the criblage: stop the run, empty the slot, hide the card — « Fermer le criblage », and
/// the dossier-switch reset (G1 G: a run of the previous dossier never lands in the next one).
pub(crate) fn close_screening(ui: &MainWindow, slot: &RefCell<Option<ScreeningSession>>) {
    stop_run(slot);
    let w = ui.global::<Watchlist>();
    w.set_screening_shown(false);
    w.set_screening_running(false);
    w.set_screening_done(0);
    w.set_screening_total(0);
    w.set_screening_quota(false);
    // The watchlist's notice slot is left alone (F4): nothing the criblage raised lives
    // there — its refusals go through the dialog.
    w.set_screening_rows(ModelRc::new(VecModel::from(Vec::<ScreeningRow>::new())));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopping_a_run_latches_its_stop_flag_and_empties_the_slot() {
        let stop = Arc::new(AtomicBool::new(false));
        let slot = RefCell::new(Some(ScreeningSession {
            batch: 7,
            rows: vec![ScreeningEntry {
                ticker: "NESN.SW".into(),
                has_study: false,
                state: RowState::Pending,
            }],
            stop: Arc::clone(&stop),
        }));
        assert!(stop_run(&slot));
        // The worker's queued rows see the latch and drain unfetched.
        assert!(stop.load(Ordering::Relaxed));
        // A late outcome of the stopped run finds no session to land in.
        assert!(slot.borrow().is_none());
        // Nothing to stop: a no-op, never a panic.
        assert!(!stop_run(&slot));
    }
}
