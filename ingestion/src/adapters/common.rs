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

use chrono::{Datelike, NaiveDate};

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
/// numerators and denominators are compounded separately and the price divided ONCE (a 3:2 split
/// of 150 is 100, no 0.666…7 factor compounded in).
///
/// ONE rounding rule (G1 H review): a rebased price is a DERIVED per-share figure, so whenever at
/// least one split applies it is rounded to 4 dp (`round_dp`, banker's midpoint) — exactly like
/// book value and dividend per share (#119). No split after the bar → the served value, untouched
/// (never re-rounded). `None` on an (astronomically unlikely) overflow — the caller then withholds
/// that whole year rather than mis-scale it.
pub(crate) fn rebase_price(value: Decimal, date: &str, splits: &[DatedSplit]) -> Option<Decimal> {
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
    Some(quotient.round_dp(4))
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
    reduce_high_low_by(bars, date_field, splits, year_of_date_key)
}

/// [`reduce_high_low_adjusted`] reduced into the company's FISCAL years instead of calendar years
/// (ssg-1.2.0, the NAIC rule: the high and low prices of a year are those of the company's fiscal
/// year, the same period its sales and EPS cover). `fiscal_ends` are the fiscal-year end dates the
/// statements report (any order, duplicates allowed); each bar goes to the fiscal year whose period
/// holds it — see [`FiscalCalendar`]. The split rebasing is per bar, exactly as before (#217, G1 H).
///
/// No readable fiscal end at all (a response without yearly statements) → the calendar years, the
/// pre-1.2.0 reduction: there is no fiscal calendar to follow, and a December year end is then the
/// only defensible reading.
pub(crate) fn reduce_high_low_fiscal(
    bars: Option<&Value>,
    date_field: &str,
    splits: &[DatedSplit],
    fiscal_ends: &[NaiveDate],
) -> (BTreeMap<i32, Decimal>, BTreeMap<i32, Decimal>) {
    let Some(calendar) = FiscalCalendar::new(fiscal_ends, bar_date_span(bars, date_field)) else {
        return reduce_high_low_adjusted(bars, date_field, splits);
    };
    reduce_high_low_by(bars, date_field, splits, |date| {
        calendar.fiscal_year_of(bar_day(date)?)
    })
}

/// The calendar day of a bar's date key (`"YYYY-MM-DD"`, optionally followed by a time).
fn bar_day(date: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date.get(0..10)?, "%Y-%m-%d").ok()
}

/// The earliest and latest readable bar days, `None` without any.
fn bar_date_span(bars: Option<&Value>, date_field: &str) -> Option<(NaiveDate, NaiveDate)> {
    let days = bars?.as_array()?.iter().filter_map(|bar| {
        bar.get(date_field)
            .and_then(Value::as_str)
            .and_then(bar_day)
    });
    days.fold(None, |span, day| match span {
        None => Some((day, day)),
        Some((lo, hi)) => Some((lo.min(day), hi.max(day))),
    })
}

/// A company's fiscal calendar, read off the fiscal-year end dates its statements report.
///
/// A fiscal year is labelled by the calendar year its END falls in (the statements' own key: NVDA's
/// year ended 2024-01-28 is « 2024 »), and its period runs from the day after the previous fiscal
/// year end through its own end: `(previous end, end]`. The boundaries:
/// - the reported ends, as served;
/// - a GAP of more than a year and a month between two reported ends (a missing statement) is
///   filled with ends one year apart from the earlier one, so a year's high/low never spans two
///   years;
/// - before the earliest reported end, ends one year apart going back (the earliest reported year
///   is thus `(end − 1 year, end]`, and the bars before it keep a one-year period each);
/// - after the latest reported end, ends one year apart going forward — the fiscal year in progress,
///   not yet reported (NVDA on 2026-09-26: FY2027, 2026-01-26 → 2027-01-25, labelled « 2027 »). Its
///   row carries prices and no sales, and the refresh drops it as it dropped the calendar year in
///   progress before (issue #109).
///
/// Two boundaries in the same calendar year (a fiscal-year-end change, a 52/53-week year ending in
/// the first days of January) would be two periods under one label: only the LATER one is that
/// year's — the same row the statements keep (#37, the latest date key wins) — and the bars of the
/// earlier period are left out, never merged into a period longer than the year.
///
/// For a December year end every boundary is a 31 December and every fiscal year is the calendar
/// year: the reduction is exactly the calendar one.
pub(crate) struct FiscalCalendar {
    /// Ascending, distinct boundaries, each with whether its period is its label year's.
    boundaries: Vec<(NaiveDate, bool)>,
}

impl FiscalCalendar {
    /// `None` when no fiscal end is known (the caller then keeps the calendar years). `span` is the
    /// first and last bar day: the projection reaches just past them both ways.
    pub(crate) fn new(
        fiscal_ends: &[NaiveDate],
        span: Option<(NaiveDate, NaiveDate)>,
    ) -> Option<Self> {
        let year = chrono::Months::new(12);
        let mut ends: Vec<NaiveDate> = fiscal_ends.to_vec();
        ends.sort();
        ends.dedup();
        let (&first, &last) = (ends.first()?, ends.last()?);
        let mut all: Vec<NaiveDate> = Vec::with_capacity(ends.len() + 8);
        // Gaps: from each reported end, one year at a time while the next reported end is more than
        // a year and a month away (a 52/53-week year moves its end by a few days, never a month).
        for pair in ends.windows(2) {
            let mut cursor = pair[0];
            all.push(cursor);
            while let Some(next) = cursor.checked_add_months(year) {
                let slack = next.checked_add_months(chrono::Months::new(1));
                if slack.is_none_or(|s| s >= pair[1]) {
                    break;
                }
                all.push(next);
                cursor = next;
            }
        }
        all.push(last);
        if let Some((first_bar, last_bar)) = span {
            // Back: boundaries while they are still on or after the first bar (each such bar then
            // finds a boundary at or after it).
            let mut cursor = first;
            while let Some(prev) = cursor.checked_sub_months(year) {
                if prev < first_bar {
                    break;
                }
                all.push(prev);
                cursor = prev;
            }
            // Forward: until a boundary reaches the last bar (the fiscal year in progress).
            let mut cursor = last;
            while cursor < last_bar {
                let Some(next) = cursor.checked_add_months(year) else {
                    break;
                };
                all.push(next);
                cursor = next;
            }
        }
        all.sort();
        all.dedup();
        let boundaries = all
            .iter()
            .enumerate()
            .map(|(i, end)| {
                let later_same_year = all.get(i + 1).is_some_and(|next| next.year() == end.year());
                (*end, !later_same_year)
            })
            .collect();
        Some(FiscalCalendar { boundaries })
    }

    /// The fiscal year (its label) whose period holds `day`: the year of the first boundary on or
    /// after it — `None` past the last boundary, or in an earlier period of a doubled label year.
    pub(crate) fn fiscal_year_of(&self, day: NaiveDate) -> Option<i32> {
        let i = self.boundaries.partition_point(|(end, _)| *end < day);
        let (end, is_the_years) = self.boundaries.get(i)?;
        is_the_years.then_some(end.year())
    }
}

/// The fiscal-year-end MONTH of a reported fiscal end date, read as the company names it: a
/// 52/53-week year ends on a weekday near the month's end and may spill into the first days of the
/// next month (a « Saturday nearest 31 December » year ended 2026-01-03) — a day in the first week
/// counts as the previous month's end, so such a company never reads as changing its year end
/// (the `fiscal_period_misalignment` check compares these months).
pub(crate) fn fiscal_year_end_month(end: NaiveDate) -> u32 {
    if end.day() <= 7 {
        if end.month() == 1 {
            12
        } else {
            end.month() - 1
        }
    } else {
        end.month()
    }
}

/// The shared per-bar reduction: each bar's date → its year label by `year_of`, rebased by the
/// splits after it, reduced to the label's max(high) / min(low).
fn reduce_high_low_by(
    bars: Option<&Value>,
    date_field: &str,
    splits: &[DatedSplit],
    year_of: impl Fn(&str) -> Option<i32>,
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
        let Some(year) = year_of(date) else {
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

    /// G1 H (#237): a fractional ratio (3:2, served "1.500000/1.000000") applies — the numerators
    /// and denominators compound separately and the price divides once, so 150 → 100; whenever a
    /// split applies the result is rounded to 4 dp (#119's per-share rule — one rule, G1 H
    /// review), including a terminating 5-dp quotient; a bar ON the split date is already
    /// post-split and served untouched (never re-rounded); a reverse split (1:10) multiplies.
    #[test]
    fn rebase_price_applies_fractional_and_reverse_ratios_with_one_rounding_rule() {
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
        let forty = [DatedSplit {
            date: "2022-06-01".into(),
            numerator: d("40"),
            denominator: d("1"),
        }];
        assert_eq!(
            rebase_price(d("589.07"), "2020-01-15", &forty),
            Some(d("14.7268")),
            "589.07 ÷ 40 = 14.72675 terminates, yet is rounded to 4 dp too (one rule)"
        );
        assert_eq!(
            rebase_price(d("150.123456"), "2022-06-01", &three_for_two),
            Some(d("150.123456")),
            "the split-date bar is already post-split — served untouched, not re-rounded"
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

    fn ymd(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    /// ssg-1.2.0: a December year end reduces EXACTLY as the calendar years did — every boundary is
    /// a 31 December, including the projected ones before the first and after the last statement.
    #[test]
    fn a_december_fiscal_calendar_reduces_exactly_like_the_calendar_years() {
        let bars = json!([
            { "date": "2021-02-01", "high": "8", "low": "7" },
            { "date": "2022-12-31", "high": "9", "low": "6" },
            { "date": "2023-01-02", "high": "12", "low": "10" },
            { "date": "2023-12-29", "high": "11", "low": "9" },
            { "date": "2024-06-01", "high": "30", "low": "26" },
            { "date": "2026-09-24", "high": "40", "low": "35" },
        ]);
        let splits = [DatedSplit {
            date: "2024-03-01".into(),
            numerator: Decimal::from(2),
            denominator: Decimal::ONE,
        }];
        let ends = [ymd("2023-12-31"), ymd("2024-12-31"), ymd("2023-12-31")];
        assert_eq!(
            reduce_high_low_fiscal(Some(&bars), "date", &splits, &ends),
            reduce_high_low_adjusted(Some(&bars), "date", &splits)
        );
        // No fiscal end known at all → the calendar years, the pre-1.2.0 reduction.
        assert_eq!(
            reduce_high_low_fiscal(Some(&bars), "date", &splits, &[]),
            reduce_high_low_adjusted(Some(&bars), "date", &splits)
        );
    }

    /// ssg-1.2.0: a January year end (NVDA) — each bar goes to the fiscal year whose period
    /// `(previous end, end]` holds it, labelled by the year of its end; the earliest year is
    /// `(end − 1 year, end]`; the bars after the last reported end go to the fiscal year in
    /// progress, one year on (FY2027 on 2026-09-26).
    #[test]
    fn a_january_fiscal_calendar_keys_bars_by_the_fiscal_year_that_holds_them() {
        let ends = [ymd("2023-01-29"), ymd("2024-01-28"), ymd("2025-01-26")];
        let cal = FiscalCalendar::new(&ends, Some((ymd("2021-06-01"), ymd("2026-09-24"))))
            .expect("known ends");
        let fy = |d: &str| cal.fiscal_year_of(ymd(d));
        assert_eq!(fy("2022-01-31"), Some(2023), "day after end − 1 year");
        assert_eq!(fy("2023-01-29"), Some(2023), "the end itself is inside");
        assert_eq!(fy("2023-01-30"), Some(2024));
        assert_eq!(
            fy("2024-01-25"),
            Some(2024),
            "calendar 2024, fiscal 2024 ended 2024-01-28"
        );
        assert_eq!(fy("2024-01-29"), Some(2025));
        assert_eq!(fy("2025-01-26"), Some(2025));
        assert_eq!(
            fy("2025-06-10"),
            Some(2026),
            "projected: (2025-01-26, 2026-01-26]"
        );
        assert_eq!(fy("2026-09-24"), Some(2027), "the fiscal year in progress");
        assert_eq!(
            fy("2021-06-01"),
            Some(2022),
            "projected back: (2021-01-29, 2022-01-29]"
        );
    }

    /// A June year end, and a missing statement: a gap of two years between reported ends is cut
    /// into one-year periods — a year's high/low never spans two years.
    #[test]
    fn a_june_fiscal_calendar_with_a_missing_year_keeps_one_year_periods() {
        let ends = [ymd("2019-06-30"), ymd("2021-06-30"), ymd("2022-06-30")];
        let bars = json!([
            { "date": "2019-06-28", "high": "10", "low": "9" },
            { "date": "2019-07-01", "high": "20", "low": "19" },
            { "date": "2020-06-30", "high": "21", "low": "18" },
            { "date": "2020-07-01", "high": "30", "low": "29" },
            { "date": "2021-06-30", "high": "31", "low": "28" },
            { "date": "2022-01-15", "high": "40", "low": "39" },
            { "date": "2022-07-01", "high": "50", "low": "45" },
        ]);
        let (h, l) = reduce_high_low_fiscal(Some(&bars), "date", &[], &ends);
        let d = |s: &str| Decimal::from_str_exact(s).unwrap();
        assert_eq!((h[&2019], l[&2019]), (d("10"), d("9")));
        assert_eq!((h[&2020], l[&2020]), (d("21"), d("18")), "the missing year");
        assert_eq!((h[&2021], l[&2021]), (d("31"), d("28")));
        assert_eq!(
            (h[&2022], l[&2022]),
            (d("40"), d("39")),
            "calendar 2022 H1 only"
        );
        assert_eq!((h[&2023], l[&2023]), (d("50"), d("45")), "in progress");
    }

    /// Two boundaries in one label year (a 52/53-week year ending 2022-01-01, then 2022-12-31):
    /// only the later period is « 2022 » — as the statements keep the later row (#37) — and the
    /// bars of the earlier one are left out, never merged into a 24-month « year ».
    #[test]
    fn two_fiscal_ends_in_one_label_year_keep_the_later_period_only() {
        let ends = [ymd("2021-01-02"), ymd("2022-01-01"), ymd("2022-12-31")];
        let bars = json!([
            { "date": "2020-12-31", "high": "5", "low": "4" },
            { "date": "2021-06-01", "high": "99", "low": "1" },
            { "date": "2022-06-01", "high": "7", "low": "6" },
        ]);
        let (h, l) = reduce_high_low_fiscal(Some(&bars), "date", &[], &ends);
        assert_eq!(h[&2021], Decimal::from(5));
        assert_eq!(
            h[&2022],
            Decimal::from(7),
            "not the 99 of the dropped period"
        );
        assert_eq!(l[&2022], Decimal::from(6));
        assert_eq!(h.len(), 2);
    }

    /// A 52/53-week year ending in the first week of the next month keeps the month it closes.
    #[test]
    fn fiscal_year_end_month_reads_a_first_week_end_as_the_previous_month() {
        assert_eq!(fiscal_year_end_month(ymd("2024-01-28")), 1);
        assert_eq!(fiscal_year_end_month(ymd("2026-01-03")), 12);
        assert_eq!(fiscal_year_end_month(ymd("2023-10-01")), 9);
        assert_eq!(fiscal_year_end_month(ymd("2024-06-30")), 6);
        assert_eq!(fiscal_year_end_month(ymd("2024-12-31")), 12);
    }
}
