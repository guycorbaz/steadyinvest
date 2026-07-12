//! Twelve Data adapter (https://twelvedata.com) — the second provider (Story 7.4), added to give a
//! redundant **price** source alongside EODHD (the Epic-4-retro D2 resolution). Twelve Data's free
//! tier serves EOD prices for SIX/global equities; its **fundamentals are mostly paid**, so this
//! adapter is **price-led**: `fetch_latest_price` is the core path (holdings refresh, Story-5.1 price
//! history), and `fetch_fundamentals` returns price-derived per-year high/low with the financial
//! fields left `None` for manual entry.
//!
//! Twelve Data reports some errors as a **200 body** `{"status":"error","code":N,"message":…}` rather
//! than an HTTP status, so [`classify_twelvedata`] inspects the body. The **pure** mappings
//! ([`map_twelvedata`], [`classify_twelvedata`], [`price_of`]) are the CI-tested heart; the HTTP layer
//! is thin and validated by a manual GO/NO-GO with a real key (no network in CI).

use std::collections::BTreeMap;

use reqwest::Client;
use rust_decimal::Decimal;
use serde_json::Value;
use steadyinvest_core::normalize::{RawAmount, RawFinancials, RawYear};

use crate::adapters::common::{self, build_client, cap_detail, dec, reduce_high_low};
use crate::error::ProviderError;
use crate::provider::{DatedClose, MarketDataProvider, RawFetch};

const DEFAULT_BASE_URL: &str = "https://api.twelvedata.com";

/// The Twelve Data HTTP adapter. `base_url` is injectable so tests / a mock can point elsewhere.
pub struct TwelveDataProvider {
    http: Client,
    base_url: String,
}

impl TwelveDataProvider {
    /// A provider against the live Twelve Data API.
    pub fn new() -> Self {
        TwelveDataProvider {
            http: build_client(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    /// A provider against an arbitrary base URL (a mock server in an integration test).
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        TwelveDataProvider {
            http: build_client(),
            base_url: base_url.into(),
        }
    }

    /// [`common::get_json`] (NFR-S1 key hygiene + HTTP-status classification, incl. the 403 body
    /// detail) followed by the Twelve-Data-specific step: many errors arrive as a **200 body**
    /// (`{"status":"error",…}`), so the body is classified before being treated as data.
    async fn get_json(&self, url: &str, ticker: &str) -> Result<Value, ProviderError> {
        let body = common::get_json(&self.http, url, ticker).await?;
        if let Some(err) = classify_twelvedata(&body, ticker) {
            return Err(err);
        }
        Ok(body)
    }
}

impl Default for TwelveDataProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MarketDataProvider for TwelveDataProvider {
    async fn fetch_fundamentals(
        &self,
        ticker: &str,
        api_key: Option<&str>,
    ) -> Result<RawFetch, ProviderError> {
        let key = api_key.ok_or(ProviderError::InvalidOrAbsentKey)?;
        // Issue #70: the wire symbol is the per-provider spelling of the stored canonical ticker
        // (`.US` → bare; pinned venues → `&mic_code=`; unknown venues verbatim). The original
        // `ticker` is kept for error identity.
        let symbol = symbol_query(ticker);
        // Daily series → per-year high/low + the latest close. `outputsize` covers several years of
        // bars; `order=desc` is pinned explicitly so `latest_close` (which reads `values[0]`) is the
        // most-recent bar regardless of the API's default ordering. Free tier serves SIX/global equities.
        let url = format!(
            "{}/time_series?{symbol}&interval=1day&outputsize=5000&order=desc&apikey={key}",
            self.base_url
        );
        let series = self.get_json(&url, ticker).await?;
        let financials = map_twelvedata(&series, ticker)?;
        let latest_price = latest_close(&series);
        // Issue #72: the newest bar's `datetime` is the real session date of `latest_price`.
        let latest_session_date = latest_close_session_date(&series);
        Ok(RawFetch {
            financials,
            latest_price,
            latest_session_date,
            // Issue #113: Twelve Data's fundamentals are None by design (Story 7.4) — no TTM EPS either.
            ttm_eps: None,
            // Twelve Data's fundamentals here are price-derived — no company sector (issue #98).
            sector: None,
        })
    }

    async fn fetch_latest_price(
        &self,
        ticker: &str,
        api_key: Option<&str>,
    ) -> Result<Option<DatedClose>, ProviderError> {
        // The price-only path (holdings refresh, Story-5.1 price history): the cheap `/price` endpoint.
        let key = api_key.ok_or(ProviderError::InvalidOrAbsentKey)?;
        // Issue #70: per-provider wire symbol (see `symbol_query`); original `ticker` kept for errors.
        let symbol = symbol_query(ticker);
        let url = format!("{}/price?{symbol}&apikey={key}", self.base_url);
        let body = self.get_json(&url, ticker).await?;
        // Issue #72: the bare `/price` body has NO date, so the close is undated (`session_date: None`)
        // and the caller keys the confront cache by the clock day. (The dated path is `/time_series`.)
        Ok(price_of(&body).map(|close| DatedClose {
            close,
            session_date: None,
        }))
    }

    async fn fetch_fx_rate(
        &self,
        base: &str,
        quote: &str,
        api_key: Option<&str>,
    ) -> Result<Option<DatedClose>, ProviderError> {
        // Story 6.5 (FR28): on Twelve Data an FX pair is served by the SAME `/price` endpoint as an
        // equity's latest price, under the native pair symbol `"{base}/{quote}"` (e.g. `EUR/CHF`).
        // So this is exactly a one-line symbol-format + delegate — the HTTP call, the 200-body error
        // classification ([`classify_twelvedata`]) and the exact-decimal parse ([`price_of`]) are
        // the already-tested `fetch_latest_price` path, never duplicated (NFR-S1 stays in one place).
        // Currency codes are plain `[A-Z]{3}`, so no URL-encoding concern beyond the fixed `/`.
        // Issue #90 (part 3): `session_date` rides along too (always `None` here — the bare
        // `/price` body is undated, same as the equity price path).
        self.fetch_latest_price(&fx_pair_symbol(base, quote), api_key)
            .await
    }
}

/// PURE: classify a Twelve Data 200 body that reports an error (`{"status":"error","code":N,
/// "message":…}`). `None` when the body is not an error. The message is capped + key-free.
pub fn classify_twelvedata(body: &Value, ticker: &str) -> Option<ProviderError> {
    if body.get("status").and_then(Value::as_str) != Some("error") {
        return None;
    }
    // `code` may be a number or a string ("401") across Twelve Data responses — accept both so an
    // auth/quota error is never mis-bucketed into the catch-all `Parse`.
    let code = body
        .get("code")
        .and_then(|c| {
            c.as_u64()
                .or_else(|| c.as_str().and_then(|s| s.trim().parse().ok()))
        })
        .unwrap_or(0);
    let detail = cap_detail(
        body.get("message")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    Some(match code {
        401 => ProviderError::InvalidOrAbsentKey,
        403 => ProviderError::Forbidden { detail },
        404 => ProviderError::TickerNotFound {
            ticker: ticker.to_string(),
        },
        // Issue #80: a 429 delivered as an HTTP status carries `Retry-After` (wired in `common`); a
        // 429 reported inside a 200 body here has no documented reset field → no hint (advance).
        429 => ProviderError::Quota {
            retry_after_secs: None,
        },
        _ => ProviderError::Parse {
            detail: format!("twelvedata error {code}: {detail}"),
        },
    })
}

/// PURE: the latest close from a `/time_series` body. The series is newest-first by default, so the
/// **first** value's `close` is the most recent. `None` when empty/missing.
pub fn latest_close(series: &Value) -> Option<Decimal> {
    let values = series.get("values")?.as_array()?;
    dec(values.first()?.get("close"))
}

/// PURE: the price from a `/price` body (`{"price":"104.23"}`). `None` when absent/empty.
pub fn price_of(body: &Value) -> Option<Decimal> {
    dec(body.get("price"))
}

/// PURE: the session date of the newest `/time_series` bar (issue #72) — `values[0].datetime`
/// (`"YYYY-MM-DD"` at the daily interval; newest-first, matching [`latest_close`]). `None` when
/// absent. Only the fundamentals (`/time_series`) fetch can supply this; the bare `/price` path has
/// no date.
pub fn latest_close_session_date(series: &Value) -> Option<String> {
    let values = series.get("values")?.as_array()?;
    values
        .first()?
        .get("datetime")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// PURE: Twelve Data's FX pair symbol (Story 6.5, FR28) — its native slash form, `"{base}/{quote}"`
/// (e.g. `EUR/CHF`), accepted by `/price` exactly like an equity ticker. Per-provider pair
/// spellings are the #70 symbol-convention class.
pub fn fx_pair_symbol(base: &str, quote: &str) -> String {
    format!("{base}/{quote}")
}

/// PURE: the pinned EODHD-venue-suffix → ISO 10383 **MIC** table (issue #70). Only venues whose
/// EODHD code and MIC are both unambiguous are listed — a MIC strictly filters the listing venue on
/// Twelve Data, so a correct entry can never fetch another exchange's price, and an *absent* entry
/// falls back to the verbatim-symbol rail (neutral no-data notice, never a venue guess). Extending
/// the table is a one-line, test-pinned change.
fn venue_mic(suffix: &str) -> Option<&'static str> {
    // EODHD `TICKER.<code>` → operating MIC (ISO 10383).
    Some(match suffix.to_ascii_uppercase().as_str() {
        "SW" => "XSWX",    // SIX Swiss Exchange
        "PA" => "XPAR",    // Euronext Paris
        "L" => "XLON",     // London Stock Exchange
        "XETRA" => "XETR", // Deutsche Börse Xetra
        "F" => "XFRA",     // Börse Frankfurt (floor)
        "AS" => "XAMS",    // Euronext Amsterdam
        "BR" => "XBRU",    // Euronext Brussels
        "LS" => "XLIS",    // Euronext Lisbon
        "IR" => "XDUB",    // Euronext Dublin
        "MC" => "XMAD",    // Bolsa de Madrid
        "MI" => "XMIL",    // Borsa Italiana (Milan)
        "VI" => "XWBO",    // Wiener Börse
        "ST" => "XSTO",    // Nasdaq Stockholm
        "OL" => "XOSL",    // Oslo Børs
        "CO" => "XCSE",    // Nasdaq Copenhagen
        "HE" => "XHEL",    // Nasdaq Helsinki
        "TO" => "XTSE",    // Toronto Stock Exchange
        "V" => "XTSX",     // TSX Venture
        "HK" => "XHKG",    // Hong Kong Stock Exchange
        "AU" => "XASX",    // Australian Securities Exchange
        _ => return None,
    })
}

/// PURE: the Twelve Data equity **query fragment** for a stored canonical ticker (issue #70). The
/// stored ticker follows EODHD's `TICKER.EXCHANGE` convention; Twelve Data expects the bare symbol
/// plus, for a non-US listing, an explicit venue:
/// - the UNAMBIGUOUS US virtual-exchange suffix `.US` is stripped (`AAPL.US` → `symbol=AAPL`; only
///   the trailing exchange, so `BRK.B.US` → `symbol=BRK.B`);
/// - a suffix in the pinned [`venue_mic`] table pins the venue by MIC
///   (`NESN.SW` → `symbol=NESN&mic_code=XSWX`) — deterministic, never a guess;
/// - any OTHER suffix is left **verbatim** (`symbol=NESN.XX`): an unmapped venue surfaces the
///   neutral no-data notice on Twelve Data (never a crash, never the wrong exchange's price — the
///   "absent, never wrong" rail). A symbol with no suffix passes through unchanged.
pub fn symbol_query(ticker: &str) -> String {
    match ticker.rsplit_once('.') {
        Some((base, exchange)) if !base.is_empty() && exchange.eq_ignore_ascii_case("US") => {
            format!("symbol={base}")
        }
        Some((base, exchange)) if !base.is_empty() => match venue_mic(exchange) {
            Some(mic) => format!("symbol={base}&mic_code={mic}"),
            None => format!("symbol={ticker}"),
        },
        _ => format!("symbol={ticker}"),
    }
}

/// PURE: a Twelve Data `/time_series` JSON → a **price-led** [`RawFinancials`]. No I/O. Currency from
/// `meta.currency`; per-year `high_price`/`low_price` reduced from the daily bars; financial fields
/// stay `None` (manual entry — the free tier serves no fundamentals); no splits. The caller passes
/// this to `core::normalize`.
pub fn map_twelvedata(series: &Value, ticker: &str) -> Result<RawFinancials, ProviderError> {
    // Require the currency (like EODHD's `General.CurrencyCode`): defaulting a missing currency to USD
    // would silently mislabel a CHF/EUR equity's prices (and suppress `core::normalize`'s
    // CurrencyMismatch finding). A live `/time_series` `meta` carries `currency`; its absence is an
    // error, not a guess.
    let currency = series
        .pointer("/meta/currency")
        .and_then(Value::as_str)
        .ok_or_else(|| ProviderError::Parse {
            detail: format!("missing meta.currency for {ticker}"),
        })?
        .to_string();

    // Per-year high/low reduced from the daily bars (`values[]`, `"datetime"`-keyed).
    let (highs, lows) = reduce_high_low(series.get("values"), "datetime");

    let mut years: BTreeMap<i32, ()> = BTreeMap::new();
    for y in highs.keys().chain(lows.keys()) {
        years.insert(*y, ());
    }

    let amount = |d: Option<Decimal>| {
        d.map(|value| RawAmount {
            value,
            currency: currency.clone(),
        })
    };

    let years: Vec<RawYear> = years
        .keys()
        .map(|&year| RawYear {
            year,
            period_months: None,
            fiscal_year_end_month: None,
            sales: None,
            eps: None,
            high_price: amount(highs.get(&year).copied()),
            low_price: amount(lows.get(&year).copied()),
            dividend_per_share: None,
            pre_tax_profit: None,
            net_profit: None,
            tax_rate: None,
            book_value_per_share: None,
        })
        .collect();

    Ok(RawFinancials {
        native_currency: currency,
        years,
        splits: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn map_twelvedata_reduces_per_year_high_low_with_currency() {
        let series = json!({
            "meta": { "symbol": "NESN", "currency": "CHF" },
            "values": [
                { "datetime": "2024-12-30", "high": "105.0", "low": "103.0", "close": "104.0" },
                { "datetime": "2024-06-01", "high": "110.0", "low": "99.0",  "close": "100.0" },
                { "datetime": "2023-12-30", "high": "95.0",  "low": "90.0",  "close": "92.0"  }
            ],
            "status": "ok"
        });
        let fin = map_twelvedata(&series, "NESN").unwrap();
        assert_eq!(fin.native_currency, "CHF");
        assert_eq!(fin.years.len(), 2);
        let y2024 = fin.years.iter().find(|y| y.year == 2024).unwrap();
        assert_eq!(y2024.high_price.as_ref().unwrap().value, dec_s("110.0"));
        assert_eq!(y2024.low_price.as_ref().unwrap().value, dec_s("99.0"));
        assert_eq!(y2024.high_price.as_ref().unwrap().currency, "CHF");
        // Financial fields are left for manual entry (free tier serves no fundamentals).
        assert!(y2024.sales.is_none() && y2024.eps.is_none());
    }

    #[test]
    fn latest_close_reads_the_newest_first_value() {
        let series = json!({ "values": [
            { "datetime": "2024-12-30", "close": "104.0" },
            { "datetime": "2024-12-29", "close": "103.0" }
        ]});
        assert_eq!(latest_close(&series), Some(dec_s("104.0")));
        assert_eq!(latest_close(&json!({ "values": [] })), None);
        assert_eq!(latest_close(&json!({})), None);
    }

    #[test]
    fn price_of_parses_the_price_endpoint() {
        assert_eq!(
            price_of(&json!({ "price": "104.23" })),
            Some(dec_s("104.23"))
        );
        assert_eq!(price_of(&json!({})), None);
    }

    #[test]
    fn latest_close_session_date_reads_the_newest_bar_datetime() {
        // Issue #72: the fundamentals `/time_series` path can supply the real session date (the
        // newest bar's `datetime`); the bare `/price` path cannot.
        let series = json!({ "values": [
            { "datetime": "2026-12-30", "close": "104.0" },
            { "datetime": "2026-12-29", "close": "103.0" }
        ]});
        assert_eq!(
            latest_close_session_date(&series),
            Some("2026-12-30".to_string())
        );
        assert_eq!(latest_close_session_date(&json!({ "values": [] })), None);
        assert_eq!(latest_close_session_date(&json!({})), None);
    }

    #[test]
    fn fx_pair_symbol_is_the_native_slash_form() {
        // Story 6.5: `fetch_fx_rate` = `fetch_latest_price` with this symbol — the only new logic.
        assert_eq!(fx_pair_symbol("EUR", "CHF"), "EUR/CHF");
        assert_eq!(fx_pair_symbol("USD", "JPY"), "USD/JPY");
    }

    #[test]
    fn symbol_query_strips_us_and_pins_known_venues_by_mic() {
        // Issue #70: EODHD-canonical `.US` → Twelve Data's bare symbol; a pinned venue suffix →
        // `&mic_code=` (deterministic table, never a guess); an UNKNOWN suffix stays verbatim
        // (neutral no-data notice → never the wrong exchange's price).
        assert_eq!(symbol_query("AAPL.US"), "symbol=AAPL");
        assert_eq!(symbol_query("aapl.us"), "symbol=aapl"); // the suffix match is case-insensitive
        assert_eq!(symbol_query("BRK.B.US"), "symbol=BRK.B"); // only the TRAILING exchange is removed
        assert_eq!(symbol_query("NESN.SW"), "symbol=NESN&mic_code=XSWX"); // SIX pinned by MIC
        assert_eq!(symbol_query("nesn.sw"), "symbol=nesn&mic_code=XSWX"); // venue match too
        assert_eq!(symbol_query("MC.PA"), "symbol=MC&mic_code=XPAR"); // Euronext Paris
        assert_eq!(symbol_query("SHEL.L"), "symbol=SHEL&mic_code=XLON"); // London
        assert_eq!(symbol_query("SAP.XETRA"), "symbol=SAP&mic_code=XETR"); // Xetra
        assert_eq!(symbol_query("RY.TO"), "symbol=RY&mic_code=XTSE"); // Toronto
        assert_eq!(symbol_query("NESN.XX"), "symbol=NESN.XX"); // unmapped venue: verbatim
        assert_eq!(symbol_query("BRK.B"), "symbol=BRK.B"); // `.B` is not a venue → verbatim
        assert_eq!(symbol_query("AAPL"), "symbol=AAPL"); // already bare — unchanged
        assert_eq!(symbol_query(".US"), "symbol=.US"); // degenerate: no base → verbatim, never ""
        assert_eq!(symbol_query("EUR/CHF"), "symbol=EUR/CHF"); // the FX pair path is untouched
    }

    #[test]
    fn venue_mic_table_is_case_insensitive_and_absent_for_unknown() {
        assert_eq!(venue_mic("SW"), Some("XSWX"));
        assert_eq!(venue_mic("sw"), Some("XSWX"));
        assert_eq!(venue_mic("XX"), None);
        assert_eq!(venue_mic("US"), None); // `.US` is handled upstream (bare symbol, no MIC)
    }

    #[test]
    fn an_fx_rate_body_parses_like_any_price_body() {
        // Story 6.5: the `/price` body for a pair symbol is the SAME shape as an equity's
        // (`{"price":"0.9312"}`) — the delegated `price_of` path parses it exactly (NFR-C1), and a
        // no-quote body yields None (never an inverted-pair guess).
        assert_eq!(
            price_of(&json!({ "price": "0.9312" })),
            Some(dec_s("0.9312"))
        );
        assert_eq!(price_of(&json!({})), None);
        // Pair-symbol errors (e.g. an unknown pair reported as a 200 body) classify identically.
        assert!(matches!(
            classify_twelvedata(
                &json!({ "status": "error", "code": 404, "message": "symbol not found" }),
                "EUR/CHF",
            ),
            Some(ProviderError::TickerNotFound { .. })
        ));
    }

    #[test]
    fn classify_twelvedata_maps_json_error_codes() {
        let err = |code: u64| {
            classify_twelvedata(
                &json!({ "status": "error", "code": code, "message": "x" }),
                "NESN",
            )
        };
        assert!(matches!(err(401), Some(ProviderError::InvalidOrAbsentKey)));
        assert!(matches!(err(403), Some(ProviderError::Forbidden { .. })));
        assert!(matches!(
            err(404),
            Some(ProviderError::TickerNotFound { .. })
        ));
        assert!(matches!(err(429), Some(ProviderError::Quota { .. })));
        assert!(matches!(err(400), Some(ProviderError::Parse { .. })));
        // A healthy body is not an error.
        assert!(classify_twelvedata(&json!({ "status": "ok", "values": [] }), "NESN").is_none());
        assert!(classify_twelvedata(&json!({ "price": "1.0" }), "NESN").is_none());
    }

    #[test]
    fn a_string_typed_error_code_is_still_classified() {
        // Review patch: Twelve Data may return `"code":"401"` (string) — must not fall to Parse.
        let err = classify_twelvedata(
            &json!({ "status": "error", "code": "401", "message": "x" }),
            "NESN",
        );
        assert!(matches!(err, Some(ProviderError::InvalidOrAbsentKey)));
    }

    #[test]
    fn a_missing_currency_is_an_error_not_a_usd_guess() {
        // Review patch: a missing meta.currency must error (like EODHD), not silently label USD.
        let series = json!({ "values": [
            { "datetime": "2024-12-30", "high": "1", "low": "1", "close": "1" }
        ]});
        assert!(matches!(
            map_twelvedata(&series, "NESN"),
            Err(ProviderError::Parse { .. })
        ));
    }

    #[test]
    fn an_error_message_never_leaks_and_is_capped() {
        let long = "z".repeat(500);
        let err = classify_twelvedata(
            &json!({ "status": "error", "code": 400, "message": long }),
            "NESN",
        );
        if let Some(ProviderError::Parse { detail }) = err {
            assert!(detail.len() < 260, "the message is capped");
        } else {
            panic!("expected Parse");
        }
    }

    fn dec_s(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }
}
