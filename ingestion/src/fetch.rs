//! Fetch orchestration: a provider → `core::normalize` → [`FetchedFinancials`].
//!
//! Enum dispatch ([`Provider`]) keeps `MarketDataProvider`'s `async fn` usable without `dyn`
//! (native async-fn-in-trait is not dyn-compatible) and without an `async-trait` dependency.

use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use steadyinvest_core::normalize::{CanonicalFinancials, RawFinancials, normalize};

use crate::error::{IngestionError, ProviderError};
use crate::provider::{MarketDataProvider, RawFetch};

/// The normalized result of a fetch plus the **dependency digest** (#21): a SHA-256 over the
/// provider + ticker + the value-normalized canonical decimals. The app stamps this into each
/// provider cell's `provenance.hash_of_dependencies`, replacing the manual `"manual"` placeholder.
///
/// `latest_price` (Story 4.4) is the present market price (the latest `/eod` close), used to fill
/// `judgment.current_price` so the §4 zone recomputes — it is **not** part of the canonical calc and
/// is deliberately **excluded** from `digest` (a moving close must not churn the dependency hash).
#[derive(Debug, Clone)]
pub struct FetchedFinancials {
    pub canonical: CanonicalFinancials,
    pub digest: String,
    pub latest_price: Option<Decimal>,
    /// Trailing-twelve-months EPS (Issue #113) — the current-P/E denominator. A present market fact
    /// (like `latest_price`), excluded from `digest` (a moving figure must not churn the hash).
    pub ttm_eps: Option<Decimal>,
}

/// Which field a fetch serves (Story 6.9, FR26) — the fallback chain is configured PER field
/// type, because provider capabilities differ (see [`supports`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// The latest market close (`fetch_latest_price` — issue #50's price-only path).
    Price,
    /// The fundamentals payload (`fetch_fundamentals` → `normalize`).
    Fundamentals,
    /// A BASE→QUOTE exchange rate (`fetch_fx_rate`, Story 6.5).
    Fx,
}

/// Whether the provider identified by `tag` can serve `field` (Story 6.9, FR26) — a DECLARED
/// capability table, not runtime discovery: Twelve Data's fundamentals fetch returns its
/// financial fields as `None` by design (Story 7.4, free tier), a success-but-empty answer that
/// would never trigger error-based failover — so a fallback chain must skip it up front for
/// fundamentals. Keyed by the stable provenance tag so the app's config layer can consult it
/// without constructing adapters. Unknown tags support nothing (defensive).
pub fn supports(tag: &str, field: FieldKind) -> bool {
    match (tag, field) {
        ("twelvedata", FieldKind::Fundamentals) => false,
        ("eodhd" | "twelvedata" | "fake", _) => true,
        _ => false,
    }
}

/// The provider's DECLARED minimum spacing between consecutive requests (Story 6.9, FR27) — a
/// documented fact about the provider, not user config: Twelve Data's free tier allows 8
/// requests/minute → 7500 ms; EODHD's quota is daily, so 200 ms is courtesy spacing; the fake
/// (and anything unknown) is unpaced. The worker's single loop enforces it per tag.
pub fn min_request_interval(tag: &str) -> std::time::Duration {
    match tag {
        "twelvedata" => std::time::Duration::from_millis(7500),
        "eodhd" => std::time::Duration::from_millis(200),
        _ => std::time::Duration::ZERO,
    }
}

/// Concrete providers, dispatched by enum so the `async fn` trait needs no `dyn`/`async-trait`.
pub enum Provider {
    Eodhd(crate::adapters::eodhd::EodhdProvider),
    /// Twelve Data (Story 7.4) — the price-led second source.
    TwelveData(crate::adapters::twelvedata::TwelveDataProvider),
    /// A deterministic, offline test double (kept in the public API so `app` tests can use it).
    Fake(FakeProvider),
}

impl Provider {
    /// A stable provenance tag for the dependency digest (Story 7.4) — distinguishes which provider a
    /// fetched value came from, so two sources never collide on an identical ticker/value set.
    pub fn tag(&self) -> &'static str {
        match self {
            Provider::Eodhd(_) => "eodhd",
            Provider::TwelveData(_) => "twelvedata",
            Provider::Fake(_) => "fake",
        }
    }

    /// [`supports`] keyed by this provider's tag (Story 6.9).
    pub fn supports(&self, field: FieldKind) -> bool {
        supports(self.tag(), field)
    }

    /// [`min_request_interval`] keyed by this provider's tag (Story 6.9, FR27).
    pub fn min_request_interval(&self) -> std::time::Duration {
        min_request_interval(self.tag())
    }
}

impl MarketDataProvider for Provider {
    async fn fetch_fundamentals(
        &self,
        ticker: &str,
        api_key: Option<&str>,
    ) -> Result<RawFetch, ProviderError> {
        match self {
            Provider::Eodhd(p) => p.fetch_fundamentals(ticker, api_key).await,
            Provider::TwelveData(p) => p.fetch_fundamentals(ticker, api_key).await,
            Provider::Fake(p) => p.fetch_fundamentals(ticker, api_key).await,
        }
    }

    async fn fetch_latest_price(
        &self,
        ticker: &str,
        api_key: Option<&str>,
    ) -> Result<Option<Decimal>, ProviderError> {
        match self {
            Provider::Eodhd(p) => p.fetch_latest_price(ticker, api_key).await,
            Provider::TwelveData(p) => p.fetch_latest_price(ticker, api_key).await,
            Provider::Fake(p) => p.fetch_latest_price(ticker, api_key).await,
        }
    }

    async fn fetch_fx_rate(
        &self,
        base: &str,
        quote: &str,
        api_key: Option<&str>,
    ) -> Result<Option<Decimal>, ProviderError> {
        match self {
            Provider::Eodhd(p) => p.fetch_fx_rate(base, quote, api_key).await,
            Provider::TwelveData(p) => p.fetch_fx_rate(base, quote, api_key).await,
            Provider::Fake(p) => p.fetch_fx_rate(base, quote, api_key).await,
        }
    }
}

/// Fetch a ticker, normalize it through `core`, and compute the dependency digest. The latest price
/// (Story 4.4) rides through alongside the canonical result, untouched by `normalize`.
pub async fn fetch_canonical(
    provider: &Provider,
    ticker: &str,
    api_key: Option<&str>,
) -> Result<FetchedFinancials, IngestionError> {
    let RawFetch {
        financials,
        latest_price,
        ttm_eps,
    } = provider.fetch_fundamentals(ticker, api_key).await?;
    let canonical = normalize(financials)?;
    let digest = dependency_digest(provider.tag(), ticker, &canonical);
    Ok(FetchedFinancials {
        canonical,
        digest,
        latest_price,
        ttm_eps,
    })
}

/// Fetch ONLY the latest market price (issue #50) — the holdings price-refresh path. No
/// `/fundamentals`, no `normalize`, no digest: just the present `/eod` close, so it works on plans
/// where fundamentals are forbidden but EOD is allowed (the free EODHD tier).
pub async fn fetch_price(
    provider: &Provider,
    ticker: &str,
    api_key: Option<&str>,
) -> Result<Option<Decimal>, IngestionError> {
    Ok(provider.fetch_latest_price(ticker, api_key).await?)
}

/// Fetch the latest BASE→QUOTE exchange rate (Story 6.5, FR28) — the FX sibling of [`fetch_price`],
/// with the same error-conversion shape. User-initiated only (FR65). Each adapter spells the pair
/// in its own symbol convention and reuses its latest-price wire path; `Ok(None)` when the provider
/// has no quote for the pair (never an inverted-pair guess — the app refuses honestly instead).
pub async fn fetch_fx_rate(
    provider: &Provider,
    base: &str,
    quote: &str,
    api_key: Option<&str>,
) -> Result<Option<Decimal>, IngestionError> {
    Ok(provider.fetch_fx_rate(base, quote, api_key).await?)
}

/// `"{provider_tag}:{sha256-hex}"` — the tag as a READABLE prefix (Story 6.9, FR26: the effective
/// provider is recorded on every refreshed cell via `provenance.hash_of_dependencies`, and a
/// human must be able to read WHICH provider without reversing a hash) over each canonical year's
/// value-normalized decimals (so `"3.0"` and `"3"` hash identically — `Money`/`Decimal` value
/// equality, not byte equality). The tag also feeds the hash (Story 7.4), so two providers'
/// digests stay distinct even for identical values, and a provider switch is observable as a
/// dependency change. Format note: pre-6.9 cells hold the bare 64-char hex — their first
/// post-upgrade refresh reads as a dependency change (visible, honest, one-time).
pub fn dependency_digest(
    provider_tag: &str,
    ticker: &str,
    canonical: &CanonicalFinancials,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(provider_tag.as_bytes());
    hasher.update(b":");
    hasher.update(ticker.as_bytes());
    for year in &canonical.years {
        hasher.update(year.year.to_le_bytes());
        for field in [
            year.sales,
            year.eps,
            year.high_price,
            year.low_price,
            year.dividend_per_share,
            year.pre_tax_profit,
            year.book_value_per_share,
        ] {
            match field {
                Some(d) => {
                    hasher.update([1u8]);
                    hasher.update(d.normalize().to_string().as_bytes());
                }
                None => hasher.update([0u8]),
            }
        }
    }
    let bytes = hasher.finalize();
    let mut hex = String::with_capacity(provider_tag.len() + 1 + bytes.len() * 2);
    hex.push_str(provider_tag);
    hex.push(':');
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// A canned, offline provider for tests (`app` and `ingestion`): returns a fixed `RawFinancials`
/// (+ an optional latest price, Story 4.4) or a fixed [`ProviderError`], ignoring the ticker/key.
pub struct FakeProvider {
    result: Result<RawFinancials, ProviderError>,
    latest_price: Option<Decimal>,
    /// The canned BASE→QUOTE rate `fetch_fx_rate` reports (Story 6.5) — `None` = "no quote for the
    /// pair", the honest provider answer; the canned `result`'s error still wins.
    fx_rate: Option<Decimal>,
    /// The canned trailing-twelve-months EPS (Issue #113) — drives current-P/E tests.
    ttm_eps: Option<Decimal>,
}

impl FakeProvider {
    /// A fake with no latest price (the pre-4.4 shape — existing callers keep `latest_price = None`).
    pub fn returning(result: Result<RawFinancials, ProviderError>) -> Self {
        FakeProvider {
            result,
            latest_price: None,
            fx_rate: None,
            ttm_eps: None,
        }
    }

    /// A fake that also reports a latest market price (Story 4.4 — drives the §4 zone in app tests).
    pub fn returning_with_price(
        result: Result<RawFinancials, ProviderError>,
        latest_price: Option<Decimal>,
    ) -> Self {
        FakeProvider {
            result,
            latest_price,
            fx_rate: None,
            ttm_eps: None,
        }
    }

    /// Give the fake a canned FX rate (Story 6.5 — drives the fx-refresh path in `app` tests).
    /// Builder-style so the existing constructors stay untouched.
    pub fn with_fx_rate(mut self, fx_rate: Option<Decimal>) -> Self {
        self.fx_rate = fx_rate;
        self
    }

    /// Give the fake a canned TTM EPS (Issue #113 — drives current-P/E tests). Builder-style.
    pub fn with_ttm_eps(mut self, ttm_eps: Option<Decimal>) -> Self {
        self.ttm_eps = ttm_eps;
        self
    }
}

impl MarketDataProvider for FakeProvider {
    async fn fetch_fundamentals(
        &self,
        _ticker: &str,
        _api_key: Option<&str>,
    ) -> Result<RawFetch, ProviderError> {
        self.result.clone().map(|financials| RawFetch {
            financials,
            latest_price: self.latest_price,
            ttm_eps: self.ttm_eps,
        })
    }

    async fn fetch_latest_price(
        &self,
        _ticker: &str,
        _api_key: Option<&str>,
    ) -> Result<Option<Decimal>, ProviderError> {
        // Mirror the canned result's success/failure, handing back the configured latest price.
        self.result.clone().map(|_| self.latest_price)
    }

    async fn fetch_fx_rate(
        &self,
        _base: &str,
        _quote: &str,
        _api_key: Option<&str>,
    ) -> Result<Option<Decimal>, ProviderError> {
        // Mirror the canned result's success/failure, handing back the configured FX rate
        // (`None` = the provider has no quote for the pair).
        self.result.clone().map(|_| self.fx_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use steadyinvest_core::normalize::{RawAmount, RawYear};

    // ── Story 6.9 — declared capabilities + pacing (FR26/FR27) ──

    #[test]
    fn twelvedata_declares_no_fundamentals_and_everything_else_serves_all() {
        assert!(!supports("twelvedata", FieldKind::Fundamentals));
        assert!(supports("twelvedata", FieldKind::Price));
        assert!(supports("twelvedata", FieldKind::Fx));
        for field in [FieldKind::Price, FieldKind::Fundamentals, FieldKind::Fx] {
            assert!(supports("eodhd", field));
            assert!(supports("fake", field));
            assert!(!supports("unknown", field), "unknown tags support nothing");
        }
    }

    #[test]
    fn declared_pacing_matches_the_documented_tiers() {
        use std::time::Duration;
        assert_eq!(
            min_request_interval("twelvedata"),
            Duration::from_millis(7500),
            "free tier: 8 requests/minute"
        );
        assert_eq!(min_request_interval("eodhd"), Duration::from_millis(200));
        assert_eq!(min_request_interval("fake"), Duration::ZERO);
        assert_eq!(min_request_interval("unknown"), Duration::ZERO);
    }

    fn raw(value: &str) -> RawFinancials {
        let amt = |v: &str| {
            Some(RawAmount {
                value: Decimal::from_str_exact(v).unwrap(),
                currency: "USD".into(),
            })
        };
        RawFinancials {
            native_currency: "USD".into(),
            years: vec![RawYear {
                sales: amt(value),
                eps: amt("1"),
                high_price: amt("20"),
                low_price: amt("10"),
                ..RawYear::empty(2024)
            }],
            splits: vec![],
        }
    }

    #[tokio::test]
    async fn fetch_canonical_normalizes_and_digests_a_fake_provider() {
        let provider = Provider::Fake(FakeProvider::returning(Ok(raw("100"))));
        let fetched = fetch_canonical(&provider, "AAPL.US", Some("key"))
            .await
            .expect("fake fetch normalizes");
        assert_eq!(fetched.canonical.years.len(), 1);
        assert_eq!(fetched.canonical.years[0].sales, Some(Decimal::from(100)));
        assert_eq!(
            fetched.digest,
            format!("fake:{}", &fetched.digest["fake:".len()..]),
            "the effective provider is a READABLE digest prefix (FR26)"
        );
        assert_eq!(
            fetched.digest.len(),
            "fake:".len() + 64,
            "tag + ':' + sha-256 hex"
        );
    }

    #[tokio::test]
    async fn digest_is_value_normalized_not_byte_sensitive() {
        // "100" vs "100.00" are the same value → identical digest.
        let a = Provider::Fake(FakeProvider::returning(Ok(raw("100"))));
        let b = Provider::Fake(FakeProvider::returning(Ok(raw("100.00"))));
        let da = fetch_canonical(&a, "AAPL.US", Some("k"))
            .await
            .unwrap()
            .digest;
        let db = fetch_canonical(&b, "AAPL.US", Some("k"))
            .await
            .unwrap()
            .digest;
        assert_eq!(da, db);
    }

    #[tokio::test]
    async fn provider_error_propagates_as_ingestion_error() {
        let provider = Provider::Fake(FakeProvider::returning(Err(
            ProviderError::InvalidOrAbsentKey,
        )));
        let err = fetch_canonical(&provider, "AAPL.US", None)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            IngestionError::Provider(ProviderError::InvalidOrAbsentKey)
        ));
    }

    #[tokio::test]
    async fn fetch_canonical_threads_the_fake_latest_price_without_touching_the_digest() {
        // Story 4.4: a settable `latest_price` rides `fetch_canonical` → `FetchedFinancials`,
        // untouched by `normalize` and EXCLUDED from the dependency digest (a moving close must not
        // churn the hash). Exercises `FakeProvider::returning_with_price` end-to-end.
        let price = Decimal::from(42);
        let with_price = Provider::Fake(FakeProvider::returning_with_price(
            Ok(raw("100")),
            Some(price),
        ));
        let fetched = fetch_canonical(&with_price, "AAPL.US", Some("key"))
            .await
            .unwrap();
        assert_eq!(fetched.latest_price, Some(price));

        // The default fake carries no price → `None` (the pre-4.4 shape).
        let no_price = Provider::Fake(FakeProvider::returning(Ok(raw("100"))));
        let plain = fetch_canonical(&no_price, "AAPL.US", Some("key"))
            .await
            .unwrap();
        assert_eq!(plain.latest_price, None);

        // Same canonical financials → same digest, regardless of `latest_price`.
        assert_eq!(fetched.digest, plain.digest);
    }

    #[tokio::test]
    async fn fetch_fx_rate_dispatches_and_mirrors_provider_errors() {
        // Story 6.5: the FX sibling of `fetch_price` — the enum dispatch + the wrapper's
        // error-conversion shape, driven through the fake.
        let rate = Decimal::from_str_exact("0.9312").unwrap();
        let ok = Provider::Fake(FakeProvider::returning(Ok(raw("100"))).with_fx_rate(Some(rate)));
        assert_eq!(
            fetch_fx_rate(&ok, "EUR", "CHF", Some("k")).await.unwrap(),
            Some(rate)
        );

        // A fake without a canned rate = the provider has no quote for the pair → Ok(None).
        let none = Provider::Fake(FakeProvider::returning(Ok(raw("100"))));
        assert_eq!(
            fetch_fx_rate(&none, "EUR", "CHF", Some("k")).await.unwrap(),
            None
        );

        // A provider failure propagates as an IngestionError, exactly like `fetch_price`.
        let err = Provider::Fake(
            FakeProvider::returning(Err(ProviderError::InvalidOrAbsentKey))
                .with_fx_rate(Some(rate)),
        );
        assert!(matches!(
            fetch_fx_rate(&err, "EUR", "CHF", None).await.unwrap_err(),
            IngestionError::Provider(ProviderError::InvalidOrAbsentKey)
        ));
    }

    #[tokio::test]
    async fn fetch_price_returns_the_latest_price_and_mirrors_provider_errors() {
        // Issue #50: the price-only path returns the latest close (no fundamentals/normalize/digest).
        let price = Decimal::from(42);
        let ok = Provider::Fake(FakeProvider::returning_with_price(
            Ok(raw("100")),
            Some(price),
        ));
        assert_eq!(
            fetch_price(&ok, "AAPL.US", Some("k")).await.unwrap(),
            Some(price)
        );

        // A provider failure (e.g. an unauthenticated request) propagates as an IngestionError.
        let err = Provider::Fake(FakeProvider::returning_with_price(
            Err(ProviderError::InvalidOrAbsentKey),
            Some(price),
        ));
        assert!(matches!(
            fetch_price(&err, "AAPL.US", None).await.unwrap_err(),
            IngestionError::Provider(ProviderError::InvalidOrAbsentKey)
        ));
    }
}
