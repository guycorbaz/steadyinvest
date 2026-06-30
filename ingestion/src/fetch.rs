//! Fetch orchestration: a provider → `core::normalize` → [`FetchedFinancials`].
//!
//! Enum dispatch ([`Provider`]) keeps `MarketDataProvider`'s `async fn` usable without `dyn`
//! (native async-fn-in-trait is not dyn-compatible) and without an `async-trait` dependency.

use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use steadyinvest_core::normalize::{normalize, CanonicalFinancials, RawFinancials};

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
    } = provider.fetch_fundamentals(ticker, api_key).await?;
    let canonical = normalize(financials)?;
    let digest = dependency_digest(provider.tag(), ticker, &canonical);
    Ok(FetchedFinancials {
        canonical,
        digest,
        latest_price,
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

/// SHA-256 hex over `"{provider_tag}:{ticker}"` + each canonical year's value-normalized decimals (so
/// `"3.0"` and `"3"` hash identically — `Money`/`Decimal` value equality, not byte equality). The
/// provider tag (Story 7.4) keeps two providers' digests distinct for the same ticker/values, so the
/// data's provenance is honest and a provider switch is observable as a dependency change.
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
    let mut hex = String::with_capacity(bytes.len() * 2);
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
}

impl FakeProvider {
    /// A fake with no latest price (the pre-4.4 shape — existing callers keep `latest_price = None`).
    pub fn returning(result: Result<RawFinancials, ProviderError>) -> Self {
        FakeProvider {
            result,
            latest_price: None,
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
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use steadyinvest_core::normalize::{RawAmount, RawYear};

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
        assert_eq!(fetched.digest.len(), 64, "sha-256 hex is 64 chars");
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
