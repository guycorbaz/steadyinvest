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

/// One member of a field-type fallback chain (Story 6.9, FR26): the provider plus ITS OWN key,
/// both resolved at ENQUEUE time (the worker never touches the keychain — NFR-S1/thread
/// discipline). A keyed member without a key is dropped at enqueue, never shipped.
#[derive(Clone)]
pub struct ChainMember {
    pub provider: ProviderChoice,
    pub api_key: Option<String>,
}

/// A study-data fetch enqueued from the UI thread (Story 3.1). `chain` (Story 6.9, FR26) is the
/// job's FIELD-TYPE fallback chain — fundamentals for `WorkerJob::Fetch`, PRICE for
/// `WorkerJob::RefreshHolding` (same struct, different chain). `primary` is the CONFIGURED
/// preferred provider captured at enqueue: the fallback notice fires whenever the effective
/// member differs from it — including when the primary was dropped from the chain at enqueue
/// (missing key, incapable of the field), never keyed off chain position (2026-07-03 review,
/// CRITICAL: a keyless primary must not let the fallback serve in silence).
pub struct FetchRequest {
    pub study_id: Uuid,
    pub ticker: String,
    pub chain: Vec<ChainMember>,
    pub primary: ProviderChoice,
}

/// A key-validation request (Story 3.2): a minimal live fetch whose data is discarded.
pub struct TestKeyRequest {
    pub api_key: Option<String>,
    pub provider: ProviderChoice,
}

/// An FX-rates refresh request (Story 6.5, FR28): one job, N `(base, quote)` pairs, each run
/// down the FX fallback `chain` (Story 6.9 — a quota on one pair fails over per pair) with the
/// declared pacing between requests. User-initiated only (FR65). `journal_id` is captured at
/// ENQUEUE time (2026-07-02 review): the outcome applies only to the journal that asked; the
/// stamped source is each pair's EFFECTIVE member (FR26), carried per result.
pub struct FxRatesRequest {
    pub pairs: Vec<(String, String)>,
    pub chain: Vec<ChainMember>,
    /// The CONFIGURED primary at enqueue (see [`FetchRequest::primary`]).
    pub primary: ProviderChoice,
    pub journal_id: Option<Uuid>,
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
/// `fell_back_to` (Story 6.9, FR26) names the NON-PRIMARY member that served a success — a
/// fallback is a visible event, never a silence; `None` = the primary served, or the whole
/// chain failed (the error names itself).
pub struct FetchOutcome {
    pub study_id: Uuid,
    pub result: Result<FetchedFinancials, IngestionError>,
    pub fell_back_to: Option<ProviderChoice>,
}

/// A holdings price-refresh result (Story 4.4 / issue #50): the latest `/eod` close only — no
/// fundamentals. `ticker` rides back so the holdings surface keys its transient per-ticker freshness
/// map; `None` price means the provider exposed no current close.
pub struct HoldingPriceOutcome {
    pub study_id: Uuid,
    pub ticker: String,
    pub result: Result<Option<Decimal>, IngestionError>,
    /// Story 6.9: the non-primary member that served a success (see [`FetchOutcome`]).
    pub fell_back_to: Option<ProviderChoice>,
}

/// One pair's FX fetch result (Story 6.5): `Ok(None)` = the provider has no quote for the pair.
/// `effective` (Story 6.9, FR26) is the wire name of the member that produced the result — the
/// `fx_rates.source` stamp records the provider that ACTUALLY fetched, never the primary's name
/// on a fallback's data.
pub struct FxRateOutcome {
    pub base: String,
    pub quote: String,
    pub result: Result<Option<Decimal>, IngestionError>,
    pub effective: String,
}

/// What the worker produces, marshalled back to the UI thread.
pub enum WorkerOutcome {
    Fetch(FetchOutcome),
    /// A holdings price-refresh result (Story 4.4) — routed to the holdings surface, not the study.
    HoldingFetch(HoldingPriceOutcome),
    /// The FX-rates refresh results (Story 6.5), one entry per requested pair — plus the
    /// enqueue-time journal identity (2026-07-02 review); each result carries its own effective
    /// source (Story 6.9). `fell_back_to` names the fallback when ANY pair used one.
    FxRates {
        journal_id: Option<Uuid>,
        results: Vec<FxRateOutcome>,
        fell_back_to: Option<ProviderChoice>,
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

/// The longest quota `retry_after` the worker will actually wait out (Story 6.9, FR27): a
/// user-initiated refresh must not hang for minutes — anything longer advances the chain (or
/// surfaces the quota honestly).
pub const QUOTA_WAIT_CAP_SECS: u64 = 30;

/// The remaining pacing delay before the provider may be hit again (Story 6.9, FR27) — pure:
/// `None` last request = no delay; an elapsed interval = no delay; otherwise the exact remainder.
pub fn delay_before(
    now: std::time::Instant,
    last: Option<std::time::Instant>,
    min_interval: std::time::Duration,
) -> std::time::Duration {
    match last {
        None => std::time::Duration::ZERO,
        Some(last) => min_interval.saturating_sub(now.duration_since(last)),
    }
}

/// Whether (and how long) to wait out a quota before ONE retry of the same member (Story 6.9,
/// FR27) — pure: only a declared, positive `retry_after` within [`QUOTA_WAIT_CAP_SECS`] is worth
/// waiting for; a missing or oversized one advances the chain instead.
pub fn quota_wait(retry_after_secs: Option<u64>) -> Option<std::time::Duration> {
    retry_after_secs
        .filter(|s| *s > 0 && *s <= QUOTA_WAIT_CAP_SECS)
        .map(std::time::Duration::from_secs)
}

/// Sleep out the declared pacing for `tag`, then stamp the request instant (Story 6.9, FR27).
/// Runs on the worker thread only — `Instant` is monotonic infra timing (ADD15's injected clock
/// governs journal facts, not throttling).
fn pace(
    last_request: &mut std::collections::HashMap<&'static str, std::time::Instant>,
    tag: &'static str,
) {
    let wait = delay_before(
        std::time::Instant::now(),
        last_request.get(tag).copied(),
        steadyinvest_ingestion::min_request_interval(tag),
    );
    if !wait.is_zero() {
        std::thread::sleep(wait);
    }
    last_request.insert(tag, std::time::Instant::now());
}

// Run one fetch down a fallback chain (Story 6.9, FR26/FR27): members in order,
// paced; a Quota with a waitable retry-after gets ONE same-member retry; ANY error
// advances (coverage, quotas, keys and even parse/normalize failures differ per
// provider — the LAST error surfaces). Returns the result, the member that produced
// it, and the fallback event: a success served by anyone OTHER than the CONFIGURED
// `primary` — by identity, never by chain position (2026-07-03 review: a primary
// dropped at enqueue leaves the fallback at index 0, and that service must still be
// a visible event).
fn run_chain<'a, T>(
    last_request: &mut std::collections::HashMap<&'static str, std::time::Instant>,
    select: impl Fn(ProviderChoice) -> &'a Provider,
    chain: &[ChainMember],
    primary: ProviderChoice,
    mut call: impl FnMut(&'a Provider, Option<&str>) -> Result<T, IngestionError>,
) -> (
    Result<T, IngestionError>,
    Option<ProviderChoice>,
    Option<ProviderChoice>,
) {
    // Unreachable through the guarded enqueue sites — defensive only.
    let mut result: Result<T, IngestionError> = Err(IngestionError::Provider(
        steadyinvest_ingestion::ProviderError::Unsupported {
            detail: "empty provider chain".to_string(),
        },
    ));
    let mut effective: Option<ProviderChoice> = None;
    for member in chain.iter() {
        let provider = select(member.provider);
        pace(last_request, provider.tag());
        let mut attempt = call(provider, member.api_key.as_deref());
        if let Err(IngestionError::Provider(steadyinvest_ingestion::ProviderError::Quota {
            retry_after_secs,
        })) = &attempt
        {
            if let Some(wait) = quota_wait(*retry_after_secs) {
                // FR27: honor the declared retry-after (bounded) — ONE retry.
                std::thread::sleep(wait);
                last_request.insert(provider.tag(), std::time::Instant::now());
                attempt = call(provider, member.api_key.as_deref());
            }
        }
        effective = Some(member.provider);
        let succeeded = attempt.is_ok();
        result = attempt;
        if succeeded {
            // By IDENTITY vs the configured primary, never by chain position (review).
            let fell_back = (member.provider != primary).then_some(member.provider);
            return (result, effective, fell_back);
        }
    }
    (result, effective, None)
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
            // FR27: one last-request instant per provider tag — the SINGLE worker loop is the
            // choke point every job serializes through, so pacing here covers the whole batch.
            let mut last_request: std::collections::HashMap<&'static str, std::time::Instant> =
                std::collections::HashMap::new();
            while let Ok(job) = rx.recv() {
                let outcome = match job {
                    WorkerJob::Fetch(req) => {
                        let (result, _, fell_back_to) = run_chain(
                            &mut last_request,
                            select,
                            &req.chain,
                            req.primary,
                            |provider, key| {
                                runtime.block_on(fetch_canonical(provider, &req.ticker, key))
                            },
                        );
                        WorkerOutcome::Fetch(FetchOutcome {
                            study_id: req.study_id,
                            result,
                            fell_back_to,
                        })
                    }
                    WorkerJob::RefreshHolding(req) => {
                        // Issue #50: a PRICE-ONLY fetch (no fundamentals) so the holdings refresh works
                        // on a free tier; routed to the holdings surface. Twelve Data uses `/price`.
                        // 2026-07-03 review: an `Ok(None)` (the provider has no quote) ADVANCES the
                        // chain like `TickerNotFound` — coverage gaps differ per provider; an
                        // all-None chain surfaces as no-data, the same message as before.
                        let (result, _, fell_back_to) = run_chain(
                            &mut last_request,
                            select,
                            &req.chain,
                            req.primary,
                            |provider, key| match runtime.block_on(fetch_price(
                                provider,
                                &req.ticker,
                                key,
                            )) {
                                Ok(None) => Err(IngestionError::Provider(
                                    steadyinvest_ingestion::ProviderError::TickerNotFound {
                                        ticker: req.ticker.clone(),
                                    },
                                )),
                                other => other,
                            },
                        );
                        WorkerOutcome::HoldingFetch(HoldingPriceOutcome {
                            study_id: req.study_id,
                            ticker: req.ticker,
                            result,
                            fell_back_to,
                        })
                    }
                    WorkerJob::FetchFxRates(req) => {
                        // Story 6.5 (FR28) + 6.9: N pairs, EACH run down the FX chain (a quota on
                        // one pair fails over per pair), all paced through the shared map. Each
                        // pair keeps its own result — one failed pair never hides the others.
                        let mut results = Vec::with_capacity(req.pairs.len());
                        let mut fell_back_to: Option<ProviderChoice> = None;
                        for (base, quote) in req.pairs {
                            // 2026-07-03 review: a no-quote `Ok(None)` advances the chain (per
                            // pair) — symbol coverage differs per provider; an all-None chain
                            // surfaces as no-data, the same message as before.
                            let (result, effective, pair_fell_back) = run_chain(
                                &mut last_request,
                                select,
                                &req.chain,
                                req.primary,
                                |provider, key| match runtime
                                    .block_on(fetch_fx_rate(provider, &base, &quote, key))
                                {
                                    Ok(None) => Err(IngestionError::Provider(
                                        steadyinvest_ingestion::ProviderError::TickerNotFound {
                                            ticker: format!("{base}/{quote}"),
                                        },
                                    )),
                                    other => other,
                                },
                            );
                            if pair_fell_back.is_some() {
                                fell_back_to = pair_fell_back;
                            }
                            results.push(FxRateOutcome {
                                base,
                                quote,
                                result,
                                effective: effective
                                    .map(|c| c.wire().to_string())
                                    .unwrap_or_default(),
                            });
                        }
                        WorkerOutcome::FxRates {
                            journal_id: req.journal_id,
                            results,
                            fell_back_to,
                        }
                    }
                    WorkerJob::TestKey(req) => {
                        // A minimal live fetch; the data is discarded — only the verdict matters. The
                        // test ticker follows the provider's symbol convention. Deliberately single-
                        // provider (testing THIS key, never a fallback's) but paced like the rest.
                        let provider = select(req.provider);
                        pace(&mut last_request, provider.tag());
                        let result = runtime
                            .block_on(fetch_canonical(
                                provider,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use steadyinvest_ingestion::{FakeProvider, ProviderError};

    // ── Story 6.9 — the pure pacing/retry decisions (FR27; no sleep-based tests) ──

    #[test]
    fn delay_before_is_zero_first_time_zero_when_elapsed_and_the_exact_remainder_otherwise() {
        let interval = Duration::from_millis(1000);
        let t0 = Instant::now();
        assert_eq!(
            delay_before(t0, None, interval),
            Duration::ZERO,
            "no prior request → no delay"
        );
        assert_eq!(
            delay_before(t0 + Duration::from_millis(1500), Some(t0), interval),
            Duration::ZERO,
            "interval elapsed → no delay"
        );
        assert_eq!(
            delay_before(t0 + Duration::from_millis(400), Some(t0), interval),
            Duration::from_millis(600),
            "the exact remainder"
        );
        assert_eq!(
            delay_before(t0, Some(t0), Duration::ZERO),
            Duration::ZERO,
            "an unpaced provider never waits"
        );
    }

    #[test]
    fn quota_wait_honors_only_a_declared_positive_bounded_retry_after() {
        assert_eq!(quota_wait(Some(5)), Some(Duration::from_secs(5)));
        assert_eq!(
            quota_wait(Some(QUOTA_WAIT_CAP_SECS)),
            Some(Duration::from_secs(QUOTA_WAIT_CAP_SECS)),
            "the cap itself is waitable"
        );
        assert_eq!(
            quota_wait(Some(QUOTA_WAIT_CAP_SECS + 1)),
            None,
            "an oversized retry-after advances the chain instead"
        );
        assert_eq!(quota_wait(Some(0)), None, "zero is not a wait");
        assert_eq!(quota_wait(None), None, "undeclared → advance");
    }

    // ── Story 6.9 — the chain runner (FR26): failover, effective member, honest last error ──

    /// Two fakes behind the same `select` shape the worker uses: `Eodhd` → the first canned
    /// provider, `TwelveData` → the second.
    fn fakes(
        first: Result<steadyinvest_core::normalize::RawFinancials, ProviderError>,
        second: Result<steadyinvest_core::normalize::RawFinancials, ProviderError>,
    ) -> (Provider, Provider) {
        (
            Provider::Fake(FakeProvider::returning_with_price(first, Some(1.into()))),
            Provider::Fake(FakeProvider::returning_with_price(second, Some(2.into()))),
        )
    }

    fn raw() -> steadyinvest_core::normalize::RawFinancials {
        steadyinvest_core::normalize::RawFinancials {
            native_currency: "USD".into(),
            years: vec![],
            splits: vec![],
        }
    }

    fn chain() -> Vec<ChainMember> {
        vec![
            ChainMember {
                provider: ProviderChoice::Eodhd,
                api_key: Some("k1".into()),
            },
            ChainMember {
                provider: ProviderChoice::TwelveData,
                api_key: Some("k2".into()),
            },
        ]
    }

    /// Drive `run_chain` over the price path with two canned members.
    fn run_price_chain(
        first: Result<steadyinvest_core::normalize::RawFinancials, ProviderError>,
        second: Result<steadyinvest_core::normalize::RawFinancials, ProviderError>,
        members: &[ChainMember],
    ) -> (
        Result<Option<rust_decimal::Decimal>, IngestionError>,
        Option<ProviderChoice>,
        Option<ProviderChoice>,
    ) {
        let (a, b) = fakes(first, second);
        let select = |choice: ProviderChoice| match choice {
            ProviderChoice::TwelveData => &b,
            _ => &a,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut last_request = std::collections::HashMap::new();
        run_chain(
            &mut last_request,
            select,
            members,
            ProviderChoice::Eodhd,
            |provider, key| match runtime.block_on(fetch_price(provider, "AAPL", key)) {
                // The worker's no-quote rule (2026-07-03 review): Ok(None) advances the chain.
                Ok(None) => Err(IngestionError::Provider(ProviderError::TickerNotFound {
                    ticker: "AAPL".into(),
                })),
                other => other,
            },
        )
    }

    #[test]
    fn a_quota_on_the_primary_lands_the_fallback_and_names_it() {
        // No waitable retry-after → the chain advances immediately (the quota_wait rule).
        let (result, effective, fell_back) = run_price_chain(
            Err(ProviderError::Quota {
                retry_after_secs: None,
            }),
            Ok(raw()),
            &chain(),
        );
        assert_eq!(result.unwrap(), Some(2.into()), "the fallback's price");
        assert_eq!(effective, Some(ProviderChoice::TwelveData));
        assert_eq!(
            fell_back,
            Some(ProviderChoice::TwelveData),
            "a non-primary success IS a fallback — a visible event"
        );
    }

    #[test]
    fn a_not_found_ticker_fails_over_too_and_a_primary_success_never_falls_back() {
        // Symbol coverage differs per provider — TickerNotFound advances the chain.
        let (result, _, fell_back) = run_price_chain(
            Err(ProviderError::TickerNotFound {
                ticker: "AAPL".into(),
            }),
            Ok(raw()),
            &chain(),
        );
        assert_eq!(result.unwrap(), Some(2.into()));
        assert_eq!(fell_back, Some(ProviderChoice::TwelveData));

        // The primary serving is the quiet path: no fallback event.
        let (result, effective, fell_back) = run_price_chain(Ok(raw()), Ok(raw()), &chain());
        assert_eq!(result.unwrap(), Some(1.into()), "the primary's price");
        assert_eq!(effective, Some(ProviderChoice::Eodhd));
        assert_eq!(fell_back, None);
    }

    #[test]
    fn an_exhausted_chain_surfaces_the_last_error_honestly() {
        let (result, effective, fell_back) = run_price_chain(
            Err(ProviderError::Network {
                detail: "offline".into(),
            }),
            Err(ProviderError::InvalidOrAbsentKey),
            &chain(),
        );
        assert!(matches!(
            result.unwrap_err(),
            IngestionError::Provider(ProviderError::InvalidOrAbsentKey)
        ));
        assert_eq!(
            effective,
            Some(ProviderChoice::TwelveData),
            "the LAST member tried"
        );
        assert_eq!(fell_back, None, "no success → no fallback event");
    }

    #[test]
    fn a_dropped_primary_still_makes_the_fallback_a_visible_event() {
        // 2026-07-03 review (CRITICAL): a keyless primary is dropped at enqueue, so the fallback
        // ships as chain[0] — the fallback event keys off the CONFIGURED primary (Eodhd here),
        // never off chain position.
        let members = vec![ChainMember {
            provider: ProviderChoice::TwelveData,
            api_key: Some("k2".into()),
        }];
        let (result, effective, fell_back) = run_price_chain(Ok(raw()), Ok(raw()), &members);
        // chain[0] routes to the SECOND fake (TwelveData) in this harness.
        assert_eq!(result.unwrap(), Some(2.into()));
        assert_eq!(effective, Some(ProviderChoice::TwelveData));
        assert_eq!(
            fell_back,
            Some(ProviderChoice::TwelveData),
            "effective ≠ configured primary ⇒ a visible fallback, even at index 0"
        );
    }

    #[test]
    fn a_no_quote_ok_none_advances_the_chain_like_a_coverage_miss() {
        // 2026-07-03 review (HIGH): the primary answers Ok(None) — no quote for the symbol —
        // and the wrapped call converts it to a coverage miss, so the fallback serves.
        let (a, b) = (
            Provider::Fake(FakeProvider::returning_with_price(Ok(raw()), None)),
            Provider::Fake(FakeProvider::returning_with_price(
                Ok(raw()),
                Some(2.into()),
            )),
        );
        let select = |choice: ProviderChoice| match choice {
            ProviderChoice::TwelveData => &b,
            _ => &a,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut last_request = std::collections::HashMap::new();
        let (result, _, fell_back) = run_chain(
            &mut last_request,
            select,
            &chain(),
            ProviderChoice::Eodhd,
            |provider, key| match runtime.block_on(fetch_price(provider, "AAPL", key)) {
                Ok(None) => Err(IngestionError::Provider(ProviderError::TickerNotFound {
                    ticker: "AAPL".into(),
                })),
                other => other,
            },
        );
        assert_eq!(result.unwrap(), Some(2.into()), "the fallback's quote");
        assert_eq!(fell_back, Some(ProviderChoice::TwelveData));
    }

    #[test]
    fn a_single_member_chain_behaves_like_the_pre_6_9_single_provider() {
        let members = vec![ChainMember {
            provider: ProviderChoice::Eodhd,
            api_key: Some("k1".into()),
        }];
        let (result, effective, fell_back) = run_price_chain(
            Err(ProviderError::Quota {
                retry_after_secs: None,
            }),
            Ok(raw()),
            &members,
        );
        assert!(matches!(
            result.unwrap_err(),
            IngestionError::Provider(ProviderError::Quota { .. })
        ));
        assert_eq!(effective, Some(ProviderChoice::Eodhd));
        assert_eq!(fell_back, None);
    }
}
