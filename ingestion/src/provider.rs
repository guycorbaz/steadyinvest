//! The `MarketDataProvider` trait — the provider-agnostic acquisition boundary (FR15).
//!
//! An adapter's single job is `fetch a ticker → core::normalize::RawFinancials`. Normalization
//! (IFRS↔GAAP, splits, fiscal period, currency) is NOT the adapter's concern — it happens once,
//! centrally, in `core::normalize` (called by [`crate::fetch_canonical`]).
//!
//! Keys are **injected by the caller** (`app`, from the OS keychain in Story 3.2; from an env var
//! as the 3.1 interim) and passed as `Option<&str>` — `None` for keyless providers. A provider
//! never reads a key from disk, the journal, or the environment itself.

use steadyinvest_core::normalize::RawFinancials;

use crate::error::ProviderError;

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
    /// Fetch a ticker's raw annual series. `api_key` is `None` for keyless providers.
    async fn fetch_fundamentals(
        &self,
        ticker: &str,
        api_key: Option<&str>,
    ) -> Result<RawFinancials, ProviderError>;
}
