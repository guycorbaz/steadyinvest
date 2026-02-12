//! Current exchange rate provider with in-memory caching and DB fallback.
//!
//! Fetches live rates from the Frankfurter API (ECB data) and caches them
//! in-memory for 24 hours. When the API is unreachable, falls back to
//! stale cache, then to the latest fiscal-year rates in the database.
//!
//! **Coexists with** [`super::exchange`] which handles historical per-year
//! rates for the harvest pipeline. This module serves current rates for
//! the comparison and portfolio views.

use chrono::{DateTime, Utc};
use loco_rs::prelude::*;
use rust_decimal::Decimal;
use sea_orm::QueryOrder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;
use tokio::sync::RwLock;

use crate::models::_entities::exchange_rates;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Default cache time-to-live: 24 hours (in seconds).
const DEFAULT_CACHE_TTL_SECS: u64 = 86_400;

/// Default Frankfurter API endpoint (EUR base, CHF and USD symbols).
const FRANKFURTER_URL: &str = "https://api.frankfurter.dev/v1/latest?symbols=CHF,USD";

/// HTTP request timeout for the exchange rate provider (seconds).
const HTTP_TIMEOUT_SECS: u64 = 5;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single directional exchange rate pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRatePair {
    pub from_currency: String,
    pub to_currency: String,
    pub rate: Decimal,
}

/// Cached exchange rates with freshness tracking.
#[derive(Debug, Clone)]
pub struct CachedRates {
    pub rates: Vec<ExchangeRatePair>,
    pub fetched_at: DateTime<Utc>,
    pub rate_date: String,
}

/// JSON response from the Frankfurter API.
#[derive(Debug, Deserialize)]
struct FrankfurterResponse {
    #[allow(dead_code)]
    amount: f64,
    #[allow(dead_code)]
    base: String,
    date: String,
    rates: HashMap<String, f64>,
}

/// API response DTO for the exchange rates endpoint.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExchangeRateResponse {
    pub rates: Vec<ExchangeRatePair>,
    pub rates_as_of: String,
    pub stale: bool,
}

// ---------------------------------------------------------------------------
// Shared HTTP client and cache
// ---------------------------------------------------------------------------

/// Shared HTTP client with connection pooling and timeout.
static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .expect("HTTP client build must succeed")
});

static RATE_CACHE: LazyLock<RwLock<Option<CachedRates>>> =
    LazyLock::new(|| RwLock::new(None));

/// Returns the Frankfurter API URL, overridable via env var for testing.
fn frankfurter_url() -> String {
    std::env::var("EXCHANGE_RATE_PROVIDER_URL")
        .unwrap_or_else(|_| FRANKFURTER_URL.to_string())
}

fn cache_ttl() -> chrono::Duration {
    let secs = std::env::var("EXCHANGE_RATE_CACHE_TTL_SECS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(DEFAULT_CACHE_TTL_SECS as i64);
    chrono::Duration::seconds(secs)
}

impl CachedRates {
    fn is_fresh(&self) -> bool {
        Utc::now().signed_duration_since(self.fetched_at) < cache_ttl()
    }
}

// ---------------------------------------------------------------------------
// Rate derivation
// ---------------------------------------------------------------------------

/// Derives all 6 directional pairs from EUR→CHF and EUR→USD base rates.
/// Returns an error if either rate is zero.
fn derive_all_pairs(eur_chf: Decimal, eur_usd: Decimal) -> Result<Vec<ExchangeRatePair>> {
    if eur_chf.is_zero() || eur_usd.is_zero() {
        tracing::warn!(eur_chf = %eur_chf, eur_usd = %eur_usd, "Zero exchange rate received");
        return Err(Error::string("Exchange rate is zero — cannot derive pairs"));
    }

    let one = Decimal::ONE;
    Ok(vec![
        ExchangeRatePair {
            from_currency: "EUR".into(),
            to_currency: "CHF".into(),
            rate: eur_chf,
        },
        ExchangeRatePair {
            from_currency: "EUR".into(),
            to_currency: "USD".into(),
            rate: eur_usd,
        },
        ExchangeRatePair {
            from_currency: "CHF".into(),
            to_currency: "EUR".into(),
            rate: one / eur_chf,
        },
        ExchangeRatePair {
            from_currency: "USD".into(),
            to_currency: "EUR".into(),
            rate: one / eur_usd,
        },
        ExchangeRatePair {
            from_currency: "CHF".into(),
            to_currency: "USD".into(),
            rate: eur_usd / eur_chf,
        },
        ExchangeRatePair {
            from_currency: "USD".into(),
            to_currency: "CHF".into(),
            rate: eur_chf / eur_usd,
        },
    ])
}

// ---------------------------------------------------------------------------
// Frankfurter API fetch
// ---------------------------------------------------------------------------

async fn fetch_from_frankfurter() -> Result<CachedRates> {
    let url = frankfurter_url();
    let resp = HTTP_CLIENT
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(provider = "frankfurter", error = %e, "Exchange rate fetch failed");
            Error::string(&format!("Frankfurter API error: {e}"))
        })?
        .json::<FrankfurterResponse>()
        .await
        .map_err(|e| {
            tracing::warn!(provider = "frankfurter", error = %e, "Exchange rate parse failed");
            Error::string(&format!("Frankfurter parse error: {e}"))
        })?;

    let eur_chf_f64 = resp.rates.get("CHF")
        .ok_or_else(|| Error::string("CHF rate missing from Frankfurter response"))?;
    let eur_usd_f64 = resp.rates.get("USD")
        .ok_or_else(|| Error::string("USD rate missing from Frankfurter response"))?;

    let eur_chf = Decimal::from_f64_retain(*eur_chf_f64)
        .ok_or_else(|| Error::string("Invalid CHF rate value"))?;
    let eur_usd = Decimal::from_f64_retain(*eur_usd_f64)
        .ok_or_else(|| Error::string("Invalid USD rate value"))?;

    let pairs = derive_all_pairs(eur_chf, eur_usd)?;

    Ok(CachedRates {
        rates: pairs,
        fetched_at: Utc::now(),
        rate_date: resp.date,
    })
}

// ---------------------------------------------------------------------------
// DB fallback
// ---------------------------------------------------------------------------

/// Queries the existing `exchange_rates` table for the latest fiscal-year rates.
///
/// Returns rates with `fetched_at` set to epoch so they are always stale
/// in the cache, ensuring a Frankfurter refresh is attempted on next request.
async fn get_latest_db_rates(db: &DatabaseConnection) -> Result<Option<CachedRates>> {
    let chf_usd = exchange_rates::Entity::find()
        .filter(exchange_rates::Column::FromCurrency.eq("CHF"))
        .filter(exchange_rates::Column::ToCurrency.eq("USD"))
        .order_by_desc(exchange_rates::Column::FiscalYear)
        .one(db)
        .await?;

    let eur_usd = exchange_rates::Entity::find()
        .filter(exchange_rates::Column::FromCurrency.eq("EUR"))
        .filter(exchange_rates::Column::ToCurrency.eq("USD"))
        .order_by_desc(exchange_rates::Column::FiscalYear)
        .one(db)
        .await?;

    match (chf_usd, eur_usd) {
        (Some(chf), Some(eur)) => {
            let chf_usd_rate = chf.rate;
            let eur_usd_rate = eur.rate;

            // Derive EUR→CHF from the two base rates: EUR→CHF = EUR/USD ÷ CHF/USD
            let eur_chf = eur_usd_rate / chf_usd_rate;

            let pairs = derive_all_pairs(eur_chf, eur_usd_rate)?;
            let year = chf.fiscal_year.max(eur.fiscal_year);

            tracing::info!(
                year = year,
                "Serving exchange rates from DB fallback (fiscal year {})",
                year,
            );

            // Use epoch for fetched_at so these are always stale in cache,
            // ensuring a Frankfurter refresh is attempted on next request.
            Ok(Some(CachedRates {
                rates: pairs,
                fetched_at: DateTime::<Utc>::from_timestamp(0, 0)
                    .expect("epoch timestamp is valid"),
                rate_date: year.to_string(),
            }))
        }
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns current exchange rates using the cache-aside pattern with DB fallback.
///
/// Fallback chain:
/// 1. In-memory cache (fresh) → `stale: false`
/// 2. Frankfurter API refresh → update cache → `stale: false`
/// 3. In-memory cache (stale) → `stale: true`
/// 4. DB latest fiscal year → cache as stale → `stale: true`
/// 5. Nothing available → error
pub async fn get_rates(db: &DatabaseConnection) -> Result<ExchangeRateResponse> {
    // Step 1: Check in-memory cache (read lock)
    {
        let cache = RATE_CACHE.read().await;
        if let Some(ref cached) = *cache {
            if cached.is_fresh() {
                return Ok(ExchangeRateResponse {
                    rates: cached.rates.clone(),
                    rates_as_of: cached.rate_date.clone(),
                    stale: false,
                });
            }
        }
    }

    // Step 2: Attempt Frankfurter refresh (write lock)
    let mut cache = RATE_CACHE.write().await;

    // Double-check: another request may have refreshed while waiting for write lock
    if let Some(ref cached) = *cache {
        if cached.is_fresh() {
            return Ok(ExchangeRateResponse {
                rates: cached.rates.clone(),
                rates_as_of: cached.rate_date.clone(),
                stale: false,
            });
        }
    }

    match fetch_from_frankfurter().await {
        Ok(fresh) => {
            let response = ExchangeRateResponse {
                rates: fresh.rates.clone(),
                rates_as_of: fresh.rate_date.clone(),
                stale: false,
            };
            *cache = Some(fresh);
            Ok(response)
        }
        Err(_) => {
            // Step 3: Return stale in-memory cache if available
            if let Some(ref stale) = *cache {
                return Ok(ExchangeRateResponse {
                    rates: stale.rates.clone(),
                    rates_as_of: stale.rate_date.clone(),
                    stale: true,
                });
            }

            // Step 4: DB fallback — store in cache to avoid repeated DB queries
            if let Some(db_rates) = get_latest_db_rates(db).await? {
                let response = ExchangeRateResponse {
                    rates: db_rates.rates.clone(),
                    rates_as_of: db_rates.rate_date.clone(),
                    stale: true,
                };
                *cache = Some(db_rates);
                Ok(response)
            } else {
                // Step 5: Nothing available
                Err(Error::string("Exchange rate data is temporarily unavailable"))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Test helpers (unconditionally public for integration tests)
// ---------------------------------------------------------------------------

/// Seeds the in-memory cache with provided rates.
pub async fn seed_cache(rates: CachedRates) {
    let mut cache = RATE_CACHE.write().await;
    *cache = Some(rates);
}

/// Clears the in-memory cache.
pub async fn clear_cache() {
    let mut cache = RATE_CACHE.write().await;
    *cache = None;
}
