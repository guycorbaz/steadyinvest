//! The `MarketDataProvider` trait — the provider-agnostic acquisition boundary (FR15).
//!
//! An adapter's single job is `fetch a ticker → core::normalize::RawFinancials`. Normalization
//! (IFRS↔GAAP, splits, fiscal period, currency) is NOT the adapter's concern — it happens once,
//! centrally, in `core::normalize` (called by [`crate::fetch_canonical`]).
//!
//! Keys are **injected by the caller** (`app`, from the OS keychain in Story 3.2; from an env var
//! as the 3.1 interim) and passed as `Option<&str>` — `None` for keyless providers. A provider
//! never reads a key from disk, the journal, or the environment itself.

use rust_decimal::Decimal;
use steadyinvest_core::normalize::RawFinancials;

use crate::error::ProviderError;

/// A latest close paired with the trading-**session** date it is *for*, when the provider supplies
/// one (issue #72). EODHD's `/eod` bars carry a `date`; Twelve Data's bare `/price` does not, so
/// `session_date` is `None` there and the caller keys its `price_history` cache by the clock day
/// instead. `session_date` is the raw provider date string (`"YYYY-MM-DD"`), validated by the caller
/// before it becomes a cache key. Keying by the real session date stops a weekend/holiday refresh
/// from filing the prior session's close under today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatedClose {
    pub close: Decimal,
    pub session_date: Option<String>,
}

/// What an adapter returns (Story 4.4): the `core` [`RawFinancials`] **plus** the present market
/// price — the **latest `/eod` close**, if the provider supplies one. The latest price is a present
/// market fact for the §4 zone marker (FR40), **not** an SSG calc input, so it rides this
/// ingestion-owned wrapper and is **never** added to `core`'s `RawFinancials`/`CanonicalFinancials`
/// (the method fingerprint stays frozen). `None` when the provider exposes no current price.
#[derive(Debug, Clone)]
pub struct RawFetch {
    pub financials: RawFinancials,
    pub latest_price: Option<Decimal>,
    /// Issue #72: the trading-session date of `latest_price`, when the provider dates its bars (EODHD
    /// `/eod`, Twelve Data `/time_series`) — threaded to `cache_close` so the confront trajectory is
    /// keyed by the real session, not the refresh day. `None` when undated → the caller falls back.
    pub latest_session_date: Option<String>,
    /// Trailing-twelve-months EPS (Issue #113) — the current-P/E denominator, a present market fact
    /// (like `latest_price`), NOT an annual figure (so it rides here, not in `RawFinancials`).
    pub ttm_eps: Option<Decimal>,
    /// The company's sector (issue #98, FR48 — EODHD `General::Sector`; `None` when the provider
    /// does not expose one). A company fact, not an annual figure — it rides beside the price.
    pub sector: Option<String>,
}

/// A source of raw annual fundamentals + prices for a security.
///
/// `async fn` in trait (MSRV 1.96). The concrete providers are dispatched via the
/// [`crate::Provider`] enum rather than `dyn` (native async-fn-in-trait is not dyn-compatible, and
/// enum dispatch avoids an `async-trait` dependency).
// The future is created and `block_on`-awaited on the same fetch worker thread (never held as a
// generic `T: MarketDataProvider` across threads), so an explicit `Send` bound on the returned
// future is unnecessary — suppress the advisory lint rather than desugar to `impl Future + Send`.
#[allow(async_fn_in_trait)]
pub trait MarketDataProvider: Send + Sync {
    /// Fetch a ticker's raw annual series + its latest price. `api_key` is `None` for keyless
    /// providers. Returns a [`RawFetch`] (the `core` `RawFinancials` + the latest-close market price).
    async fn fetch_fundamentals(
        &self,
        ticker: &str,
        api_key: Option<&str>,
    ) -> Result<RawFetch, ProviderError>;

    /// Fetch ONLY the latest market price — the latest `/eod` close — with **no** `/fundamentals`
    /// request (issue #50). The holdings price refresh (Story 4.4) is price-led: it needs the present
    /// price, not the annual series, so it must not pay for (or be blocked by) fundamentals. This is
    /// what makes the refresh work on plans where EOD is allowed but fundamentals are forbidden (the
    /// free EODHD tier 403s `/fundamentals`). `None` when the provider exposes no current price / the
    /// series is empty. Issue #72: the close rides with its trading-**session** date ([`DatedClose`])
    /// when the provider dates its quote (EODHD `/eod`); an undated quote carries `session_date: None`.
    async fn fetch_latest_price(
        &self,
        ticker: &str,
        api_key: Option<&str>,
    ) -> Result<Option<DatedClose>, ProviderError>;

    /// Fetch the latest BASE→QUOTE exchange rate (Story 6.5, FR28) — how many units of `quote` one
    /// unit of `base` buys. **User-initiated only** (FR65: never background-polled); the app calls
    /// this from the « Actualiser les taux » action, once per foreign currency actually in use.
    ///
    /// Each adapter spells the pair in its own symbol convention (Twelve Data `"{base}/{quote}"` on
    /// `/price`; EODHD `"{base}{quote}.FOREX"` on `/eod` — the #70 per-provider-symbol class) and
    /// **delegates to its own latest-price path**, because an FX quote is the same wire shape as an
    /// equity's latest price — same [`DatedClose`] shape, so issue #90 (part 3) gets the SAME
    /// real-session-date keying issue #72 gave the price cache, for free. Returns `Ok(None)` when
    /// the provider has no quote for the pair — never an inverted-pair guess. Same NFR-S1
    /// discipline as every fetch (key never in errors).
    async fn fetch_fx_rate(
        &self,
        base: &str,
        quote: &str,
        api_key: Option<&str>,
    ) -> Result<Option<DatedClose>, ProviderError>;
}
