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

/// What an adapter returns (Story 4.4): the `core` [`RawFinancials`] **plus** the present market
/// price — the **latest `/eod` close**, if the provider supplies one. The latest price is a present
/// market fact for the §4 zone marker (FR40), **not** an SSG calc input, so it rides this
/// ingestion-owned wrapper and is **never** added to `core`'s `RawFinancials`/`CanonicalFinancials`
/// (the method fingerprint stays frozen). `None` when the provider exposes no current price.
#[derive(Debug, Clone)]
pub struct RawFetch {
    pub financials: RawFinancials,
    pub latest_price: Option<Decimal>,
    /// Trailing-twelve-months EPS (Issue #113) — the current-P/E denominator, a present market fact
    /// (like `latest_price`), NOT an annual figure (so it rides here, not in `RawFinancials`).
    pub ttm_eps: Option<Decimal>,
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
    /// series is empty.
    async fn fetch_latest_price(
        &self,
        ticker: &str,
        api_key: Option<&str>,
    ) -> Result<Option<Decimal>, ProviderError>;

    /// Fetch the latest BASE→QUOTE exchange rate (Story 6.5, FR28) — how many units of `quote` one
    /// unit of `base` buys. **User-initiated only** (FR65: never background-polled); the app calls
    /// this from the « Actualiser les taux » action, once per foreign currency actually in use.
    ///
    /// Each adapter spells the pair in its own symbol convention (Twelve Data `"{base}/{quote}"` on
    /// `/price`; EODHD `"{base}{quote}.FOREX"` on `/eod` — the #70 per-provider-symbol class) and
    /// **delegates to its own latest-price path**, because an FX quote is the same wire shape as an
    /// equity's latest price. Returns `Ok(None)` when the provider has no quote for the pair —
    /// never an inverted-pair guess. Same NFR-S1 discipline as every fetch (key never in errors).
    async fn fetch_fx_rate(
        &self,
        base: &str,
        quote: &str,
        api_key: Option<&str>,
    ) -> Result<Option<Decimal>, ProviderError>;
}
