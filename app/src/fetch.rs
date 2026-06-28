//! Off-UI-thread provider work (Story 3.1 fetch; Story 3.2 key test).
//!
//! Network I/O must never run on the Slint event loop. A dedicated worker thread owns a
//! `current_thread` tokio runtime and services jobs over a channel; each result is marshalled back
//! to the UI thread via [`slint::invoke_from_event_loop`].
//!
//! The worker closure is `Send` (it carries only the `Send` [`WorkerOutcome`]); it cannot touch the
//! UI-thread `Rc<RefCell<JournalState>>`. The bridge is a UI-thread `thread_local` handler set once
//! at startup (capturing the `Rc` state) — the marshalled closure looks it up when it runs on the
//! UI thread.
//!
//! Story 3.2 adds a **key test** job onto this same worker (no second thread/runtime): a minimal
//! live fetch whose data is discarded — only the `Result` (valid / invalid-key / network / quota)
//! matters.

use std::cell::RefCell;
use std::sync::mpsc;

use steadyinvest_ingestion::{
    adapters::eodhd::EodhdProvider, fetch_canonical, FetchedFinancials, IngestionError, Provider,
};
use uuid::Uuid;

/// The cheap, always-available ticker used to validate a key (works under EODHD's `demo` key too).
const KEY_TEST_TICKER: &str = "AAPL.US";

/// A study-data fetch enqueued from the UI thread (Story 3.1).
pub struct FetchRequest {
    pub study_id: Uuid,
    pub ticker: String,
    pub api_key: Option<String>,
}

/// A key-validation request (Story 3.2): a minimal live fetch whose data is discarded.
pub struct TestKeyRequest {
    pub api_key: Option<String>,
}

/// A job for the worker thread.
pub enum WorkerJob {
    Fetch(FetchRequest),
    /// A holdings price-refresh fetch (Story 4.4): the SAME provider fetch as [`Self::Fetch`], routed
    /// to the holdings surface (current_price + per-ticker freshness), not the open study screen.
    RefreshHolding(FetchRequest),
    TestKey(TestKeyRequest),
}

/// A study-data fetch result, marshalled back to the UI thread. `Send` (no `Rc`, no Slint handle).
/// `ticker` rides back so the holdings refresh (Story 4.4) can key its transient per-ticker freshness
/// map without a second lookup (the study screen arm ignores it).
pub struct FetchOutcome {
    pub study_id: Uuid,
    pub ticker: String,
    pub result: Result<FetchedFinancials, IngestionError>,
}

/// What the worker produces, marshalled back to the UI thread.
pub enum WorkerOutcome {
    Fetch(FetchOutcome),
    /// A holdings price-refresh result (Story 4.4) — routed to the holdings surface, not the study.
    HoldingFetch(FetchOutcome),
    /// Key-test verdict: `Ok` = the provider accepted the key; `Err` carries the cause.
    TestKey(Result<(), IngestionError>),
}

/// The UI-thread handler that applies a [`WorkerOutcome`] to the app state + UI.
type OutcomeHandler = Box<dyn Fn(WorkerOutcome)>;

thread_local! {
    /// Set once at startup (captures the `Rc` state); only ever touched on the UI thread.
    static OUTCOME_HANDLER: RefCell<Option<OutcomeHandler>> = const { RefCell::new(None) };
}

/// Register the UI-thread handler (captures the `Rc` state + UI weak). Call once from `main`.
pub fn set_outcome_handler(handler: impl Fn(WorkerOutcome) + 'static) {
    OUTCOME_HANDLER.with(|h| *h.borrow_mut() = Some(Box::new(handler)));
}

/// Runs on the UI thread (via `invoke_from_event_loop`) — dispatch to the registered handler.
fn dispatch_outcome(outcome: WorkerOutcome) {
    OUTCOME_HANDLER.with(|h| {
        if let Some(handler) = h.borrow().as_ref() {
            handler(outcome);
        }
    });
}

/// Spawn the worker thread and return the job sender. The worker lives for the process.
pub fn spawn_fetch_worker() -> mpsc::Sender<WorkerJob> {
    let (tx, rx) = mpsc::channel::<WorkerJob>();
    std::thread::Builder::new()
        .name("provider-fetch".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("the fetch worker tokio runtime builds");
            // One provider (and its `reqwest::Client` connection pool) reused across all jobs —
            // `Client::new()` is expensive and meant to be shared (review P2).
            let provider = Provider::Eodhd(EodhdProvider::new());
            while let Ok(job) = rx.recv() {
                let outcome = match job {
                    WorkerJob::Fetch(req) => {
                        let result = runtime.block_on(fetch_canonical(
                            &provider,
                            &req.ticker,
                            req.api_key.as_deref(),
                        ));
                        WorkerOutcome::Fetch(FetchOutcome {
                            study_id: req.study_id,
                            ticker: req.ticker,
                            result,
                        })
                    }
                    WorkerJob::RefreshHolding(req) => {
                        // Same provider fetch as the study path; the outcome is routed to the
                        // holdings surface (Story 4.4) — current_price + per-ticker freshness.
                        let result = runtime.block_on(fetch_canonical(
                            &provider,
                            &req.ticker,
                            req.api_key.as_deref(),
                        ));
                        WorkerOutcome::HoldingFetch(FetchOutcome {
                            study_id: req.study_id,
                            ticker: req.ticker,
                            result,
                        })
                    }
                    WorkerJob::TestKey(req) => {
                        // A minimal live fetch; the data is discarded — only the verdict matters.
                        let result = runtime
                            .block_on(fetch_canonical(
                                &provider,
                                KEY_TEST_TICKER,
                                req.api_key.as_deref(),
                            ))
                            .map(|_| ());
                        WorkerOutcome::TestKey(result)
                    }
                };
                // Hand the (Send) outcome back to the UI thread; ignore if the loop is shutting down.
                let _ = slint::invoke_from_event_loop(move || dispatch_outcome(outcome));
            }
        })
        .expect("the fetch worker thread spawns");
    tx
}
