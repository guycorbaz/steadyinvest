//! Off-UI-thread provider fetch (Story 3.1).
//!
//! Network I/O must never run on the Slint event loop. A dedicated worker thread owns a
//! `current_thread` tokio runtime and services fetch requests over a channel; each result is
//! marshalled back to the UI thread via [`slint::invoke_from_event_loop`].
//!
//! The worker closure is `Send` (it carries only the `Send` [`FetchOutcome`]); it cannot touch the
//! UI-thread `Rc<RefCell<JournalState>>`. The bridge is a UI-thread `thread_local` handler set once
//! at startup (capturing the `Rc` state) — the marshalled closure looks it up when it runs on the
//! UI thread.

use std::cell::RefCell;
use std::sync::mpsc;

use steadyinvest_ingestion::{
    adapters::eodhd::EodhdProvider, fetch_canonical, FetchedFinancials, IngestionError, Provider,
};
use uuid::Uuid;

/// A request enqueued from the UI thread onto the fetch worker.
pub struct FetchRequest {
    pub study_id: Uuid,
    pub ticker: String,
    pub api_key: Option<String>,
}

/// The worker's result, marshalled back to the UI thread. `Send` (no `Rc`, no Slint handle).
pub struct FetchOutcome {
    pub study_id: Uuid,
    pub result: Result<FetchedFinancials, IngestionError>,
}

/// The UI-thread handler that applies a [`FetchOutcome`] to the app state + UI.
type OutcomeHandler = Box<dyn Fn(FetchOutcome)>;

thread_local! {
    /// Set once at startup (captures the `Rc` state); only ever touched on the UI thread.
    static OUTCOME_HANDLER: RefCell<Option<OutcomeHandler>> = const { RefCell::new(None) };
}

/// Register the UI-thread handler (captures the `Rc` state + UI weak). Call once from `main`.
pub fn set_outcome_handler(handler: impl Fn(FetchOutcome) + 'static) {
    OUTCOME_HANDLER.with(|h| *h.borrow_mut() = Some(Box::new(handler)));
}

/// Runs on the UI thread (via `invoke_from_event_loop`) — dispatch to the registered handler.
fn dispatch_outcome(outcome: FetchOutcome) {
    OUTCOME_HANDLER.with(|h| {
        if let Some(handler) = h.borrow().as_ref() {
            handler(outcome);
        }
    });
}

/// Spawn the fetch worker thread and return the request sender. The worker lives for the process.
pub fn spawn_fetch_worker() -> mpsc::Sender<FetchRequest> {
    let (tx, rx) = mpsc::channel::<FetchRequest>();
    std::thread::Builder::new()
        .name("provider-fetch".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("the fetch worker tokio runtime builds");
            // One provider (and its `reqwest::Client` connection pool) reused across all requests —
            // `Client::new()` is expensive and meant to be shared (review P2).
            let provider = Provider::Eodhd(EodhdProvider::new());
            while let Ok(req) = rx.recv() {
                let result = runtime.block_on(fetch_canonical(
                    &provider,
                    &req.ticker,
                    req.api_key.as_deref(),
                ));
                let outcome = FetchOutcome {
                    study_id: req.study_id,
                    result,
                };
                // Hand the (Send) outcome back to the UI thread; ignore if the loop is shutting down.
                let _ = slint::invoke_from_event_loop(move || dispatch_outcome(outcome));
            }
        })
        .expect("the fetch worker thread spawns");
    tx
}
