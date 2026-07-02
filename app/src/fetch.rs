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

use rust_decimal::Decimal;
use steadyinvest_ingestion::{
    adapters::eodhd::EodhdProvider, adapters::twelvedata::TwelveDataProvider, fetch_canonical,
    fetch_fx_rate, fetch_price, FetchedFinancials, IngestionError, Provider,
};
use uuid::Uuid;

use crate::provider::ProviderChoice;

/// The cheap, always-available ticker used to validate a key, per provider (the symbol convention
/// differs: EODHD `AAPL.US`, Twelve Data the bare `AAPL`).
fn key_test_ticker(provider: ProviderChoice) -> &'static str {
    match provider {
        ProviderChoice::TwelveData => "AAPL",
        _ => "AAPL.US",
    }
}

/// A study-data fetch enqueued from the UI thread (Story 3.1). `provider` (Story 7.4) selects the
/// adapter the worker routes to.
pub struct FetchRequest {
    pub study_id: Uuid,
    pub ticker: String,
    pub api_key: Option<String>,
    pub provider: ProviderChoice,
}

/// A key-validation request (Story 3.2): a minimal live fetch whose data is discarded.
pub struct TestKeyRequest {
    pub api_key: Option<String>,
    pub provider: ProviderChoice,
}

/// An FX-rates refresh request (Story 6.5, FR28): one job, N `(base, quote)` pairs fetched
/// sequentially (batching/fallback is Story 6.9). User-initiated only (FR65). `journal_id` and
/// `source` are captured at ENQUEUE time (2026-07-02 review): the outcome must be applied to the
/// journal that asked and stamped with the provider that fetched — never read back from mutable
/// config after an in-flight switch.
pub struct FxRatesRequest {
    pub pairs: Vec<(String, String)>,
    pub api_key: Option<String>,
    pub provider: ProviderChoice,
    pub journal_id: Option<Uuid>,
    pub source: String,
}

/// A job for the worker thread.
pub enum WorkerJob {
    Fetch(FetchRequest),
    /// A holdings PRICE refresh (Story 4.4 / issue #50): a price-only `/eod` fetch (no
    /// `/fundamentals`), routed to the holdings surface, not the open study screen.
    RefreshHolding(FetchRequest),
    /// An FX-rates refresh (Story 6.5): the latest BASE→QUOTE rate per pair.
    FetchFxRates(FxRatesRequest),
    TestKey(TestKeyRequest),
}

/// A study-data fetch result, marshalled back to the UI thread. `Send` (no `Rc`, no Slint handle).
pub struct FetchOutcome {
    pub study_id: Uuid,
    pub result: Result<FetchedFinancials, IngestionError>,
}

/// A holdings price-refresh result (Story 4.4 / issue #50): the latest `/eod` close only — no
/// fundamentals. `ticker` rides back so the holdings surface keys its transient per-ticker freshness
/// map; `None` price means the provider exposed no current close.
pub struct HoldingPriceOutcome {
    pub study_id: Uuid,
    pub ticker: String,
    pub result: Result<Option<Decimal>, IngestionError>,
}

/// One pair's FX fetch result (Story 6.5): `Ok(None)` = the provider has no quote for the pair.
pub struct FxRateOutcome {
    pub base: String,
    pub quote: String,
    pub result: Result<Option<Decimal>, IngestionError>,
}

/// What the worker produces, marshalled back to the UI thread.
pub enum WorkerOutcome {
    Fetch(FetchOutcome),
    /// A holdings price-refresh result (Story 4.4) — routed to the holdings surface, not the study.
    HoldingFetch(HoldingPriceOutcome),
    /// The FX-rates refresh results (Story 6.5), one entry per requested pair — plus the
    /// enqueue-time journal identity and provider source (2026-07-02 review).
    FxRates {
        journal_id: Option<Uuid>,
        source: String,
        results: Vec<FxRateOutcome>,
    },
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
            // Both providers (each with its `reqwest::Client` connection pool) built once and reused
            // across jobs — `Client::new()` is expensive and meant to be shared (review P2). Each job
            // selects the adapter for its configured `ProviderChoice` (Story 7.4).
            let eodhd = Provider::Eodhd(EodhdProvider::new());
            let twelvedata = Provider::TwelveData(TwelveDataProvider::new());
            let select = |choice: ProviderChoice| match choice {
                ProviderChoice::TwelveData => &twelvedata,
                _ => &eodhd,
            };
            while let Ok(job) = rx.recv() {
                let outcome = match job {
                    WorkerJob::Fetch(req) => {
                        let result = runtime.block_on(fetch_canonical(
                            select(req.provider),
                            &req.ticker,
                            req.api_key.as_deref(),
                        ));
                        WorkerOutcome::Fetch(FetchOutcome {
                            study_id: req.study_id,
                            result,
                        })
                    }
                    WorkerJob::RefreshHolding(req) => {
                        // Issue #50: a PRICE-ONLY fetch (no fundamentals) so the holdings refresh works
                        // on a free tier; routed to the holdings surface. Twelve Data uses `/price`.
                        let result = runtime.block_on(fetch_price(
                            select(req.provider),
                            &req.ticker,
                            req.api_key.as_deref(),
                        ));
                        WorkerOutcome::HoldingFetch(HoldingPriceOutcome {
                            study_id: req.study_id,
                            ticker: req.ticker,
                            result,
                        })
                    }
                    WorkerJob::FetchFxRates(req) => {
                        // Story 6.5 (FR28): one job, N pairs sequentially (batching = 6.9). Each
                        // pair keeps its own result — one failed pair never hides the others.
                        let mut results = Vec::with_capacity(req.pairs.len());
                        for (base, quote) in req.pairs {
                            let result = runtime.block_on(fetch_fx_rate(
                                select(req.provider),
                                &base,
                                &quote,
                                req.api_key.as_deref(),
                            ));
                            results.push(FxRateOutcome {
                                base,
                                quote,
                                result,
                            });
                        }
                        WorkerOutcome::FxRates {
                            journal_id: req.journal_id,
                            source: req.source,
                            results,
                        }
                    }
                    WorkerJob::TestKey(req) => {
                        // A minimal live fetch; the data is discarded — only the verdict matters. The
                        // test ticker follows the provider's symbol convention.
                        let result = runtime
                            .block_on(fetch_canonical(
                                select(req.provider),
                                key_test_ticker(req.provider),
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
