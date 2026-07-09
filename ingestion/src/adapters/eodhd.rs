//! EODHD adapter (https://eodhd.com) — CH/EU+US coverage. Story 3.1, first adapter.
//!
//! Two endpoints feed one [`RawFinancials`]:
//! - `/fundamentals/{ticker}` → currency, per-year income-statement / balance-sheet / earnings,
//!   declared splits;
//! - `/eod/{ticker}` (daily OHLC) → each fiscal year's high/low, reduced from the daily bars.
//!
//! The **pure mapping** [`map_eodhd`] (no I/O) is the CI-tested heart; the HTTP layer is thin
//! (shared with Twelve Data via [`crate::adapters::common`]) and validated by a manual GO/NO-GO
//! with a real key (no network in CI). The assumed JSON shape follows EODHD's documented
//! structure — the manual run confirms fidelity to a live response.

use std::collections::BTreeMap;

use reqwest::Client;
use rust_decimal::Decimal;
use serde_json::Value;
use steadyinvest_core::normalize::{RawAmount, RawFinancials, RawYear, SplitEvent};

use crate::adapters::common::{build_client, dec, get_json, reduce_high_low, year_of_date_key};
use crate::error::ProviderError;
use crate::provider::{DatedClose, MarketDataProvider, RawFetch};

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
        let fundamentals = get_json(&self.http, &fundamentals_url, ticker).await?;
        let prices = get_json(&self.http, &eod_url, ticker).await?;
        let financials = map_eodhd(&fundamentals, &prices, ticker)?;
        // Story 4.4: the latest `/eod` close (the series is `order=a`, so the last bar is the most
        // recent) is the present market price for the §4 zone marker — `None` if the series is empty.
        // Issue #72: the bar carries its session `date`, threaded on for the confront cache key.
        let dated = latest_eod_close(&prices);
        let latest_price = dated.as_ref().map(|d| d.close);
        let latest_session_date = dated.and_then(|d| d.session_date);
        // Issue #113: the trailing-twelve-months EPS (the current-P/E denominator) — EODHD's own TTM
        // figure `Highlights.EarningsShare` (verified = the sum of the last 4 reported quarters, and it
        // skips the not-yet-reported current quarter). A present market fact, not an annual figure.
        let ttm_eps = dec(fundamentals.pointer("/Highlights/EarningsShare"));
        // Issue #98 (FR48): the company's sector — `General::Sector`, already in the fundamentals
        // response (no extra call). Trimmed; an empty/absent field is an honest `None`.
        let sector = fundamentals
            .pointer("/General/Sector")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        Ok(RawFetch {
            financials,
            latest_price,
            latest_session_date,
            ttm_eps,
            sector,
        })
    }

    async fn fetch_latest_price(
        &self,
        ticker: &str,
        api_key: Option<&str>,
    ) -> Result<Option<DatedClose>, ProviderError> {
        // Issue #50: hit ONLY `/eod` (no `/fundamentals`) — works on the free EODHD tier, which
        // allows EOD but 403s fundamentals. The series is `order=a`, so the last bar is the latest.
        let token = api_key.ok_or(ProviderError::InvalidOrAbsentKey)?;
        let eod_url = format!(
            "{}/eod/{ticker}?api_token={token}&period=d&fmt=json&order=a",
            self.base_url
        );
        let prices = get_json(&self.http, &eod_url, ticker).await?;
        Ok(latest_eod_close(&prices))
    }

    async fn fetch_fx_rate(
        &self,
        base: &str,
        quote: &str,
        api_key: Option<&str>,
    ) -> Result<Option<DatedClose>, ProviderError> {
        // Story 6.5 (FR28): EODHD serves FX pairs on the SAME `/eod` endpoint as equities, under the
        // symbol `"{base}{quote}.FOREX"` (e.g. `EURCHF.FOREX`); the latest EOD close IS the rate.
        // So this is exactly a one-line symbol-format + delegate — the HTTP call, status
        // classification and exact-decimal parse ([`latest_eod_close`]) are the already-tested
        // `fetch_latest_price` path, never duplicated (NFR-S1 stays in one place). It also inherits
        // the #50 property: `/eod`-only, so it works on the free tier that 403s `/fundamentals`.
        // Issue #90 (part 3): the bar's `date` rides along too, same as the price path.
        self.fetch_latest_price(&fx_pair_symbol(base, quote), api_key)
            .await
    }
}

/// The most recent close from the daily EOD array (Story 4.4) with its trading-session `date` (issue
/// #72). The series is requested `order=a` (ascending), so the **last** bar is today's; we read its
/// `close` (raw — comparable to the §4 forecast band, which is in present price terms) and its `date`
/// (the real EOD session date, used to key the confront cache). `None` when the array is empty or the
/// last bar has no `close`.
pub fn latest_eod_close(prices: &Value) -> Option<DatedClose> {
    let bars = prices.as_array()?;
    let last = bars.last()?;
    let close = dec(last.get("close"))?;
    let session_date = last.get("date").and_then(Value::as_str).map(str::to_string);
    Some(DatedClose {
        close,
        session_date,
    })
}

/// PURE: EODHD's FX pair symbol (Story 6.5, FR28) — the concatenated pair on the virtual FOREX
/// exchange, `"{base}{quote}.FOREX"` (e.g. `EURCHF.FOREX`), served by the same `/eod` endpoint as
/// an equity ticker. Per-provider pair spellings are the #70 symbol-convention class.
pub fn fx_pair_symbol(base: &str, quote: &str) -> String {
    format!("{base}{quote}.FOREX")
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
    let cash_flow = obj(fundamentals.pointer("/Financials/Cash_Flow/yearly"));
    let earnings = obj(fundamentals.pointer("/Earnings/Annual"));

    // Per-year high/low reduced from the daily EOD bars (root array, `"date"`-keyed).
    let (highs, lows) = reduce_high_low(Some(prices), "date");

    // Union of every fiscal year mentioned by any section, ascending.
    let mut years_set: BTreeMap<i32, ()> = BTreeMap::new();
    for key in income
        .keys()
        .chain(balance.keys())
        .chain(cash_flow.keys())
        .chain(earnings.keys())
    {
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
            let cash = year_row(cash_flow, y);
            let earn = year_row(earnings, y);
            RawYear {
                year: y,
                period_months: None,
                fiscal_year_end_month: None,
                sales: amount(field_dec(inc, "totalRevenue")),
                eps: amount(field_dec(earn, "epsActual")),
                high_price: amount(highs.get(&y).copied()),
                low_price: amount(lows.get(&y).copied()),
                // Issue #112: per-share dividend for the fiscal year — the cash-flow `dividendsPaid`
                // (total, same yearly statement → aligned to the fiscal year, no ex-date guessing)
                // over shares outstanding. `None` when either is absent (a non-payer or a gap).
                dividend_per_share: amount(dividend_per_share(cash, bal)),
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
        if let (Some(numerator), Some(denominator)) = (num, den)
            && numerator > 0
            && denominator > 0
        {
            out.push(SplitEvent {
                effective_year: year,
                numerator,
                denominator,
            });
        }
    }
    out.sort_by_key(|s| s.effective_year);
    out
}

/// EODHD writes split ratios as decimals ("4.000000"); take the integer part.
fn parse_split_part(s: &str) -> Option<u32> {
    // #37: a real split ratio is a positive WHOLE number (e.g. "4.000000/1.000000"). Be strict so a
    // malformed part is REJECTED (the split is dropped, never silently mis-applied) rather than
    // floored: reject a leading sign and any non-zero fractional remainder — the old `split('.')`
    // truncation parsed "4.9" as 4 and `u32::parse` accepted a leading "+".
    let s = s.trim();
    if s.starts_with('+') || s.starts_with('-') {
        return None;
    }
    let mut parts = s.split('.');
    let integer = parts.next()?;
    if let Some(fraction) = parts.next() {
        // A fractional remainder is allowed ONLY if it is all zeros ("4.000000"); else reject.
        if fraction.bytes().any(|b| b != b'0') {
            return None;
        }
    }
    if parts.next().is_some() {
        return None; // more than one '.' → malformed
    }
    integer.parse::<u32>().ok()
}

/// `totalStockholderEquity / commonStockSharesOutstanding` when both are present and shares ≠ 0.
fn book_value_per_share(year: Option<&Value>) -> Option<Decimal> {
    let equity = field_dec(year, "totalStockholderEquity")?;
    let shares = field_dec(year, "commonStockSharesOutstanding")?;
    if shares.is_zero() {
        return None;
    }
    // Issue #119: `equity / shares` carries ~28 spurious decimals from the division; a book-value-
    // PER-SHARE figure is a dollars-per-share amount — round to 4 dp (plenty for the §2 ROE) so the
    // grid cell reads "12.4972", not "12.49724842767295...". The other provider figures are integers,
    // so this is the only derived ratio that needs it.
    equity.checked_div(shares).map(|d| d.round_dp(4))
}

/// PURE: the per-share dividend for a fiscal year (Issue #112) = `|dividendsPaid| / shares`, from the
/// cash-flow + balance-sheet rows of the SAME yearly statement (so it is aligned to the fiscal year,
/// with no ex-dividend-date → year guessing). `dividendsPaid` may be reported as a cash OUTFLOW
/// (negative) — the magnitude is the dividend; rounded to 4 dp like [`book_value_per_share`]. `None`
/// when either input is absent (a non-payer, or a year the cash-flow statement does not cover).
fn dividend_per_share(cash: Option<&Value>, balance: Option<&Value>) -> Option<Decimal> {
    let paid = field_dec(cash, "dividendsPaid")?;
    let shares = field_dec(balance, "commonStockSharesOutstanding")?;
    if shares.is_zero() {
        return None;
    }
    paid.abs().checked_div(shares).map(|d| d.round_dp(4))
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
    // #37: when two keys fall in the same fiscal year (a fiscal-year-end change or a restated
    // period), pick the row with the **latest** date key — the most recent / restated figures win.
    // `YYYY-MM-DD` keys sort lexicographically = chronologically, so the `max` key is the latest date
    // (previously `find` took the first match = the lexicographically-smallest = OLDEST date).
    map.iter()
        .filter(|(k, _)| year_of_date_key(k) == Some(y))
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, v)| v)
}

/// A named field of an optional row, parsed as a `Decimal` (number or numeric string).
fn field_dec(row: Option<&Value>, field: &str) -> Option<Decimal> {
    dec(row?.get(field))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn latest_eod_close_reads_the_last_bar_in_ascending_order() {
        // `/eod?order=a` → ascending; the LAST bar is the most recent close (the present price for
        // the §4 zone marker, Story 4.4). Parsed exactly — never via `f64`. Issue #72: the bar's
        // real session `date` rides alongside the close.
        let prices = json!([
            { "date": "2026-06-25", "close": "101.5" },
            { "date": "2026-06-26", "close": "103.25" },
        ]);
        assert_eq!(
            latest_eod_close(&prices),
            Some(DatedClose {
                close: Decimal::from_str_exact("103.25").unwrap(),
                session_date: Some("2026-06-26".to_string()),
            })
        );
    }

    #[test]
    fn latest_eod_close_is_none_when_the_series_is_empty_or_malformed() {
        assert_eq!(latest_eod_close(&json!([])), None);
        assert_eq!(latest_eod_close(&json!({})), None);
        // Last bar present but no `close` field → no price, not a zero.
        assert_eq!(latest_eod_close(&json!([{ "date": "2026-06-26" }])), None);
        // A close with no `date` still yields the price, with `session_date: None` (caller falls back).
        assert_eq!(
            latest_eod_close(&json!([{ "close": "103.25" }])),
            Some(DatedClose {
                close: Decimal::from_str_exact("103.25").unwrap(),
                session_date: None,
            })
        );
    }

    /// Issue #112: `map_eodhd` derives the per-share dividend for each fiscal year from the cash-flow
    /// `dividendsPaid` over the balance-sheet shares (aligned, no ex-date guessing), rounded to 4 dp;
    /// a cash OUTFLOW (negative) is taken by magnitude; a year with no cash-flow row (a non-payer) is
    /// `None`. Also covers the #119 book-value rounding on the same fixture.
    #[test]
    fn map_eodhd_derives_per_share_dividend_from_the_cash_flow_statement() {
        let fundamentals = json!({
            "General": { "CurrencyCode": "USD" },
            "Earnings": { "Annual": {
                "2023-09-30": { "epsActual": "5.9" },
                "2024-09-30": { "epsActual": "6.1" },
            } },
            "Financials": {
                "Income_Statement": { "yearly": {
                    "2023-09-30": { "totalRevenue": "383000000000", "incomeBeforeTax": "114000000000" },
                    "2024-09-30": { "totalRevenue": "391000000000", "incomeBeforeTax": "120000000000" },
                } },
                "Balance_Sheet": { "yearly": {
                    "2023-09-30": { "totalStockholderEquity": "62000000000", "commonStockSharesOutstanding": "15500000000" },
                    "2024-09-30": { "totalStockholderEquity": "57000000000", "commonStockSharesOutstanding": "15000000000" },
                } },
                "Cash_Flow": { "yearly": {
                    // 2024 pays (positive), 2023 reports the outflow as NEGATIVE, 2022 is absent (non-payer year).
                    "2024-09-30": { "dividendsPaid": "15000000000" },
                    "2023-09-30": { "dividendsPaid": "-15500000000" },
                } },
            }
        });
        let fin = map_eodhd(&fundamentals, &json!([]), "AAPL.US").expect("maps");
        let y = |year: i32| {
            fin.years
                .iter()
                .find(|y| y.year == year)
                .expect("year present")
        };
        let div = |year: i32| y(year).dividend_per_share.as_ref().map(|a| a.value);
        // 2024: 15e9 / 15e9 = 1.0000.
        assert_eq!(div(2024), Some(Decimal::from_str_exact("1").unwrap()));
        // 2023: |-15.5e9| / 15.5e9 = 1.0000 (a negative outflow is taken by magnitude).
        assert_eq!(div(2023), Some(Decimal::from_str_exact("1").unwrap()));
        // #119: book value 2024 = 57e9 / 15e9 = 3.8, rounded 4 dp.
        assert_eq!(
            y(2024).book_value_per_share.as_ref().map(|a| a.value),
            Some(Decimal::from_str_exact("3.8").unwrap())
        );
    }

    #[test]
    fn fx_pair_symbol_is_the_concatenated_forex_form() {
        // Story 6.5: `fetch_fx_rate` = `fetch_latest_price` with this symbol — the only new logic.
        assert_eq!(fx_pair_symbol("EUR", "CHF"), "EURCHF.FOREX");
        assert_eq!(fx_pair_symbol("USD", "JPY"), "USDJPY.FOREX");
    }

    #[test]
    fn an_fx_eod_body_parses_like_any_eod_body() {
        // Story 6.5: `/eod` bars for a `.FOREX` symbol are the SAME shape as an equity's — the
        // delegated `latest_eod_close` path reads the last ascending bar's close exactly (NFR-C1),
        // and an empty series (no quote for the pair) yields None, never an inverted-pair guess.
        let bars = json!([
            { "date": "2026-07-01", "close": "0.9328" },
            { "date": "2026-07-02", "close": "0.9312" },
        ]);
        assert_eq!(
            latest_eod_close(&bars).map(|d| d.close),
            Some(Decimal::from_str_exact("0.9312").unwrap())
        );
        assert_eq!(latest_eod_close(&json!([])), None);
    }

    #[test]
    fn parse_split_part_accepts_whole_numbers_and_rejects_malformed_ratios() {
        // Real EODHD ratio shapes parse; malformed parts are REJECTED (the split is dropped), never
        // floored or sign-accepted (#37).
        assert_eq!(parse_split_part("4"), Some(4));
        assert_eq!(parse_split_part("4.000000"), Some(4));
        assert_eq!(parse_split_part(" 7 "), Some(7));
        assert_eq!(parse_split_part("4.9"), None, "no longer floored to 4");
        assert_eq!(parse_split_part("+4"), None, "leading sign rejected");
        assert_eq!(parse_split_part("-4"), None);
        assert_eq!(parse_split_part("1.2.3"), None);
        assert_eq!(parse_split_part("abc"), None);
        assert_eq!(parse_split_part(""), None);
    }

    #[test]
    fn year_row_picks_the_latest_date_in_a_duplicated_fiscal_year() {
        // Two keys in 2023 (a restated period / fiscal-year-end change): the LATEST date wins (#37).
        let m = json!({
            "2023-03-31": { "v": "old" },
            "2023-12-31": { "v": "restated" },
            "2022-12-31": { "v": "prior" },
        });
        let obj = m.as_object().unwrap();
        assert_eq!(
            year_row(obj, 2023)
                .and_then(|r| r.get("v"))
                .and_then(Value::as_str),
            Some("restated"),
            "the most recent same-year row wins"
        );
        assert_eq!(
            year_row(obj, 2022)
                .and_then(|r| r.get("v"))
                .and_then(Value::as_str),
            Some("prior")
        );
        assert!(year_row(obj, 2021).is_none());
    }
}
