//! EODHD adapter (https://eodhd.com) — CH/EU+US coverage. Story 3.1, first adapter.
//!
//! Three endpoints feed one [`RawFinancials`]:
//! - `/fundamentals/{ticker}` → currency, per-year income-statement / balance-sheet / earnings;
//! - `/eod/{ticker}` (daily OHLC) → each fiscal year's high/low, reduced from the daily bars;
//! - `/splits/{ticker}` → the split history that rebases those raw bars (issue #217).
//!
//! The **pure mapping** [`map_eodhd`] (no I/O) is the CI-tested heart; the HTTP layer is thin
//! (shared with Twelve Data via [`crate::adapters::common`]) and validated by a manual GO/NO-GO
//! with a real key (no network in CI). The assumed JSON shape follows EODHD's documented
//! structure — the manual run confirms fidelity to a live response.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use reqwest::Client;
use rust_decimal::Decimal;
use serde_json::Value;
use steadyinvest_core::normalize::{RawAmount, RawFinancials, RawYear};

use crate::adapters::common::{
    DatedSplit, build_client, cap_detail, dec, get_json, rebase_price, reduce_high_low_adjusted,
    year_of_date_key,
};
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
        // Issue #217: the split history lives on its own endpoint (the fundamentals block carries
        // only the LAST split). It rebases the raw price bars into today's shares — the per-share
        // fundamentals EODHD serves are already restated.
        let splits_url = format!(
            "{}/splits/{ticker}?api_token={token}&fmt=json&from=1900-01-01",
            self.base_url
        );
        let fundamentals = get_json(&self.http, &fundamentals_url, ticker).await?;
        let prices = get_json(&self.http, &eod_url, ticker).await?;
        // G1 H (#237, owner decision 10): a failed split request stays a HARD failure — the prices
        // cannot be put at today's share scale without it — but it is NAMED as the split history's
        // (a 403 plan without `/splits`, a 429, a network cut), never passed off as a fundamentals
        // or prices failure. The three requests share this job's single pacing slot (the worker's
        // `pace` + one quota retry, both keyed off `ProviderError::root_cause`).
        let splits = get_json(&self.http, &splits_url, ticker)
            .await
            .map_err(split_history_failure)?;
        // G1 H review: the fetch day bounds the split history (a split dated after it is refused).
        // Read here, in the I/O shell; the mapping below stays pure and takes it as a parameter.
        // G1 final review L11: in the LOCAL calendar — see [`fetch_day_at`].
        let fetch_day = fetch_day_at(chrono::Local::now().fixed_offset());
        let financials = map_eodhd(&fundamentals, &prices, &splits, fetch_day, ticker)?;
        // Story 4.4: the latest `/eod` close (the series is `order=a`, so the last bar is the most
        // recent) is the present market price for the §4 zone marker — `None` if the series is empty.
        // Issue #72: the bar carries its session `date`, threaded on for the confront cache key.
        // G1 final review M2: rebased into today's shares like every bar the yearly high/low come
        // from — a split effective after the last bar (today's split, a lagging series) otherwise
        // paired a pre-split price with the restated per-share figures. `map_eodhd` above already
        // accepted this split history, so reading it again cannot fail here.
        let split_history = map_split_history(&splits, fetch_day).map_err(split_history_failure)?;
        let dated = latest_eod_close_rebased(&prices, &split_history);
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

/// PURE: [`latest_eod_close`] brought into TODAY's shares by the splits dated after its bar (G1
/// final review M2) — the same per-bar rule as the yearly high/low ([`rebase_price`]: 4 dp whenever
/// a split applies, the served close untouched otherwise). Absent (`None`), never wrong: a last bar
/// without a date while a split history exists cannot be placed against it, and a rebase that
/// overflows is withheld. The session date rides on unchanged.
fn latest_eod_close_rebased(prices: &Value, splits: &[DatedSplit]) -> Option<DatedClose> {
    let dated = latest_eod_close(prices)?;
    if splits.is_empty() {
        return Some(dated);
    }
    let close = rebase_price(dated.close, dated.session_date.as_deref()?, splits)?;
    Some(DatedClose { close, ..dated })
}

/// The fetch day that bounds the split history (G1 final review L11), in the LOCAL calendar of
/// `now`. EODHD dates both its bars and its splits by the exchange's trading calendar. The fetch
/// day was the UTC date, which runs up to two hours BEHIND the user's calendar in Switzerland:
/// between 00:00 and 02:00 local, a split dated today was refused as « dated after the fetch day ».
/// The user's local date runs at or ahead of every European and American exchange's calendar for
/// a user in Europe, so a split effective today there is never refused; one dated tomorrow still
/// is. (An exchange east of the user — Asia — may start its day before the user's: a split dated
/// its today is then refused, NAMED, until the user's own midnight — never applied early.)
fn fetch_day_at(now: chrono::DateTime<chrono::FixedOffset>) -> NaiveDate {
    now.date_naive()
}

/// PURE: EODHD's FX pair symbol (Story 6.5, FR28) — the concatenated pair on the virtual FOREX
/// exchange, `"{base}{quote}.FOREX"` (e.g. `EURCHF.FOREX`), served by the same `/eod` endpoint as
/// an equity ticker. Per-provider pair spellings are the #70 symbol-convention class.
pub fn fx_pair_symbol(base: &str, quote: &str) -> String {
    format!("{base}{quote}.FOREX")
}

/// PURE: EODHD `/fundamentals` + `/eod` + `/splits` JSON → [`RawFinancials`]. No I/O. Missing
/// fields stay `None` (never coerced to 0). The caller passes this straight to `core::normalize`.
///
/// Issue #217 — share splits: EODHD RESTATES its per-share fundamentals (`epsActual`, the balance-
/// sheet shares behind the derived dividend and book value per share) into today's shares, but
/// serves the daily price bars RAW. So the split history adjusts the price bars here, and the
/// returned `splits` stay EMPTY: `normalize` must not rebase the already-restated per-share figures a
/// second time. `fetch_day` (the user's local calendar day, [`fetch_day_at`]) bounds the split
/// history: a split dated after it is refused.
pub fn map_eodhd(
    fundamentals: &Value,
    prices: &Value,
    splits: &Value,
    fetch_day: NaiveDate,
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

    // Per-year high/low reduced from the daily EOD bars (root array, `"date"`-keyed), each bar
    // first rebased into today's shares by the splits dated after it (issue #217). An unreadable
    // split history fails the whole mapping, named (G1 H) — never « no splits ».
    let split_history = map_split_history(splits, fetch_day).map_err(split_history_failure)?;
    let (highs, lows) = reduce_high_low_adjusted(Some(prices), "date", &split_history);

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

    Ok(RawFinancials {
        native_currency: currency,
        years,
        // Issue #217: the prices are already in today's shares and the per-share fundamentals come
        // restated — nothing is left for `normalize` to rebase.
        splits: Vec::new(),
    })
}

/// Any failure of the split history — the request's or the body's — under its own name (G1 H).
fn split_history_failure(cause: ProviderError) -> ProviderError {
    ProviderError::SplitHistory {
        cause: Box::new(cause),
    }
}

/// The `/splits/{ticker}` body is `[{ "date": "YYYY-MM-DD", "split": "num/den" }]` (e.g.
/// `"4.000000/1.000000"`, or `"1.500000/1.000000"` for a 3:2), ascending or not — sorted by date
/// here. An empty array is a company that never split.
///
/// G1 H (#237, owner decision 10): anything else is a `Parse` FAILURE, never « no splits » and
/// never a skipped row — a body that is not an array (an error object in a 200, a changed format),
/// a row without a `YYYY-MM-DD` date, a malformed or non-positive ratio. Skipping one row « with a
/// stated reason » was weighed and refused: every price before that split would still be served
/// at the wrong scale (the #217 defect itself), and the study has no channel to carry the reason
/// beside the figures it corrupts. The detail names the row, from the body only (key-free).
///
/// G1 H review — two more refusals, same strict policy:
/// - **Two rows on one date** (a duplicated 2:1 would compound into ÷4; two different ratios on
///   one day are ambiguous) — refused, never compounded nor de-duplicated by guess.
/// - **A split dated after `fetch_day`** (an announced, not-yet-effective split) — refused: applied,
///   it would rebase EVERY served bar, today's included, by a split that has not happened. The
///   bound is the FETCH DAY, not the last bar: a split effective today (or after a lagging last
///   bar) is real and must stay applied — the last-bar bound would refuse exactly that case.
fn map_split_history(body: &Value, fetch_day: NaiveDate) -> Result<Vec<DatedSplit>, ProviderError> {
    let Some(rows) = body.as_array() else {
        return Err(ProviderError::Parse {
            detail: cap_detail(&format!("split history is not a list: {body}")),
        });
    };
    let today = iso_date(fetch_day);
    let mut out: Vec<DatedSplit> = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let bad = |what: &str| ProviderError::Parse {
            detail: cap_detail(&format!("split history row {index}: {what}: {row}")),
        };
        let date = row
            .get("date")
            .and_then(Value::as_str)
            .filter(|d| is_iso_date(d))
            .ok_or_else(|| bad("no YYYY-MM-DD date"))?;
        let (numerator, denominator) = row
            .get("split")
            .and_then(Value::as_str)
            .and_then(parse_split_ratio)
            .ok_or_else(|| bad("malformed ratio"))?;
        if date > today.as_str() {
            return Err(bad(&format!("dated after the fetch day {today}")));
        }
        if out.iter().any(|s| s.date == date) {
            return Err(bad("a second split on the same date"));
        }
        out.push(DatedSplit {
            date: date.to_string(),
            numerator,
            denominator,
        });
    }
    out.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(out)
}

/// `YYYY-MM-DD` of a calendar date — the form the split and bar dates share.
fn iso_date(day: NaiveDate) -> String {
    format!("{:04}-{:02}-{:02}", day.year(), day.month(), day.day())
}

/// Exactly `YYYY-MM-DD` (ASCII digits and dashes) naming a real calendar day — the form the bar
/// dates share, so the split dates compare with them lexicographically = chronologically.
fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    let shaped = b.len() == 10
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            _ => c.is_ascii_digit(),
        });
    shaped
        && match (s[0..4].parse(), s[5..7].parse(), s[8..10].parse()) {
            (Ok(y), Ok(m), Ok(d)) => NaiveDate::from_ymd_opt(y, m, d).is_some(),
            _ => false,
        }
}

/// `"num/den"` → two exact, strictly positive `Decimal`s (G1 H: "1.500000/1.000000" is a 3:2
/// split, applied as 1.5 — the old whole-number parse dropped it in silence). Strict otherwise:
/// exactly one `/`, no sign (#37), no exponent, no zero — `None` is a malformed ratio.
fn parse_split_ratio(ratio: &str) -> Option<(Decimal, Decimal)> {
    let (num, den) = ratio.split_once('/')?;
    Some((parse_split_part(num)?, parse_split_part(den)?))
}

/// One side of a split ratio: plain digits with at most one decimal point, strictly positive.
fn parse_split_part(s: &str) -> Option<Decimal> {
    let s = s.trim();
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None; // a sign, an exponent, a second '/', letters → malformed (#37)
    }
    Decimal::from_str_exact(s)
        .ok()
        .filter(|d| d.is_sign_positive() && !d.is_zero())
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

    /// A fixed fetch day — never the wall clock in a test.
    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 25).unwrap()
    }

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
        let fin = map_eodhd(&fundamentals, &json!([]), &json!([]), day(), "AAPL.US").expect("maps");
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

    /// Issue #217: the `/splits` history rebases the RAW price bars into today's shares (the
    /// pre-split years' highs/lows divide by the compounded ratio); the returned `splits` stay
    /// empty so `normalize` never re-rebases the provider's already-restated per-share figures.
    #[test]
    fn map_eodhd_rebases_price_bars_by_the_split_history_and_passes_no_splits_on() {
        let fundamentals = json!({
            "General": { "CurrencyCode": "USD" },
            "Earnings": { "Annual": {
                "2020-01-26": { "epsActual": "0.1453" },
                "2025-01-26": { "epsActual": "2.992" }
            }}
        });
        let prices = json!([
            { "date": "2020-01-15", "high": "589.07", "low": "180.68", "close": "500" },
            { "date": "2025-01-15", "high": "212.19", "low": "86.62", "close": "200" }
        ]);
        let splits = json!([
            { "date": "2024-06-10", "split": "10.000000/1.000000" },
            { "date": "2021-07-20", "split": "4.000000/1.000000" }
        ]);
        let fin = map_eodhd(&fundamentals, &prices, &splits, day(), "NVDA.US").expect("maps");
        let y2020 = fin.years.iter().find(|y| y.year == 2020).expect("2020");
        let d = |s: &str| Decimal::from_str_exact(s).unwrap();
        assert_eq!(
            y2020.high_price.as_ref().map(|a| a.value),
            Some(d("14.7268")),
            "589.07 ÷ 40 = 14.72675, rounded to 4 dp (the one rule)"
        );
        assert_eq!(
            y2020.low_price.as_ref().map(|a| a.value),
            Some(d("4.517")),
            "180.68 ÷ 40"
        );
        // The EPS is taken as served (already restated) — never divided again.
        assert_eq!(y2020.eps.as_ref().map(|a| a.value), Some(d("0.1453")));
        let y2025 = fin.years.iter().find(|y| y.year == 2025).expect("2025");
        assert_eq!(
            y2025.high_price.as_ref().map(|a| a.value),
            Some(d("212.19")),
            "post-split: untouched"
        );
        assert!(
            fin.splits.is_empty(),
            "nothing left for normalize to rebase"
        );
        // Both rows kept, sorted by date.
        let history = map_split_history(&splits, day()).expect("a well-formed history");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].date, "2021-07-20");
    }

    /// G1 H (#237): an empty array is a company that never split — the bars pass through raw.
    #[test]
    fn an_empty_split_history_is_no_splits() {
        assert_eq!(
            map_split_history(&json!([]), day()).expect("empty is valid"),
            vec![]
        );
    }

    /// G1 H review: two rows on one date are refused (named), never compounded — a duplicated
    /// 2:1 would otherwise divide the pre-split prices by 4; two ratios on one day are ambiguous.
    #[test]
    fn a_duplicated_split_date_is_refused_never_compounded() {
        for second in ["2.000000/1.000000", "3.000000/1.000000"] {
            let body = json!([
                { "date": "2024-06-01", "split": "2.000000/1.000000" },
                { "date": "2024-06-01", "split": second },
            ]);
            let err = map_split_history(&body, day()).unwrap_err();
            let ProviderError::Parse { detail } = &err else {
                panic!("expected Parse, got {err:?}");
            };
            assert!(
                detail.contains("row 1") && detail.contains("same date"),
                "{detail}"
            );
        }
    }

    /// G1 H review: a split dated after the FETCH day (announced, not yet effective) is refused,
    /// named — applied, it would rebase today's bars too. A split ON the fetch day, or after a
    /// lagging last bar but not after the fetch day, is real and stays applied.
    #[test]
    fn a_split_dated_after_the_fetch_day_is_refused_but_a_past_one_after_the_last_bar_applies() {
        let future = json!([{ "date": "2026-09-26", "split": "2.000000/1.000000" }]);
        let err = map_split_history(&future, day()).unwrap_err();
        let ProviderError::Parse { detail } = &err else {
            panic!("expected Parse, got {err:?}");
        };
        assert!(
            detail.contains("after the fetch day 2026-09-25"),
            "{detail}"
        );
        assert!(
            map_split_history(&json!([{ "date": "2026-09-25", "split": "2/1" }]), day()).is_ok(),
            "a split effective on the fetch day is real"
        );
        // The last bar predates a PAST split (a lagging series): the split still applies.
        let fundamentals = json!({ "General": { "CurrencyCode": "USD" } });
        let prices = json!([{ "date": "2026-09-22", "high": "40", "low": "30" }]);
        let past = json!([{ "date": "2026-09-24", "split": "2.000000/1.000000" }]);
        let fin = map_eodhd(&fundamentals, &prices, &past, day(), "X.US").expect("maps");
        let y = fin.years.iter().find(|y| y.year == 2026).expect("2026");
        assert_eq!(
            y.high_price.as_ref().map(|a| a.value),
            Some(Decimal::from(20))
        );
    }

    /// G1 final review M2: the present price is rebased like the bars — a split effective after
    /// the last bar (a lagging series, today's split) divides it too; a close on or after every
    /// split is served untouched; with no split history, byte-for-byte the raw close.
    #[test]
    fn the_latest_close_is_rebased_by_a_split_after_the_last_bar() {
        let prices = json!([
            { "date": "2026-09-23", "close": "400" },
            { "date": "2026-09-24", "close": "401.5" },
        ]);
        let split = |date: &str| DatedSplit {
            date: date.into(),
            numerator: Decimal::from(4),
            denominator: Decimal::ONE,
        };
        let after = latest_eod_close_rebased(&prices, &[split("2026-09-25")]).expect("a close");
        assert_eq!(after.close, Decimal::from_str_exact("100.375").unwrap());
        assert_eq!(after.session_date.as_deref(), Some("2026-09-24"));
        let before = latest_eod_close_rebased(&prices, &[split("2026-09-24")]).expect("a close");
        assert_eq!(before.close, Decimal::from_str_exact("401.5").unwrap());
        assert_eq!(
            latest_eod_close_rebased(&prices, &[]),
            latest_eod_close(&prices)
        );
        // Consistent with the bars: the same split divides the yearly high by the same rule.
        let fundamentals = json!({ "General": { "CurrencyCode": "USD" } });
        let bars =
            json!([{ "date": "2026-09-24", "high": "401.5", "low": "400", "close": "401.5" }]);
        let splits = json!([{ "date": "2026-09-25", "split": "4.000000/1.000000" }]);
        let fin = map_eodhd(&fundamentals, &bars, &splits, day(), "X.US").expect("maps");
        let high = fin.years[0].high_price.as_ref().map(|a| a.value);
        let history = map_split_history(&splits, day()).unwrap();
        assert_eq!(
            high,
            latest_eod_close_rebased(&bars, &history).map(|d| d.close)
        );
        // A dateless last bar cannot be placed against a split history: absent, never wrong.
        let dateless = json!([{ "close": "400" }]);
        assert_eq!(
            latest_eod_close_rebased(&dateless, &[split("2026-09-25")]),
            None
        );
    }

    /// G1 final review L11: the fetch day is the LOCAL calendar day — at 00:30 in Zurich (22:30
    /// UTC the day before), a split dated today is accepted; one dated tomorrow is still refused.
    #[test]
    fn a_split_dated_today_is_accepted_just_after_local_midnight() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-09-26T00:30:00+02:00").unwrap();
        let fetch_day = fetch_day_at(now);
        assert_eq!(fetch_day, NaiveDate::from_ymd_opt(2026, 9, 26).unwrap());
        assert_ne!(
            fetch_day,
            now.naive_utc().date(),
            "the UTC date is still the 25th"
        );
        let today = json!([{ "date": "2026-09-26", "split": "2/1" }]);
        assert!(map_split_history(&today, fetch_day).is_ok());
        let tomorrow = json!([{ "date": "2026-09-27", "split": "2/1" }]);
        assert!(map_split_history(&tomorrow, fetch_day).is_err());
    }

    /// G1 H (#237, owner decision 10): a 200 whose body is NOT the expected array — an error
    /// object, a bare string, `null` — is a named split-history FAILURE of the whole mapping,
    /// never « no splits » (which would serve every pre-split price at the wrong scale).
    #[test]
    fn a_non_array_split_body_fails_the_mapping_named_never_no_splits() {
        let fundamentals = json!({ "General": { "CurrencyCode": "USD" } });
        let prices = json!([{ "date": "2020-01-15", "high": "589.07", "low": "180.68" }]);
        for body in [
            json!({ "error": "Only EOD data allowed for this plan" }),
            json!("Unauthenticated"),
            json!(null),
        ] {
            let err = map_eodhd(&fundamentals, &prices, &body, day(), "NVDA.US").unwrap_err();
            let ProviderError::SplitHistory { cause } = &err else {
                panic!("expected a named split-history failure, got {err:?}");
            };
            assert!(
                matches!(**cause, ProviderError::Parse { .. }),
                "{body}: {cause:?}"
            );
        }
    }

    /// G1 H (#237): a malformed row fails the history (named), never silently dropped — a dropped
    /// real split would leave its pre-split prices at the wrong scale. The detail names the row.
    #[test]
    fn a_malformed_split_row_fails_the_history_named() {
        for row in [
            json!({ "date": "bad", "split": "2/1" }),
            json!({ "date": "2021-13-45", "split": "2/1" }),
            json!({ "date": "2021-07-20" }),
            json!({ "date": "2021-07-20", "split": "4.000000" }),
            json!({ "date": "2021-07-20", "split": "0/1" }),
            json!({ "date": "2021-07-20", "split": "-4/1" }),
            json!({ "date": "2021-07-20", "split": "four/one" }),
            json!({ "split": "2/1" }),
        ] {
            let body = json!([{ "date": "2024-06-10", "split": "10.000000/1.000000" }, row]);
            let err = map_split_history(&body, day()).unwrap_err();
            let ProviderError::Parse { detail } = &err else {
                panic!("expected Parse, got {err:?}");
            };
            assert!(detail.contains("row 1"), "{detail}");
        }
    }

    #[test]
    fn parse_split_ratio_reads_exact_decimals_and_rejects_malformed_ratios() {
        let d = |s: &str| Decimal::from_str_exact(s).unwrap();
        // Real EODHD ratio shapes — including the fractional 3:2 the u32 parse used to drop (G1 H).
        assert_eq!(
            parse_split_ratio("4.000000/1.000000"),
            Some((d("4"), d("1")))
        );
        assert_eq!(
            parse_split_ratio("1.500000/1.000000"),
            Some((d("1.5"), d("1")))
        );
        assert_eq!(parse_split_ratio(" 7 / 1 "), Some((d("7"), d("1"))));
        assert_eq!(
            parse_split_ratio("1.000000/10.000000"),
            Some((d("1"), d("10"))),
            "a reverse split"
        );
        // Malformed → None (#37: no sign; plus no zero, no exponent, exactly one '/').
        for bad in [
            "4.9", "+4/1", "-4/1", "4/0", "0/1", "1.2.3/1", "abc/1", "4/1/1", "4e1/1", "", "/",
        ] {
            assert_eq!(parse_split_ratio(bad), None, "{bad:?}");
        }
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
