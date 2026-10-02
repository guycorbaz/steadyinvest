//! The current price's **age horizon** (FR23, PRD Appendix A — owner decision 2026-10-01): a
//! price older than the configured number of **trading days** (default 1) counts as stale, exactly
//! like a price a failed refresh flagged — the verdict becomes provisional (« Prix actuel —
//! périmé ») and the traceability / PDF name it « périmé ».
//!
//! The age is a **read-time** fact, never a persisted flag: it depends on today and on a setting
//! the owner can change, so [`apply_price_age`] marks the read copy through
//! [`PriceOrigin::aged`], a field the contract never serializes. Raising the horizon un-ages a
//! price on the next read; nothing to repair in the dossier.
//!
//! The rule (pure, dates injected):
//! - the price's **date** is the provider's trading-session date when it supplied one, else the
//!   date it was written (`at` — a fetch without a session date, or the owner's typing). A typed
//!   price ages too: FR23 is about the price, whatever its origin;
//! - its **age** is the number of trading days (Monday–Friday, no holiday calendar) after that
//!   date up to and including today — a Friday close is 1 trading day old on the Monday, 2 on the
//!   Tuesday;
//! - it is **aged** when that age exceeds the horizon. An unknown origin (a price written before
//!   origins were recorded) or an unreadable date is never called aged — absent, never wrong.

use steadyinvest_contract::{PriceOrigin, Source, Study, Timestamp};

/// The default horizon in trading days (PRD Appendix A: « older than one trading day »).
pub const DEFAULT_PRICE_STALE_AFTER_TRADING_DAYS: u32 = 1;

/// The largest horizon the setting accepts (about one year of sessions).
pub const MAX_PRICE_STALE_AFTER_TRADING_DAYS: u32 = 260;

/// A calendar date (proleptic Gregorian), as days since 1970-01-01.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Day(i64);

impl Day {
    /// `YYYY-MM-DD` (or the date part of an RFC 3339 stamp) → the day; `None` when malformed.
    pub fn parse(s: &str) -> Option<Day> {
        let d = s.get(..10)?;
        let b = d.as_bytes();
        if b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') {
            return None;
        }
        let y: i64 = d.get(..4)?.parse().ok()?;
        let m: i64 = d.get(5..7)?.parse().ok()?;
        let day: i64 = d.get(8..10)?.parse().ok()?;
        if !(1..=12).contains(&m) || day < 1 || day > days_in_month(y, m) {
            return None;
        }
        Some(Day(days_from_civil(y, m, day)))
    }

    /// The UTC day of an app-clock stamp.
    pub fn of(t: &Timestamp) -> Option<Day> {
        Day::parse(&t.0)
    }

    /// Saturday or Sunday.
    fn is_weekend(self) -> bool {
        // 1970-01-01 was a Thursday: index 0 = Thursday … 2 = Saturday, 3 = Sunday.
        matches!(self.0.rem_euclid(7), 2 | 3)
    }
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 (H. Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The trading days (Monday–Friday) strictly after `from` up to and including `to`, counted up
/// to `cap + 1` at most (enough to compare against a horizon of `cap`). `0` when `to <= from`.
pub fn trading_days_after(from: Day, to: Day, cap: u32) -> u32 {
    let mut count = 0u32;
    let mut d = from.0 + 1;
    while d <= to.0 && count <= cap {
        if !Day(d).is_weekend() {
            count += 1;
        }
        d += 1;
    }
    count
}

/// The date the price stands for: the provider's session date when known, else when it was
/// written. `None` when unreadable (or for a `Derived` origin, which a price never has).
pub fn price_day(origin: &PriceOrigin) -> Option<Day> {
    match (origin.source, &origin.session_date) {
        (Source::Provider, Some(session)) => Day::parse(session),
        (Source::Provider | Source::Manual, _) => Day::of(&origin.at),
        (Source::Derived, _) => None,
    }
}

/// Is a price of this origin older than `horizon` trading days on `today`? PURE.
pub fn price_is_aged(origin: &PriceOrigin, today: Day, horizon: u32) -> bool {
    price_day(origin).is_some_and(|day| trading_days_after(day, today, horizon) > horizon)
}

/// Mark the read copy of `study` with the age rule: [`PriceOrigin::aged`] set when the current
/// price is older than the horizon, cleared otherwise. Never persisted (the contract skips the
/// field) — call it on what is READ for display, on every read (the setting and today move).
pub fn apply_price_age(study: &mut Study, today: Option<Day>, horizon: u32) {
    let present = study.judgment.current_price.is_some();
    if let Some(origin) = study.judgment.current_price_origin.as_mut() {
        origin.aged = present && today.is_some_and(|t| price_is_aged(origin, t, horizon));
    }
}

/// The horizon setting as stored in app-config: a whole number of trading days in
/// `1..=`[`MAX_PRICE_STALE_AFTER_TRADING_DAYS`]; anything else is `None` (the caller's default).
pub fn parse_horizon(s: &str) -> Option<u32> {
    s.trim()
        .parse::<u32>()
        .ok()
        .filter(|n| (1..=MAX_PRICE_STALE_AFTER_TRADING_DAYS).contains(n))
}

/// The effective horizon of a stored setting: [`parse_horizon`], else the default.
pub fn horizon_or_default(stored: Option<&str>) -> u32 {
    stored
        .and_then(parse_horizon)
        .unwrap_or(DEFAULT_PRICE_STALE_AFTER_TRADING_DAYS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::Freshness;

    fn day(s: &str) -> Day {
        Day::parse(s).expect("a valid date")
    }

    fn origin(source: Source, at: &str, session: Option<&str>) -> PriceOrigin {
        PriceOrigin {
            source,
            at: Timestamp(at.to_string()),
            session_date: session.map(str::to_string),
            freshness: Freshness::Current,
            aged: false,
        }
    }

    #[test]
    fn dates_parse_strictly() {
        assert_eq!(day("1970-01-01"), Day(0));
        assert_eq!(day("2026-10-01T08:00:00Z"), day("2026-10-01"));
        assert_eq!(day("2024-02-29").0 + 1, day("2024-03-01").0, "leap day");
        for bad in [
            "",
            "2026-13-01",
            "2026-02-30",
            "2025-02-29",
            "2026/10/01",
            "abcd-ef-gh",
        ] {
            assert_eq!(Day::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn weekends_are_saturday_and_sunday() {
        // 2026-10-03 is a Saturday, 2026-10-04 a Sunday, 2026-10-05 a Monday.
        assert!(day("2026-10-03").is_weekend());
        assert!(day("2026-10-04").is_weekend());
        assert!(!day("2026-10-05").is_weekend());
        assert!(!day("2026-10-02").is_weekend(), "Friday");
        assert!(!day("1970-01-01").is_weekend(), "Thursday");
        assert!(day("1969-12-28").is_weekend(), "a Sunday before the epoch");
    }

    #[test]
    fn trading_days_skip_weekends() {
        let fri = day("2026-10-02");
        assert_eq!(trading_days_after(fri, fri, 5), 0, "same day");
        assert_eq!(trading_days_after(fri, day("2026-10-03"), 5), 0, "Saturday");
        assert_eq!(trading_days_after(fri, day("2026-10-04"), 5), 0, "Sunday");
        assert_eq!(trading_days_after(fri, day("2026-10-05"), 5), 1, "Monday");
        assert_eq!(trading_days_after(fri, day("2026-10-06"), 5), 2, "Tuesday");
        assert_eq!(
            trading_days_after(fri, day("2026-10-09"), 9),
            5,
            "a week later"
        );
        assert_eq!(
            trading_days_after(day("2026-10-05"), fri, 5),
            0,
            "a future date"
        );
        // Counting stops past the cap (a years-old price costs a few iterations, not thousands).
        assert_eq!(trading_days_after(day("2000-01-03"), fri, 1), 2);
    }

    #[test]
    fn a_friday_close_ages_on_tuesday_with_the_default_horizon() {
        let o = origin(Source::Provider, "2026-10-03T09:00:00Z", Some("2026-10-02"));
        let h = DEFAULT_PRICE_STALE_AFTER_TRADING_DAYS;
        assert!(!price_is_aged(&o, day("2026-10-02"), h), "the session day");
        assert!(!price_is_aged(&o, day("2026-10-04"), h), "the weekend");
        assert!(
            !price_is_aged(&o, day("2026-10-05"), h),
            "Monday: one trading day"
        );
        assert!(price_is_aged(&o, day("2026-10-06"), h), "Tuesday: two");
        assert!(!price_is_aged(&o, day("2026-10-06"), 2), "a wider horizon");
        assert!(price_is_aged(&o, day("2026-10-07"), 2));
    }

    #[test]
    fn a_thursday_close_ages_on_monday() {
        let o = origin(Source::Provider, "2026-10-02T07:00:00Z", Some("2026-10-01"));
        assert!(
            !price_is_aged(&o, day("2026-10-04"), 1),
            "Sunday: Friday only"
        );
        assert!(
            price_is_aged(&o, day("2026-10-05"), 1),
            "Monday: Friday and Monday"
        );
    }

    #[test]
    fn the_session_date_wins_over_the_fetch_date() {
        // Fetched on Tuesday, but the close is Thursday's: two trading days old on Tuesday.
        let o = origin(Source::Provider, "2026-10-06T08:00:00Z", Some("2026-10-01"));
        assert!(price_is_aged(&o, day("2026-10-06"), 1));
        // Without a session date, the fetch date stands.
        let o = origin(Source::Provider, "2026-10-06T08:00:00Z", None);
        assert!(!price_is_aged(&o, day("2026-10-07"), 1));
        assert!(price_is_aged(&o, day("2026-10-08"), 1));
    }

    #[test]
    fn a_typed_price_ages_from_its_entry_date() {
        // Typed on a Saturday: Monday is its first trading day, Tuesday its second.
        let o = origin(Source::Manual, "2026-10-03T10:00:00Z", None);
        assert!(!price_is_aged(&o, day("2026-10-05"), 1));
        assert!(price_is_aged(&o, day("2026-10-06"), 1));
    }

    #[test]
    fn an_unreadable_date_is_never_called_aged() {
        let o = origin(Source::Provider, "garbage", Some("not-a-date"));
        assert!(!price_is_aged(&o, day("2030-01-01"), 1));
        let o = origin(Source::Derived, "2000-01-03T00:00:00Z", None);
        assert!(!price_is_aged(&o, day("2030-01-01"), 1));
    }

    #[test]
    fn apply_marks_the_read_copy_and_clears_it_again() {
        use steadyinvest_core::verdict::Verdict;
        let mut study = crate::form::tests::full_study();
        assert!(matches!(
            crate::form::build_snapshot(&study).unwrap().verdict(),
            Verdict::Full(_)
        ));
        study.judgment.current_price_origin =
            Some(origin(Source::Manual, "2026-10-01T10:00:00Z", None));
        let aged = |s: &Study| {
            s.judgment
                .current_price_origin
                .as_ref()
                .is_some_and(|o| o.aged)
        };
        apply_price_age(&mut study, Some(day("2026-10-06")), 1);
        assert!(aged(&study));
        assert!(
            study
                .judgment
                .current_price_origin
                .as_ref()
                .unwrap()
                .is_stale()
        );
        // The verdict degrades like for a failed refresh.
        assert!(matches!(
            crate::form::build_snapshot(&study).unwrap().verdict(),
            Verdict::Provisional(_)
        ));
        // The mark never reaches the serialized study.
        let json = steadyinvest_contract::to_export_json(&study);
        assert!(!json.contains("aged"), "{json}");
        // The owner widens the horizon: the next read un-ages it.
        apply_price_age(&mut study, Some(day("2026-10-06")), 5);
        assert!(!aged(&study));
        // No today (an unreadable clock) → never aged.
        apply_price_age(&mut study, Some(day("2026-10-06")), 1);
        apply_price_age(&mut study, None, 1);
        assert!(!aged(&study));
        // No price → nothing to age, whatever the origin says.
        study.judgment.current_price = None;
        apply_price_age(&mut study, Some(day("2026-10-06")), 1);
        assert!(!aged(&study));
    }

    #[test]
    fn the_setting_parses_whole_days_in_range() {
        assert_eq!(parse_horizon("1"), Some(1));
        assert_eq!(parse_horizon(" 5 "), Some(5));
        assert_eq!(parse_horizon("260"), Some(260));
        for bad in ["0", "261", "-1", "1.5", "", "un"] {
            assert_eq!(parse_horizon(bad), None, "{bad}");
        }
        assert_eq!(horizon_or_default(None), 1);
        assert_eq!(horizon_or_default(Some("garbage")), 1);
        assert_eq!(horizon_or_default(Some("3")), 3);
    }
}
