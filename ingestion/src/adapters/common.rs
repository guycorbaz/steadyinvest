//! Shared adapter plumbing — EODHD (Story 3.1) + Twelve Data (Story 7.4) deduplicated, so the
//! NFR-S1 key-hygiene and exact-decimal (NFR-C1) invariants live in ONE place instead of two
//! drift-prone copies.
//!
//! Everything here is `pub(crate)`; the crate's public surface is unchanged. Each adapter keeps
//! only what is provider-specific: URL building, response-**body** classification (Twelve Data
//! reports many errors in a 200 body), and the mapping to `RawFinancials`. HTTP-**status**
//! classification is identical across providers (401/403/404/429/other) and is owned here.

use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::Client;
use rust_decimal::Decimal;
use serde_json::Value;

use crate::error::ProviderError;

/// The shared `reqwest` client with sane timeouts (#39). Without these, a hung connection never
/// resolves, so the off-thread fetch/key-test never returns and the UI latches "Récupération…" /
/// "Test… en cours" with no recovery. A connect + overall-request bound guarantees every job
/// terminates (as a `Network` error on timeout — cause-named by Story 3.5). The builder config is
/// static (timeouts only), so `.expect` can only trip on a TLS-backend init failure — the same
/// panic surface as `Client::new()` — and panicking at provider construction is the right call
/// there (no client, no adapter).
pub(crate) fn build_client() -> Client {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("the reqwest client builds with timeouts")
}

/// GET `url` and parse the JSON body, with every failure mapped to a cause-named
/// [`ProviderError`]. Providers that report errors inside a 200 body (Twelve Data) classify the
/// returned `Value` themselves — this function only owns transport + HTTP status.
///
/// NFR-S1: the request URL carries the API key (`?api_token=…` / `?apikey=…`). `reqwest::Error`'s
/// Display can include the URL, so `.without_url()` is MANDATORY before stringifying — otherwise
/// the key would leak into `ProviderError::{Network,Parse}` detail and on into a user-facing
/// notice.
pub(crate) async fn get_json(
    http: &Client,
    url: &str,
    ticker: &str,
) -> Result<Value, ProviderError> {
    let resp = http
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
        return Err(ProviderError::Forbidden {
            detail: cap_detail(&detail),
        });
    }
    // A 429 carries the server's rate-limit hint in the `Retry-After` header (issue #80): parse it
    // here (like 403) while the response is in hand, so the worker's `quota_wait` can actually wait
    // it out for one same-member retry instead of always advancing on an undeclared quota.
    if status.as_u16() == 429 {
        let retry_after_secs = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_retry_after_secs);
        return Err(ProviderError::Quota { retry_after_secs });
    }
    Err(classify_status(status.as_u16(), ticker))
}

/// PURE: an HTTP `Retry-After` header value → a whole number of seconds to wait (issue #80). Only the
/// **delta-seconds** form (`"120"`) is honored — the form rate-limit JSON APIs use; the alternative
/// HTTP-date form (`"Wed, 21 Oct 2015 07:28:00 GMT"`) yields `None` (this transport layer has no date
/// parser or injected clock, and guessing a wrong wait is worse than advancing). A non-numeric,
/// signed, or empty value is `None`, so a malformed header never becomes a nonsense wait.
fn parse_retry_after_secs(raw: &str) -> Option<u64> {
    raw.trim().parse::<u64>().ok()
}

/// HTTP status → cause-named [`ProviderError`]. (403 and 429 are handled in [`get_json`] with the
/// response in hand — 403 needs the body, 429 the `Retry-After` header — so they never reach here;
/// only 401 maps to an invalid/absent key. The 429 arm here is a defensive fallback with no hint.)
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

/// Trim + cap a provider-supplied detail string at 200 chars — enough to be actionable in a
/// notice, small enough to never dump a page of HTML/JSON into the UI. Key-free by construction:
/// only ever fed response *bodies*, never URLs.
pub(crate) fn cap_detail(s: &str) -> String {
    s.trim().chars().take(200).collect()
}

/// Parse a `Decimal` from a JSON value that is a number or a numeric string. Never uses `f64`
/// (NFR-C1 — exact decimal end to end).
pub(crate) fn dec(v: Option<&Value>) -> Option<Decimal> {
    match v? {
        Value::String(s) => Decimal::from_str_exact(s.trim()).ok(),
        Value::Number(n) => Decimal::from_str_exact(&n.to_string()).ok(),
        _ => None,
    }
}

/// Leading `YYYY` of a `"YYYY-MM-DD"` (or `"YYYY-MM-DD HH:MM:SS"`) date key → fiscal year.
pub(crate) fn year_of_date_key(key: &str) -> Option<i32> {
    key.get(0..4)?.parse::<i32>().ok()
}

/// Reduce an array of daily bars into per-year max(`high`) / min(`low`). The providers differ only
/// in where the array lives and what the date field is called (`"date"` for EODHD's `/eod`,
/// `"datetime"` for Twelve Data's `/time_series` values), so both are parameters; `None` / a
/// non-array yields empty maps (no bars → no high/low, never a zero).
pub(crate) fn reduce_high_low(
    bars: Option<&Value>,
    date_field: &str,
) -> (BTreeMap<i32, Decimal>, BTreeMap<i32, Decimal>) {
    reduce_high_low_adjusted(bars, date_field, &[])
}

/// A dated share split as the provider lists it: `numerator` new shares for `denominator` old
/// ones, effective on `date` (`"YYYY-MM-DD"`). Issue #217. Exact `Decimal`s, both strictly
/// positive (the adapter refuses anything else): a 3:2 split is served `"1.500000/1.000000"` and
/// must be applied as 1.5, never truncated to 1 nor dropped (G1 H, #237).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DatedSplit {
    pub date: String,
    pub numerator: Decimal,
    pub denominator: Decimal,
}

/// A raw price `value` dated `date`, brought into TODAY's shares: for every split dated strictly
/// after the bar, × `denominator / numerator` (a 10:1 split divides the pre-split prices by 10).
/// ISO dates compare lexicographically; a bar ON the split date is already post-split. The
/// numerators and denominators are compounded separately and the price divided ONCE, so a 3:2
/// split of 150 is exactly 100 (no 0.666…7 factor). A quotient that does not terminate within 8
/// decimals (an odd price over 1.5) is rounded to 4 dp, like the other derived per-share figures
/// (#119). No split after the bar → the value untouched. `None` on an (astronomically unlikely)
/// overflow — the caller then withholds that whole year rather than mis-scale it.
fn rebase_price(value: Decimal, date: &str, splits: &[DatedSplit]) -> Option<Decimal> {
    let mut numerators = Decimal::ONE;
    let mut denominators = Decimal::ONE;
    let mut any = false;
    for split in splits.iter().filter(|s| s.date.as_str() > date) {
        numerators = numerators.checked_mul(split.numerator)?;
        denominators = denominators.checked_mul(split.denominator)?;
        any = true;
    }
    if !any {
        return Some(value);
    }
    let quotient = value.checked_mul(denominators)?.checked_div(numerators)?;
    let exact = quotient.round_dp(8);
    Some(if exact == quotient {
        quotient
    } else {
        quotient.round_dp(4)
    })
}

/// [`reduce_high_low`] with each DAILY bar first rebased into today's shares by the splits dated
/// after it (issue #217): a provider that restates its per-share fundamentals but serves raw price
/// bars would otherwise pair a post-split EPS with a pre-split price (NVDA: high P/E ≈ 3 400). The
/// adjustment is per bar, not per year, so a mid-year split never mixes two share bases inside one
/// yearly high/low. No splits → the raw reduce. A bar whose rebase overflows withholds its whole
/// year (both high and low): a yearly extreme reduced from the OTHER bars would be a partial figure
/// passed off as the year's (« absent, never wrong »).
pub(crate) fn reduce_high_low_adjusted(
    bars: Option<&Value>,
    date_field: &str,
    splits: &[DatedSplit],
) -> (BTreeMap<i32, Decimal>, BTreeMap<i32, Decimal>) {
    let mut highs: BTreeMap<i32, Decimal> = BTreeMap::new();
    let mut lows: BTreeMap<i32, Decimal> = BTreeMap::new();
    let mut withheld: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    let Some(bars) = bars.and_then(Value::as_array) else {
        return (highs, lows);
    };
    for bar in bars {
        let Some(date) = bar.get(date_field).and_then(Value::as_str) else {
            continue;
        };
        let Some(year) = year_of_date_key(date) else {
            continue;
        };
        let mut scaled = |v: Option<Decimal>| {
            let raw = v?;
            let rebased = rebase_price(raw, date, splits);
            if rebased.is_none() {
                withheld.insert(year);
            }
            rebased
        };
        let high = scaled(dec(bar.get("high")));
        let low = scaled(dec(bar.get("low")));
        if let Some(high) = high {
            highs
                .entry(year)
                .and_modify(|m| {
                    if high > *m {
                        *m = high;
                    }
                })
                .or_insert(high);
        }
        if let Some(low) = low {
            lows.entry(year)
                .and_modify(|m| {
                    if low < *m {
                        *m = low;
                    }
                })
                .or_insert(low);
        }
    }
    for year in &withheld {
        highs.remove(year);
        lows.remove(year);
    }
    (highs, lows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_retry_after_reads_delta_seconds_and_rejects_the_rest() {
        // Issue #80: the delta-seconds form is honored; anything else → None (advance, never guess).
        assert_eq!(parse_retry_after_secs("120"), Some(120));
        assert_eq!(parse_retry_after_secs("  30 "), Some(30));
        assert_eq!(parse_retry_after_secs("0"), Some(0)); // quota_wait treats 0 as "no wait"
        // The HTTP-date form is not parsed (no clock in this layer) → None.
        assert_eq!(
            parse_retry_after_secs("Wed, 21 Oct 2015 07:28:00 GMT"),
            None
        );
        assert_eq!(parse_retry_after_secs("-5"), None, "signed → None");
        assert_eq!(parse_retry_after_secs("soon"), None);
        assert_eq!(parse_retry_after_secs(""), None);
    }

    #[test]
    fn cap_detail_trims_and_caps_at_200_chars() {
        assert_eq!(cap_detail("  plan limit  "), "plan limit");
        let long = "é".repeat(500); // chars, not bytes — a multi-byte char never gets split
        assert_eq!(cap_detail(&long).chars().count(), 200);
    }

    #[test]
    fn dec_parses_strings_and_numbers_exactly_never_f64() {
        assert_eq!(
            dec(Some(&json!("103.25"))),
            Some(Decimal::from_str_exact("103.25").unwrap())
        );
        assert_eq!(
            dec(Some(&json!(1.5))),
            Some(Decimal::from_str_exact("1.5").unwrap())
        );
        assert_eq!(dec(Some(&json!(null))), None);
        assert_eq!(dec(None), None);
    }

    #[test]
    fn year_of_date_key_reads_the_leading_year() {
        assert_eq!(year_of_date_key("2024-12-30"), Some(2024));
        assert_eq!(year_of_date_key("2024-12-30 15:30:00"), Some(2024));
        assert_eq!(year_of_date_key("bad"), None);
    }

    /// Issue #217: a 4:1 split on 2021-07-20 rebases the bars BEFORE it (÷4) and leaves the later
    /// ones; a mid-year split keeps one share base inside the yearly high/low; two splits compound.
    #[test]
    fn reduce_high_low_adjusted_rebases_each_bar_by_the_splits_after_it() {
        let d = |s: &str| Decimal::from_str_exact(s).unwrap();
        let bars = json!([
            { "date": "2020-06-01", "high": "800", "low": "400" },
            { "date": "2021-03-01", "high": "600", "low": "500" },
            { "date": "2021-09-01", "high": "220", "low": "180" },
            { "date": "2024-03-01", "high": "1000", "low": "900" },
            { "date": "2024-09-01", "high": "140", "low": "100" },
        ]);
        let splits = vec![
            DatedSplit {
                date: "2021-07-20".into(),
                numerator: d("4"),
                denominator: d("1"),
            },
            DatedSplit {
                date: "2024-06-10".into(),
                numerator: d("10"),
                denominator: d("1"),
            },
        ];
        let (h, l) = reduce_high_low_adjusted(Some(&bars), "date", &splits);
        assert_eq!(h[&2020], d("20"), "800 ÷ 4 ÷ 10");
        assert_eq!(l[&2020], d("10"));
        // 2021: the March bar (pre-split) is 600 ÷ 40 = 15 / 12.5; the September bar (post-4:1,
        // pre-10:1) is 220 ÷ 10 = 22 / 18 — the yearly high is the September one, not a raw 600.
        assert_eq!(h[&2021], d("22"));
        assert_eq!(l[&2021], d("12.5"));
        // 2024: March ÷ 10 = 100 / 90; September untouched 140 / 100.
        assert_eq!(h[&2024], d("140"));
        assert_eq!(l[&2024], d("90"));
        // No splits → the raw reduce, byte-for-byte the old behaviour.
        let (h0, _) = reduce_high_low_adjusted(Some(&bars), "date", &[]);
        assert_eq!(h0[&2020], d("800"));
    }

    /// G1 H (#237): a fractional ratio (3:2, served "1.500000/1.000000") applies exactly — the
    /// numerators and denominators compound separately and the price divides once, so 150 → 100
    /// with no 0.666…7 residue; a non-terminating quotient is rounded to 4 dp (#119's figure
    /// rule); a bar ON the split date is already post-split; a reverse split (1:10) multiplies.
    #[test]
    fn rebase_price_applies_fractional_and_reverse_ratios_exactly() {
        let d = |s: &str| Decimal::from_str_exact(s).unwrap();
        let three_for_two = [DatedSplit {
            date: "2022-06-01".into(),
            numerator: d("1.5"),
            denominator: d("1"),
        }];
        assert_eq!(
            rebase_price(d("150"), "2022-05-31", &three_for_two),
            Some(d("100"))
        );
        assert_eq!(
            rebase_price(d("100"), "2022-05-31", &three_for_two),
            Some(d("66.6667")),
            "100 ÷ 1.5 does not terminate → 4 dp"
        );
        assert_eq!(
            rebase_price(d("150"), "2022-06-01", &three_for_two),
            Some(d("150")),
            "the split-date bar is already post-split"
        );
        let reverse = [DatedSplit {
            date: "2022-06-01".into(),
            numerator: d("1"),
            denominator: d("10"),
        }];
        assert_eq!(
            rebase_price(d("1.25"), "2021-01-04", &reverse),
            Some(d("12.5"))
        );
    }

    /// An overflowing rebase withholds the WHOLE year (high and low): an extreme reduced from the
    /// remaining bars would be a partial figure passed off as the year's (« absent, never wrong »).
    #[test]
    fn an_overflowing_rebase_withholds_the_whole_year_not_just_the_bar() {
        let d = |s: &str| Decimal::from_str_exact(s).unwrap();
        let bars = json!([
            { "date": "2020-03-01", "high": "10", "low": "9" },
            { "date": "2020-09-01", "high": "79228162514264337593543950335", "low": "5" },
            { "date": "2021-03-01", "high": "7", "low": "6" },
        ]);
        let reverse = [DatedSplit {
            date: "2020-12-01".into(),
            numerator: d("1"),
            denominator: d("10"),
        }];
        let (h, l) = reduce_high_low_adjusted(Some(&bars), "date", &reverse);
        assert!(!h.contains_key(&2020) && !l.contains_key(&2020));
        assert_eq!(h[&2021], d("7"));
        assert_eq!(l[&2021], d("6"));
    }

    #[test]
    fn reduce_high_low_is_date_field_parameterized() {
        // The same bars keyed by either provider's date field reduce identically.
        let eodhd_bars = json!([
            { "date": "2024-06-01", "high": "110.0", "low": "99.0" },
            { "date": "2024-12-30", "high": "105.0", "low": "103.0" },
        ]);
        let twelve_bars = json!([
            { "datetime": "2024-06-01", "high": "110.0", "low": "99.0" },
            { "datetime": "2024-12-30", "high": "105.0", "low": "103.0" },
        ]);
        let (h1, l1) = reduce_high_low(Some(&eodhd_bars), "date");
        let (h2, l2) = reduce_high_low(Some(&twelve_bars), "datetime");
        assert_eq!(h1, h2);
        assert_eq!(l1, l2);
        assert_eq!(h1[&2024], Decimal::from_str_exact("110.0").unwrap());
        assert_eq!(l1[&2024], Decimal::from_str_exact("99.0").unwrap());
        // Missing / non-array input → empty maps, not a panic.
        assert!(reduce_high_low(None, "date").0.is_empty());
        assert!(reduce_high_low(Some(&json!({})), "date").0.is_empty());
    }
}
