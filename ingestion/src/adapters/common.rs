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
    Err(classify_status(status.as_u16(), ticker))
}

/// HTTP status → cause-named [`ProviderError`]. (403 is handled in [`get_json`] with the response
/// body, so it never reaches here — only 401 maps to an invalid/absent key.)
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
    let mut highs: BTreeMap<i32, Decimal> = BTreeMap::new();
    let mut lows: BTreeMap<i32, Decimal> = BTreeMap::new();
    let Some(bars) = bars.and_then(Value::as_array) else {
        return (highs, lows);
    };
    for bar in bars {
        let Some(year) = bar
            .get(date_field)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
