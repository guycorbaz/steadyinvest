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
//! The rule (pure, dates and time zone injected — owner decisions Guy 2026-10-03, review of
//! PR #293):
//! - the price's **date** is the provider's trading-session date when it supplied one (already a
//!   date, taken as given), else the **local** day it was written (`at` — a fetch without a session
//!   date, or the owner's typing). A typed price ages too: FR23 is about the price, whatever its
//!   origin;
//! - « today » and the day of a stamp are read in the owner's **local time zone** ([`DayZone`]:
//!   `Local` in the app and the MCP server, a fixed offset in tests — never the machine's zone);
//! - its **age** is the number of trading days (Monday–Friday, no holiday calendar) after that
//!   date up to and including today — a Friday close is 1 trading day old on the Monday, 2 on the
//!   Tuesday;
//! - it is **aged** when that age exceeds the horizon;
//! - a present price with **no recorded origin** (written before origins were recorded) — or with
//!   an origin whose **date cannot be read** ([`price_date_known`]), treated the same way — has an
//!   **unknown date** ([`PriceAge::UnknownDate`]): it counts as stale (the verdict gate
//!   `form::price_to_gate_state`), named « date inconnue » apart from « périmé »; writing a price
//!   again — retyped or fetched — records a readable origin and makes it fresh
//!   ([`same_price_renews`]);
//! - the date **shown** for a price's origin (« récupéré le … », « saisi le … » — traceability,
//!   study PDF) is that same day, read in the same zone ([`price_date_shown`]), so the shown day
//!   never contradicts the age. A provider's session date (« séance du … ») is shown as given.
//!
//! There is no exchange-holiday calendar (accepted limitation): after a holiday, the last close
//! counts as one trading day older than it is, so with the default horizon it is « périmé » until
//! the next close is fetched.

use steadyinvest_contract::{Judgment, PriceOrigin, Source, Study, Timestamp};

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

    /// The day of a calendar date.
    fn of_date(d: chrono::NaiveDate) -> Day {
        use chrono::Datelike;
        Day(days_from_civil(
            i64::from(d.year()),
            i64::from(d.month()),
            i64::from(d.day()),
        ))
    }

    /// `JJ/MM/AAAA` — the day as the traceability and the study PDF print it.
    pub fn jj_mm_aaaa(self) -> String {
        self.formatted("%d/%m/%Y")
    }

    /// `JJ/MM` — the day without its year (the frozen verdict's and the provenance's short date).
    pub fn jj_mm(self) -> String {
        self.formatted("%d/%m")
    }

    /// `AAAA-MM-JJ` — the day in ISO form (the study list, the comparison's column dates).
    pub fn iso(self) -> String {
        self.formatted("%Y-%m-%d")
    }

    fn formatted(self, pattern: &str) -> String {
        // 719 163 = days from 0001-01-01 (CE day 1) to 1970-01-01.
        i32::try_from(self.0 + 719_163)
            .ok()
            .and_then(chrono::NaiveDate::from_num_days_from_ce_opt)
            .map(|d| d.format(pattern).to_string())
            .unwrap_or_default()
    }

    /// Saturday or Sunday.
    fn is_weekend(self) -> bool {
        // 1970-01-01 was a Thursday: index 0 = Thursday … 2 = Saturday, 3 = Sunday.
        matches!(self.0.rem_euclid(7), 2 | 3)
    }
}

/// The time zone in which an app-clock stamp (RFC 3339 UTC) is read as a **day** — THE one
/// reading of a stamp's day: the age computation, « today », [`same_price_renews`] and the app's
/// « written today » test all go through [`DayZone::day_of`]. Owner decision (Guy 2026-10-03):
/// the trading day is counted in the owner's **local** time — a price typed at 00:30 in Zurich is
/// that day's, not the day before. Injected so tests never depend on the machine's zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayZone {
    /// The machine's local time zone (`chrono::Local`, its offset at each instant, so a summer
    /// stamp read in winter keeps its summer day) — the running app and the MCP server.
    Local,
    /// A fixed offset from UTC, in seconds east (tests; `0` is UTC).
    FixedSecondsEast(i32),
}

impl DayZone {
    /// UTC (a fixed zero offset).
    pub const UTC: DayZone = DayZone::FixedSecondsEast(0);

    /// The day of a stamp in this zone; a bare `YYYY-MM-DD` is already a day (taken as given);
    /// `None` when unreadable (or for an offset out of range).
    pub fn day_of(self, t: &Timestamp) -> Option<Day> {
        let Ok(at) = chrono::DateTime::parse_from_rfc3339(&t.0) else {
            return if t.0.len() == 10 {
                Day::parse(&t.0)
            } else {
                None
            };
        };
        let date = match self {
            DayZone::Local => at.with_timezone(&chrono::Local).date_naive(),
            DayZone::FixedSecondsEast(secs) => at
                .with_timezone(&chrono::FixedOffset::east_opt(secs)?)
                .date_naive(),
        };
        Some(Day::of_date(date))
    }

    /// Today, from the injected clock's `now` — the same day reading as a price's.
    pub fn today(self, now: &Timestamp) -> Option<Day> {
        self.day_of(now)
    }

    /// The time of day (`HH:MM`) of an RFC 3339 stamp in this zone; `None` for a bare date or an
    /// unreadable stamp.
    pub fn hh_mm(self, t: &Timestamp) -> Option<String> {
        let at = chrono::DateTime::parse_from_rfc3339(&t.0).ok()?;
        Some(match self {
            DayZone::Local => at.with_timezone(&chrono::Local).format("%H:%M").to_string(),
            DayZone::FixedSecondsEast(secs) => at
                .with_timezone(&chrono::FixedOffset::east_opt(secs)?)
                .format("%H:%M")
                .to_string(),
        })
    }

    /// THE shown date of a stamp (owner decision, Guy 2026-10-03: every user-visible timestamp in
    /// the owner's local time, read like the price's age): its day in this zone, written by
    /// `as_shown` (`Day::jj_mm_aaaa`, `Day::jj_mm`, `Day::iso`); an unreadable stamp passes
    /// through unchanged — a display transform, it never repairs a value.
    pub fn shown(self, t: &Timestamp, as_shown: fn(Day) -> String) -> String {
        self.day_of(t).map_or_else(|| t.0.clone(), as_shown)
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

/// The date the price stands for: the provider's session date when known (taken as given), else
/// the local day it was written ([`DayZone::day_of`]). `None` when unreadable (or for a `Derived`
/// origin, which a price never has).
pub fn price_day(origin: &PriceOrigin, zone: DayZone) -> Option<Day> {
    match (origin.source, &origin.session_date) {
        (Source::Provider, Some(session)) => Day::parse(session),
        (Source::Provider | Source::Manual, _) => zone.day_of(&origin.at),
        (Source::Derived, _) => None,
    }
}

/// Can the date this origin stands for be read at all ([`price_day`])? An unreadable one (a
/// malformed stamp or session date, or a `Derived` origin a price never has) is an **unknown
/// date**, exactly like a missing origin: stale, « date inconnue » (owner decision C, Guy
/// 2026-10-03). Readability is a property of the stamp, not of the zone — read in UTC here.
pub fn price_date_known(origin: &PriceOrigin) -> bool {
    price_day(origin, DayZone::UTC).is_some()
}

/// The date shown for a price's origin, `JJ/MM/AAAA`: THE day the age is counted from
/// ([`price_day`] in `zone` — the provider's session date as given, else the **local** day the
/// price was written), so « saisi le … » / « récupéré le … » never contradicts the age (owner
/// decision D, Guy 2026-10-03). `None` when the date cannot be read (« date inconnue »).
pub fn price_date_shown(origin: &PriceOrigin, zone: DayZone) -> Option<String> {
    price_day(origin, zone).map(Day::jj_mm_aaaa)
}

/// Is a price of this origin older than `horizon` trading days on `today`? PURE.
pub fn price_is_aged(origin: &PriceOrigin, today: Day, horizon: u32, zone: DayZone) -> bool {
    price_day(origin, zone).is_some_and(|day| trading_days_after(day, today, horizon) > horizon)
}

/// The current price's age on a read copy marked by [`apply_price_age`] (owner decision C, Guy
/// 2026-10-03): `None` when there is no current price (the missing-input path, unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceAge {
    /// Within the horizon.
    Fresh,
    /// Past the horizon on this read (« périmé »).
    Aged,
    /// No recorded origin — written before origins were recorded — or an origin whose date cannot
    /// be read ([`price_date_known`]): its date is unknown, and it counts as stale
    /// (« date inconnue »), distinct from « périmé ».
    UnknownDate,
}

/// The [`PriceAge`] of `judgment`'s current price — THE one reading the verdict gate
/// (`form::price_to_gate_state`), its open-gate label, the traceability, the PDF and the MCP use.
pub fn current_price_age(judgment: &Judgment) -> Option<PriceAge> {
    judgment.current_price?;
    Some(match &judgment.current_price_origin {
        None => PriceAge::UnknownDate,
        Some(o) if !price_date_known(o) => PriceAge::UnknownDate,
        Some(o) if o.aged => PriceAge::Aged,
        Some(_) => PriceAge::Fresh,
    })
}

/// Does writing the SAME price value again with origin `written` over the `recorded` origin renew
/// it? Yes when the written price stands for a **later day** than the recorded one ([`price_day`]
/// — the age computation's own reading), so a confirmed price becomes fresh again: the owner
/// retyping the same value on a later day, or a fetch on a later day bringing the same quote
/// without a session date. The same day — or the same provider session — stays a no-op (no undo
/// step, no history entry). An unknown recorded date — no origin, or one whose date cannot be
/// read (stale either way) — always renews when the write's own date is readable: the write
/// records a readable origin (owner decision C, 2026-10-03).
pub fn same_price_renews(
    recorded: Option<&PriceOrigin>,
    written: &PriceOrigin,
    zone: DayZone,
) -> bool {
    let Some(new) = price_day(written, zone) else {
        return false;
    };
    match recorded.and_then(|r| price_day(r, zone)) {
        None => true,
        Some(old) => new > old,
    }
}

/// Mark the read copy of `study` with the age rule: [`PriceOrigin::aged`] set when the current
/// price is older than the horizon, cleared otherwise. Never persisted (the contract skips the
/// field) — call it on what is READ for display, on every read (the setting and today move).
pub fn apply_price_age(study: &mut Study, today: Option<Day>, horizon: u32, zone: DayZone) {
    let present = study.judgment.current_price.is_some();
    // No origin, no mark: a price with no recorded origin is [`PriceAge::UnknownDate`], read by
    // the verdict gate itself (`form::price_to_gate_state`).
    // An unreadable date is never called aged either: it is [`PriceAge::UnknownDate`] too.
    if let Some(origin) = study.judgment.current_price_origin.as_mut() {
        origin.aged = present && today.is_some_and(|t| price_is_aged(origin, t, horizon, zone));
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

    const UTC: DayZone = DayZone::UTC;

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
        assert!(
            !price_is_aged(&o, day("2026-10-02"), h, UTC),
            "the session day"
        );
        assert!(!price_is_aged(&o, day("2026-10-04"), h, UTC), "the weekend");
        assert!(
            !price_is_aged(&o, day("2026-10-05"), h, UTC),
            "Monday: one trading day"
        );
        assert!(price_is_aged(&o, day("2026-10-06"), h, UTC), "Tuesday: two");
        assert!(
            !price_is_aged(&o, day("2026-10-06"), 2, UTC),
            "a wider horizon"
        );
        assert!(price_is_aged(&o, day("2026-10-07"), 2, UTC));
    }

    #[test]
    fn a_thursday_close_ages_on_monday() {
        let o = origin(Source::Provider, "2026-10-02T07:00:00Z", Some("2026-10-01"));
        assert!(
            !price_is_aged(&o, day("2026-10-04"), 1, UTC),
            "Sunday: Friday only"
        );
        assert!(
            price_is_aged(&o, day("2026-10-05"), 1, UTC),
            "Monday: Friday and Monday"
        );
    }

    #[test]
    fn the_session_date_wins_over_the_fetch_date() {
        // Fetched on Tuesday, but the close is Thursday's: two trading days old on Tuesday.
        let o = origin(Source::Provider, "2026-10-06T08:00:00Z", Some("2026-10-01"));
        assert!(price_is_aged(&o, day("2026-10-06"), 1, UTC));
        // Without a session date, the fetch date stands.
        let o = origin(Source::Provider, "2026-10-06T08:00:00Z", None);
        assert!(!price_is_aged(&o, day("2026-10-07"), 1, UTC));
        assert!(price_is_aged(&o, day("2026-10-08"), 1, UTC));
    }

    #[test]
    fn a_typed_price_ages_from_its_entry_date() {
        // Typed on a Saturday: Monday is its first trading day, Tuesday its second.
        let o = origin(Source::Manual, "2026-10-03T10:00:00Z", None);
        assert!(!price_is_aged(&o, day("2026-10-05"), 1, UTC));
        assert!(price_is_aged(&o, day("2026-10-06"), 1, UTC));
    }

    #[test]
    fn an_unreadable_date_is_never_called_aged() {
        let o = origin(Source::Provider, "garbage", Some("not-a-date"));
        assert!(!price_is_aged(&o, day("2030-01-01"), 1, UTC));
        let o = origin(Source::Derived, "2000-01-03T00:00:00Z", None);
        assert!(!price_is_aged(&o, day("2030-01-01"), 1, UTC));
    }

    // Owner decision C (Guy 2026-10-03): an origin whose date cannot be read is treated like a
    // missing origin — an unknown date, stale (« date inconnue »), not « périmé ».
    #[test]
    fn an_unreadable_origin_date_is_an_unknown_date() {
        let mut study = crate::form::tests::full_study();
        for unreadable in [
            origin(Source::Manual, "garbage", None),
            origin(Source::Provider, "2026-13-45T10:00:00Z", None),
            origin(Source::Provider, "2026-10-06T10:00:00Z", Some("not-a-date")),
            origin(Source::Derived, "2026-10-06T10:00:00Z", None),
        ] {
            assert!(!price_date_known(&unreadable), "{unreadable:?}");
            assert_eq!(price_date_shown(&unreadable, UTC), None);
            study.judgment.current_price_origin = Some(unreadable);
            apply_price_age(&mut study, Some(day("2026-10-06")), 1, UTC);
            assert_eq!(
                current_price_age(&study.judgment),
                Some(PriceAge::UnknownDate)
            );
            // The verdict gate counts it stale, like a price with no origin.
            assert_eq!(
                crate::form::price_to_gate_state(
                    study.judgment.current_price,
                    study.judgment.current_price_origin.as_ref()
                ),
                steadyinvest_core::verdict::GateState::Stale
            );
        }
        // A readable origin of the same day is fresh.
        study.judgment.current_price_origin =
            Some(origin(Source::Manual, "2026-10-06T10:00:00Z", None));
        apply_price_age(&mut study, Some(day("2026-10-06")), 1, UTC);
        assert_eq!(current_price_age(&study.judgment), Some(PriceAge::Fresh));
    }

    // Owner decision D (Guy 2026-10-03): the date shown for an origin is the day the age counts
    // from — the local day of the stamp, never its UTC date; a session date is shown as given.
    #[test]
    fn the_shown_origin_date_is_the_local_day_of_the_age() {
        let zurich_summer = DayZone::FixedSecondsEast(2 * 3600);
        let new_york = DayZone::FixedSecondsEast(-4 * 3600);
        // Typed at 00:30 in Zurich on Friday (22:30 UTC on Thursday).
        let typed = origin(Source::Manual, "2026-10-01T22:30:00Z", None);
        assert_eq!(
            price_date_shown(&typed, zurich_summer).as_deref(),
            Some("02/10/2026")
        );
        assert_eq!(price_date_shown(&typed, UTC).as_deref(), Some("01/10/2026"));
        // Fetched at 02:00 UTC on Friday = 22:00 on Thursday in New York.
        let fetched = origin(Source::Provider, "2026-10-02T02:00:00Z", None);
        assert_eq!(
            price_date_shown(&fetched, new_york).as_deref(),
            Some("01/10/2026")
        );
        // A provider session date: shown as given, whatever the zone.
        let close = origin(Source::Provider, "2026-10-02T23:30:00Z", Some("2026-10-02"));
        for zone in [UTC, zurich_summer, new_york] {
            assert_eq!(
                price_date_shown(&close, zone).as_deref(),
                Some("02/10/2026")
            );
        }
        // The shown day is the age's day: Friday's price is fresh on Monday in Zurich.
        assert!(!price_is_aged(&typed, day("2026-10-05"), 1, zurich_summer));
        assert_eq!(day("1970-01-01").jj_mm_aaaa(), "01/01/1970");
        assert_eq!(day("2024-02-29").jj_mm_aaaa(), "29/02/2024");
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
        apply_price_age(&mut study, Some(day("2026-10-06")), 1, UTC);
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
        apply_price_age(&mut study, Some(day("2026-10-06")), 5, UTC);
        assert!(!aged(&study));
        // No today (an unreadable clock) → never aged.
        apply_price_age(&mut study, Some(day("2026-10-06")), 1, UTC);
        apply_price_age(&mut study, None, 1, UTC);
        assert!(!aged(&study));
        // No price → nothing to age, whatever the origin says.
        study.judgment.current_price = None;
        apply_price_age(&mut study, Some(day("2026-10-06")), 1, UTC);
        assert!(!aged(&study));
    }

    #[test]
    fn the_same_price_renews_only_on_a_later_day() {
        let typed_tue = origin(Source::Manual, "2026-09-29T10:00:00Z", None);
        let typed_thu = origin(Source::Manual, "2026-10-01T09:00:00Z", None);
        let typed_thu_late = origin(Source::Manual, "2026-10-01T18:00:00Z", None);
        assert!(same_price_renews(Some(&typed_tue), &typed_thu, UTC));
        assert!(
            !same_price_renews(Some(&typed_thu), &typed_thu_late, UTC),
            "same day"
        );
        assert!(
            !same_price_renews(Some(&typed_thu), &typed_tue, UTC),
            "earlier"
        );
        // A fetch without a session date: its fetch date is its day.
        let fetched_tue = origin(Source::Provider, "2026-09-29T10:00:00Z", None);
        let fetched_thu = origin(Source::Provider, "2026-10-01T09:00:00Z", None);
        assert!(same_price_renews(Some(&fetched_tue), &fetched_thu, UTC));
        // The same session fetched later: the session is its day — nothing renews.
        let session_tue = origin(Source::Provider, "2026-09-29T20:00:00Z", Some("2026-09-29"));
        let session_tue_again =
            origin(Source::Provider, "2026-10-01T09:00:00Z", Some("2026-09-29"));
        assert!(!same_price_renews(
            Some(&session_tue),
            &session_tue_again,
            UTC
        ));
        // A typed confirmation today renews yesterday's close.
        assert!(same_price_renews(Some(&session_tue), &typed_thu, UTC));
        // Unknown origin (date unknown, so stale — decision C): any write renews, recording one.
        assert!(same_price_renews(None, &typed_thu, UTC));
        // An unreadable recorded date is an unknown date (stale), like no origin: a write renews.
        let garbage = origin(Source::Manual, "garbage", None);
        assert!(same_price_renews(Some(&garbage), &typed_thu, UTC));
        // A write whose own date cannot be read never renews.
        assert!(!same_price_renews(None, &garbage, UTC));
        assert_eq!(
            UTC.today(&Timestamp("2026-10-01T23:59:59Z".to_string())),
            Some(day("2026-10-01"))
        );
    }

    // Owner decision D (Guy 2026-10-03): a stamp's day is the LOCAL day — injected, never the
    // machine's zone in a test.
    #[test]
    fn a_stamp_is_read_as_its_local_day() {
        let zurich_summer = DayZone::FixedSecondsEast(2 * 3600);
        let new_york = DayZone::FixedSecondsEast(-4 * 3600);
        let late = Timestamp("2026-10-01T22:30:00Z".to_string());
        assert_eq!(UTC.day_of(&late), Some(day("2026-10-01")));
        assert_eq!(
            zurich_summer.day_of(&late),
            Some(day("2026-10-02")),
            "00:30 in Zurich"
        );
        let early = Timestamp("2026-10-02T02:00:00Z".to_string());
        assert_eq!(
            new_york.day_of(&early),
            Some(day("2026-10-01")),
            "22:00 the evening before"
        );
        // A non-UTC stamp is converted too; a bare date is taken as given; garbage is unreadable.
        let offset = Timestamp("2026-10-02T01:00:00+02:00".to_string());
        assert_eq!(UTC.day_of(&offset), Some(day("2026-10-01")));
        assert_eq!(
            new_york.day_of(&Timestamp("2026-10-02".to_string())),
            Some(day("2026-10-02"))
        );
        assert_eq!(UTC.day_of(&Timestamp("garbage".to_string())), None);
        assert_eq!(
            DayZone::FixedSecondsEast(100_000).day_of(&late),
            None,
            "an offset out of range"
        );
        // The real local zone reads some day (which one depends on the machine — not asserted).
        assert!(DayZone::Local.day_of(&late).is_some());
        // A price typed at 23:30 UTC on Thursday is Friday's in Zurich: on Monday it is one
        // trading day old there (fresh at horizon 1), two in UTC (aged).
        let typed = origin(Source::Manual, "2026-10-01T23:30:00Z", None);
        assert!(!price_is_aged(&typed, day("2026-10-05"), 1, zurich_summer));
        assert!(price_is_aged(&typed, day("2026-10-05"), 1, UTC));
        // The provider's session date is a date: no zone moves it.
        let close = origin(Source::Provider, "2026-10-02T23:30:00Z", Some("2026-10-02"));
        assert_eq!(price_day(&close, zurich_summer), Some(day("2026-10-02")));
        // The same price retyped the same local day is a no-op, the next local day renews it.
        let evening = origin(Source::Manual, "2026-10-01T21:00:00Z", None);
        let after_midnight = origin(Source::Manual, "2026-10-01T22:30:00Z", None);
        assert!(!same_price_renews(Some(&evening), &after_midnight, UTC));
        assert!(same_price_renews(
            Some(&evening),
            &after_midnight,
            zurich_summer
        ));
    }

    // Owner decision C (Guy 2026-10-03): a present price with no recorded origin has an unknown
    // date — stale, named apart from « périmé »; no price is the missing-input path.
    #[test]
    fn a_price_without_origin_has_an_unknown_date() {
        let mut study = crate::form::tests::full_study();
        study.judgment.current_price_origin = None;
        apply_price_age(&mut study, Some(day("2026-10-06")), 1, UTC);
        assert_eq!(
            current_price_age(&study.judgment),
            Some(PriceAge::UnknownDate)
        );
        study.judgment.current_price_origin =
            Some(origin(Source::Manual, "2026-10-06T10:00:00Z", None));
        apply_price_age(&mut study, Some(day("2026-10-06")), 1, UTC);
        assert_eq!(current_price_age(&study.judgment), Some(PriceAge::Fresh));
        apply_price_age(&mut study, Some(day("2026-10-08")), 1, UTC);
        assert_eq!(current_price_age(&study.judgment), Some(PriceAge::Aged));
        study.judgment.current_price = None;
        assert_eq!(current_price_age(&study.judgment), None);
        study.judgment.current_price_origin = None;
        assert_eq!(
            current_price_age(&study.judgment),
            None,
            "no price: missing"
        );
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
