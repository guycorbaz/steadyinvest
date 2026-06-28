//! EODHD adapter (https://eodhd.com) — CH/EU+US coverage. Story 3.1, first adapter.
//!
//! Two endpoints feed one [`RawFinancials`]:
//! - `/fundamentals/{ticker}` → currency, per-year income-statement / balance-sheet / earnings,
//!   declared splits;
//! - `/eod/{ticker}` (daily OHLC) → each fiscal year's high/low, reduced from the daily bars.
//!
//! The **pure mapping** [`map_eodhd`] (no I/O) is the CI-tested heart; the HTTP layer is thin and
//! validated by a manual GO/NO-GO with a real key (no network in CI). The assumed JSON shape
//! follows EODHD's documented structure — the manual run confirms fidelity to a live response.

use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::Client;
use rust_decimal::Decimal;
use serde_json::Value;
use steadyinvest_core::normalize::{RawAmount, RawFinancials, RawYear, SplitEvent};

use crate::error::ProviderError;
use crate::provider::{MarketDataProvider, RawFetch};

const DEFAULT_BASE_URL: &str = "https://eodhd.com/api";

/// The EODHD HTTP adapter. `base_url` is injectable so tests / a mock can point elsewhere.
pub struct EodhdProvider {
    http: Client,
    base_url: String,
}

impl EodhdProvider {
    /// A provider against the live EODHD API.
    pub fn new() -> Self {
        EodhdProvider {
            http: build_client(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    /// A provider against an arbitrary base URL (a mock server in an integration test).
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        EodhdProvider {
            http: build_client(),
            base_url: base_url.into(),
        }
    }

    async fn get_json(&self, url: &str, ticker: &str) -> Result<Value, ProviderError> {
        // NFR-S1: the request URL carries `?api_token=…`. `reqwest::Error`'s Display can include the
        // URL, so `.without_url()` is MANDATORY before stringifying — otherwise the key would leak
        // into `ProviderError::{Network,Parse}` detail and on into a user-facing notice.
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| ProviderError::Network {
                detail: e.without_url().to_string(),
            })?;
        let status = resp.status();
        if status.is_success() {
            return resp
                .json::<Value>()
                .await
                .map_err(|e| ProviderError::Parse {
                    detail: e.without_url().to_string(),
                });
        }
        // A 403 means the key is valid but the account/plan is not authorized for this resource
        // (e.g. EODHD's free tier excludes /fundamentals). Surface the provider's own reason — far
        // more honest than "key invalid" — capped, and key-free (the body never carries the token).
        if status.as_u16() == 403 {
            let detail = resp.text().await.unwrap_or_default();
            let detail: String = detail.trim().chars().take(200).collect();
            return Err(ProviderError::Forbidden { detail });
        }
        Err(classify_status(status.as_u16(), ticker))
    }
}

/// The shared `reqwest` client with sane timeouts (#39). Without these, a hung connection never
/// resolves, so the off-thread fetch/key-test never returns and the UI latches "Récupération…" /
/// "Test… en cours" with no recovery. A connect + overall-request bound guarantees every job
/// terminates (as a `Network` error on timeout — cause-named by Story 3.5). Falls back to the
/// default client if the builder fails (only on a TLS-backend init error — same panic surface as
/// `Client::new()`).
fn build_client() -> Client {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("the reqwest client builds with timeouts")
}

impl Default for EodhdProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MarketDataProvider for EodhdProvider {
    async fn fetch_fundamentals(
        &self,
        ticker: &str,
        api_key: Option<&str>,
    ) -> Result<RawFetch, ProviderError> {
        // EODHD requires a token; `demo` works only for AAPL.US. A keyless request is unauthenticated.
        let token = api_key.ok_or(ProviderError::InvalidOrAbsentKey)?;
        let fundamentals_url = format!(
            "{}/fundamentals/{ticker}?api_token={token}&fmt=json",
            self.base_url
        );
        let eod_url = format!(
            "{}/eod/{ticker}?api_token={token}&period=d&fmt=json&order=a",
            self.base_url
        );
        let fundamentals = self.get_json(&fundamentals_url, ticker).await?;
        let prices = self.get_json(&eod_url, ticker).await?;
        let financials = map_eodhd(&fundamentals, &prices, ticker)?;
        // Story 4.4: the latest `/eod` close (the series is `order=a`, so the last bar is the most
        // recent) is the present market price for the §4 zone marker — `None` if the series is empty.
        let latest_price = latest_eod_close(&prices);
        Ok(RawFetch {
            financials,
            latest_price,
        })
    }

    async fn fetch_latest_price(
        &self,
        ticker: &str,
        api_key: Option<&str>,
    ) -> Result<Option<Decimal>, ProviderError> {
        // Issue #50: hit ONLY `/eod` (no `/fundamentals`) — works on the free EODHD tier, which
        // allows EOD but 403s fundamentals. The series is `order=a`, so the last bar is the latest.
        let token = api_key.ok_or(ProviderError::InvalidOrAbsentKey)?;
        let eod_url = format!(
            "{}/eod/{ticker}?api_token={token}&period=d&fmt=json&order=a",
            self.base_url
        );
        let prices = self.get_json(&eod_url, ticker).await?;
        Ok(latest_eod_close(&prices))
    }
}

/// The most recent close from the daily EOD array (Story 4.4). The series is requested `order=a`
/// (ascending), so the **last** bar is today's; we read its `close` (raw — comparable to the §4
/// forecast band, which is in present price terms). `None` when the array is empty/missing.
pub fn latest_eod_close(prices: &Value) -> Option<Decimal> {
    let bars = prices.as_array()?;
    let last = bars.last()?;
    dec(last.get("close"))
}

/// HTTP status → cause-named [`ProviderError`]. (403 is handled in `get_json` with the body, so it
/// never reaches here — only 401 maps to an invalid/absent key.)
fn classify_status(status: u16, ticker: &str) -> ProviderError {
    match status {
        401 => ProviderError::InvalidOrAbsentKey,
        404 => ProviderError::TickerNotFound {
            ticker: ticker.to_string(),
        },
        429 => ProviderError::Quota {
            retry_after_secs: None,
        },
        s => ProviderError::Network {
            detail: format!("provider responded with HTTP status {s}"),
        },
    }
}

/// PURE: EODHD `/fundamentals` + `/eod` JSON → [`RawFinancials`]. No I/O. Missing fields stay
/// `None` (never coerced to 0). The caller passes this straight to `core::normalize`.
pub fn map_eodhd(
    fundamentals: &Value,
    prices: &Value,
    ticker: &str,
) -> Result<RawFinancials, ProviderError> {
    let currency = fundamentals
        .pointer("/General/CurrencyCode")
        .and_then(Value::as_str)
        .ok_or_else(|| ProviderError::Parse {
            detail: format!("missing General.CurrencyCode for {ticker}"),
        })?
        .to_string();

    let income = obj(fundamentals.pointer("/Financials/Income_Statement/yearly"));
    let balance = obj(fundamentals.pointer("/Financials/Balance_Sheet/yearly"));
    let earnings = obj(fundamentals.pointer("/Earnings/Annual"));

    // Per-year high/low reduced from the daily EOD bars.
    let (highs, lows) = reduce_eod_high_low(prices);

    // Union of every fiscal year mentioned by any section, ascending.
    let mut years_set: BTreeMap<i32, ()> = BTreeMap::new();
    for key in income.keys().chain(balance.keys()).chain(earnings.keys()) {
        if let Some(y) = year_of_date_key(key) {
            years_set.insert(y, ());
        }
    }
    for y in highs.keys().chain(lows.keys()) {
        years_set.insert(*y, ());
    }

    let amount = |d: Option<Decimal>| {
        d.map(|value| RawAmount {
            value,
            currency: currency.clone(),
        })
    };

    let years = years_set
        .keys()
        .map(|&y| {
            let inc = year_row(income, y);
            let bal = year_row(balance, y);
            let earn = year_row(earnings, y);
            RawYear {
                year: y,
                period_months: None,
                fiscal_year_end_month: None,
                sales: amount(field_dec(inc, "totalRevenue")),
                eps: amount(field_dec(earn, "epsActual")),
                high_price: amount(highs.get(&y).copied()),
                low_price: amount(lows.get(&y).copied()),
                dividend_per_share: None, // EODHD per-share dividends come from a later endpoint (#21 / Story 3.x)
                pre_tax_profit: amount(field_dec(inc, "incomeBeforeTax")),
                net_profit: amount(field_dec(inc, "netIncome")),
                tax_rate: None, // pre_tax_profit is reported directly → no gross-up needed
                book_value_per_share: amount(book_value_per_share(bal)),
            }
        })
        .collect();

    let splits = map_splits(fundamentals);

    Ok(RawFinancials {
        native_currency: currency,
        years,
        splits,
    })
}

/// `SplitsDividends.Splits` is `{ "YYYY-MM-DD": "num/den" }` (e.g. `"4.000000/1.000000"`).
fn map_splits(fundamentals: &Value) -> Vec<SplitEvent> {
    let raw = obj(fundamentals.pointer("/SplitsDividends/Splits"));
    let mut out = Vec::new();
    for (date_key, v) in raw {
        let (Some(year), Some(s)) = (year_of_date_key(date_key), v.as_str()) else {
            continue;
        };
        let mut parts = s.split('/');
        let num = parts.next().and_then(parse_split_part);
        let den = parts.next().and_then(parse_split_part);
        if let (Some(numerator), Some(denominator)) = (num, den) {
            if numerator > 0 && denominator > 0 {
                out.push(SplitEvent {
                    effective_year: year,
                    numerator,
                    denominator,
                });
            }
        }
    }
    out.sort_by_key(|s| s.effective_year);
    out
}

/// EODHD writes split ratios as decimals ("4.000000"); take the integer part.
fn parse_split_part(s: &str) -> Option<u32> {
    s.trim().split('.').next()?.parse::<u32>().ok()
}

/// Reduce the daily EOD array into per-year max(high) and min(low).
fn reduce_eod_high_low(prices: &Value) -> (BTreeMap<i32, Decimal>, BTreeMap<i32, Decimal>) {
    let mut highs: BTreeMap<i32, Decimal> = BTreeMap::new();
    let mut lows: BTreeMap<i32, Decimal> = BTreeMap::new();
    let Some(bars) = prices.as_array() else {
        return (highs, lows);
    };
    for bar in bars {
        let Some(year) = bar
            .get("date")
            .and_then(Value::as_str)
            .and_then(year_of_date_key)
        else {
            continue;
        };
        if let Some(high) = dec(bar.get("high")) {
            highs
                .entry(year)
                .and_modify(|m| {
                    if high > *m {
                        *m = high;
                    }
                })
                .or_insert(high);
        }
        if let Some(low) = dec(bar.get("low")) {
            lows.entry(year)
                .and_modify(|m| {
                    if low < *m {
                        *m = low;
                    }
                })
                .or_insert(low);
        }
    }
    (highs, lows)
}

/// `totalStockholderEquity / commonStockSharesOutstanding` when both are present and shares ≠ 0.
fn book_value_per_share(year: Option<&Value>) -> Option<Decimal> {
    let equity = field_dec(year, "totalStockholderEquity")?;
    let shares = field_dec(year, "commonStockSharesOutstanding")?;
    if shares.is_zero() {
        return None;
    }
    equity.checked_div(shares)
}

// ── small JSON helpers ────────────────────────────────────────────────────────────────────────

/// Borrow a JSON object, or an empty static map when the pointer missed / isn't an object.
fn obj(v: Option<&Value>) -> &serde_json::Map<String, Value> {
    static EMPTY: std::sync::OnceLock<serde_json::Map<String, Value>> = std::sync::OnceLock::new();
    v.and_then(Value::as_object)
        .unwrap_or_else(|| EMPTY.get_or_init(serde_json::Map::new))
}

/// The row of a `{date: {...}}` yearly map whose date key falls in fiscal year `y`.
fn year_row(map: &serde_json::Map<String, Value>, y: i32) -> Option<&Value> {
    map.iter()
        .find(|(k, _)| year_of_date_key(k) == Some(y))
        .map(|(_, v)| v)
}

/// A named field of an optional row, parsed as a `Decimal` (number or numeric string).
fn field_dec(row: Option<&Value>, field: &str) -> Option<Decimal> {
    dec(row?.get(field))
}

/// Parse a `Decimal` from a JSON value that is a number or a numeric string. Never uses `f64`.
fn dec(v: Option<&Value>) -> Option<Decimal> {
    match v? {
        Value::String(s) => Decimal::from_str_exact(s.trim()).ok(),
        Value::Number(n) => Decimal::from_str_exact(&n.to_string()).ok(),
        _ => None,
    }
}

/// Leading `YYYY` of a `"YYYY-MM-DD"` date key → fiscal year.
fn year_of_date_key(key: &str) -> Option<i32> {
    key.get(0..4)?.parse::<i32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn latest_eod_close_reads_the_last_bar_in_ascending_order() {
        // `/eod?order=a` → ascending; the LAST bar is the most recent close (the present price for
        // the §4 zone marker, Story 4.4). Parsed exactly — never via `f64`.
        let prices = json!([
            { "date": "2026-06-25", "close": "101.5" },
            { "date": "2026-06-26", "close": "103.25" },
        ]);
        assert_eq!(
            latest_eod_close(&prices),
            Some(Decimal::from_str_exact("103.25").unwrap())
        );
    }

    #[test]
    fn latest_eod_close_is_none_when_the_series_is_empty_or_malformed() {
        assert_eq!(latest_eod_close(&json!([])), None);
        assert_eq!(latest_eod_close(&json!({})), None);
        // Last bar present but no `close` field → no price, not a zero.
        assert_eq!(latest_eod_close(&json!([{ "date": "2026-06-26" }])), None);
    }
}
