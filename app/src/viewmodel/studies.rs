//! Study list view-model adapter (Story 2.2): map `persistence::StudySummary` into the Slint
//! `StudyRow` struct for the dashboard list. Presentation only — nothing here calculates (Cardinal
//! Rule). The reopened-study **form** adapter (header + §3 P/E rows) lives in [`crate::viewmodel::form`]
//! (Story 2.3); the 2.2 minimal restore view it replaced — and its `detail()` string builder — are
//! gone now that the faithful §1–§5 form renders the open study.

use std::collections::HashMap;

use rust_decimal::Decimal;
use steadyinvest_core::ssg::{ReturnOutputs, UpsideDownside};
use steadyinvest_persistence::StudySummary;
use uuid::Uuid;

use crate::StudyRow;
use crate::state::created_at_date;
use steadyinvest_report::price_age::DayZone;

/// One study's per-refresh derived list facts. Computed app-side via `build_snapshot` (Cardinal Rule:
/// `curate` stays pure and only READS this map):
/// - issue #107 — the estimated potential (§5 projected total annualized return): `value` used for
///   sorting (`None` when the study withholds it — sorts LAST, never a top pick) and the already-
///   locale-formatted `display` string ("—" when absent);
/// - issue #148 — `incomplete`: the study still has a MISSING load-bearing input (verdict `Withheld`),
///   the list-level "à compléter" signal;
/// - 2026-07-12 — `zone`: the present-price zone key (`engine::zone_key`), the list-level coloured
///   dot + noun ("" = unknown/outside the band → nothing shown, never a guess).
#[derive(Debug, Clone)]
pub struct StudyReturn {
    pub value: Option<Decimal>,
    pub display: String,
    pub incomplete: bool,
    pub zone: &'static str,
    /// Guy's on-screen test (2026-09-30) — the upside/downside ratio, built by [`ud_facts`]. The
    /// list deliberately DIFFERS from the study screen in one place (Guy, 2026-10-01/03): a
    /// withheld verdict lists « — » (the study screen still shows the engine's ratio over the open
    /// inputs). An undefined ratio (price ≤ forecast low) is « ∞ » and an unknown one « — » on
    /// both (owner decision, 2026-10-03).
    pub ud: String,
    /// Guy, 2026-10-01/03 (« Tri : U/D »): the sort value behind `ud` — `None` exactly when `ud`
    /// reads « — » (withheld verdict or unknown ratio), which sorts LAST in both directions.
    pub ud_value: Option<UdRank>,
    /// 2026-07-12 — the study's user-entered company name (empty when unset), shown after the
    /// ticker on the list row. Owned (read off the full study during refresh, not the summary).
    pub company_name: String,
    /// Issue #294 (Guy, 2026-10-03): the U/D emoticon beside the ticker, classified off `ud_value`
    /// by [`ud_mood`] — `None` exactly when the U/D column reads « — ».
    pub ud_mood: Option<UdMood>,
    /// Issue #294: the verdict is provisional (e.g. a stale or undated price, PR #293) — the
    /// emoticon is drawn greyed. Never set for a withheld verdict (no emoticon there).
    pub provisional: bool,
}

impl Default for StudyReturn {
    /// An unknown potential: no sort value, and the app's faithful em-dash (never `0`) for display —
    /// so a study missing from the map (or one that withholds the return) reads "—", not blank. Not
    /// flagged incomplete: absence from the map is "unknown", never a false "à compléter" shout.
    /// No zone or company name either — absent facts stay absent.
    fn default() -> Self {
        StudyReturn {
            value: None,
            display: crate::viewmodel::form::EMPTY_SLOT.to_string(),
            ud: crate::viewmodel::form::EMPTY_SLOT.to_string(),
            ud_value: None,
            incomplete: false,
            zone: "",
            company_name: String::new(),
            ud_mood: None,
            provisional: false,
        }
    }
}

/// The list's « no value » cell: the faithful em-dash and nothing to sort on — it sorts LAST in
/// both directions (`curate`).
fn no_value<T>() -> (String, Option<T>) {
    (crate::viewmodel::form::EMPTY_SLOT.to_string(), None)
}

/// THE « withheld → no value » rule (Guy, 2026-10-03, decision B), shared by the potentiel and
/// U/D columns: a study whose verdict is withheld (`incomplete` — a MISSING load-bearing input)
/// lists « — » and no sort value, even when the engine could state a figure over the inputs it
/// has — the list does not rank on figures built over open inputs. Deliberately unlike the study
/// screen, which still shows those figures beside the withheld verdict. Otherwise `stated` decides.
pub fn unless_withheld<T>(
    incomplete: bool,
    stated: impl FnOnce() -> (String, Option<T>),
) -> (String, Option<T>) {
    if incomplete { no_value() } else { stated() }
}

/// The U/D sort value (Guy, 2026-10-03): a stated ratio, or the unbounded one, which ranks above
/// every ratio (variant order: `Ratio < Unbounded`, ratios by exact decimal) — first in descending
/// order, last in ascending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UdRank {
    Ratio(Decimal),
    Unbounded,
}

/// The list's U/D facts: the displayed ratio and its sort value. Withheld verdict → « — », no
/// value ([`unless_withheld`]); otherwise THE screens' formatter (`engine::fmt_ud`): `Ratio` →
/// « 3,4:1 »; `Undefined` → « ∞ » (`steadyinvest_report::UD_UNBOUNDED`), ranked as +∞ (decision
/// A — the MOST favourable case, not an absence); `Unknown` → « — », no value.
pub fn ud_facts(
    ud: &UpsideDownside,
    incomplete: bool,
    format: crate::viewmodel::format::NumberFormat,
) -> (String, Option<UdRank>) {
    unless_withheld(incomplete, || {
        let rank = match ud {
            UpsideDownside::Ratio(d) => UdRank::Ratio(*d),
            UpsideDownside::Undefined => UdRank::Unbounded,
            UpsideDownside::Unknown => return no_value(),
        };
        (crate::viewmodel::engine::fmt_ud(ud, format), Some(rank))
    })
}

/// Issue #294 (Guy, 2026-10-03): the default high U/D threshold of the list's emoticon — 3, the
/// NAIC 25/50/25 boundary (owner's wording). Canonical spelling (see [`parse_ud_threshold`]).
pub const DEFAULT_UD_HIGH: &str = "3";
/// Issue #294: the default low U/D threshold — one third, held as the fraction « 1/3 » (exact
/// input; see [`parse_ud_threshold`] for how it compares).
pub const DEFAULT_UD_LOW: &str = "1/3";

/// A U/D threshold's canonical spelling → its value (issue #294). Two forms, both strictly
/// positive:
/// - a decimal (« 3 », « 0.33 » — the canonical, point spelling app-config stores; the Réglages
///   field reads it in the user's number format);
/// - a fraction of two whole numbers « n/d » (« 1/3 »), `d > 0`.
///
/// Exact decimals, never `f64`. A fraction's value is the `Decimal` quotient `n / d` — the SAME
/// `checked_div` the core uses for the ratio (`(high − price) / (price − low)`), so a ratio of
/// exactly one third lands ON the « 1/3 » threshold, not a rounding hair below it (tested).
pub fn parse_ud_threshold(canonical: &str) -> Option<Decimal> {
    let s = canonical.trim();
    let value = match s.split_once('/') {
        Some((n, d)) => {
            let whole = |part: &str| {
                let part = part.trim();
                (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
                    .then(|| Decimal::from_str_exact(part).ok())
                    .flatten()
            };
            let (n, d) = (whole(n)?, whole(d)?);
            if d.is_zero() {
                return None;
            }
            n.checked_div(d)?
        }
        None => Decimal::from_str_exact(s).ok()?,
    };
    (value > Decimal::ZERO).then_some(value)
}

/// The pair of U/D thresholds the list's emoticon compares against (issue #294); `low < high`
/// always (the config accessor falls back to the defaults whole otherwise, the size-table rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdThresholds {
    pub low: Decimal,
    pub high: Decimal,
}

impl UdThresholds {
    /// A pair from its canonical spellings: `None` unless both parse and `low < high` — the ONE
    /// ordering rule behind the config accessor and the list's classification.
    pub fn from_spellings(high: &str, low: &str) -> Option<Self> {
        let (high, low) = (parse_ud_threshold(high)?, parse_ud_threshold(low)?);
        (low < high).then_some(UdThresholds { low, high })
    }
}

impl Default for UdThresholds {
    fn default() -> Self {
        UdThresholds::from_spellings(DEFAULT_UD_HIGH, DEFAULT_UD_LOW)
            .expect("the default thresholds parse and are ordered")
    }
}

/// The list's U/D emoticon (issue #294, Guy 2026-10-03). Screen only, never in a PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdMood {
    /// 🙂 — the ratio is at or above the high threshold.
    High,
    /// 🙂 — the ratio is undefined: price at or below the forecast low (« ∞ », decision A).
    Unbounded,
    /// 😐 — between the thresholds (`low ≤ ratio < high`).
    Middle,
    /// 🙁 — the ratio is below the low threshold.
    Low,
}

impl UdMood {
    /// The wire key the `.slint` row reads (`StudyRow.ud-mood`); "" = no emoticon.
    pub fn key(self) -> &'static str {
        match self {
            UdMood::High => "high",
            UdMood::Unbounded => "unbounded",
            UdMood::Middle => "middle",
            UdMood::Low => "low",
        }
    }
}

/// THE emoticon classification (issue #294), pure, keyed off the U/D column's own sort value so
/// the two never disagree: no value (« — » — withheld verdict, unknown ratio) → no emoticon;
/// `Unbounded` (« ∞ ») → 🙂; a ratio `≥ high` → 🙂, `< low` → 🙁, otherwise 😐. Edges: exactly
/// `high` is 🙂, exactly `low` is 😐. Exact decimal comparisons.
pub fn ud_mood(rank: Option<UdRank>, thresholds: UdThresholds) -> Option<UdMood> {
    Some(match rank? {
        UdRank::Unbounded => UdMood::Unbounded,
        UdRank::Ratio(r) if r >= thresholds.high => UdMood::High,
        UdRank::Ratio(r) if r < thresholds.low => UdMood::Low,
        UdRank::Ratio(_) => UdMood::Middle,
    })
}

/// The list's potentiel facts (issue #107, #189; Guy, 2026-10-03, decision B): withheld verdict
/// → « — », no value ([`unless_withheld`], as the U/D column); otherwise the §5 projected total
/// annualized return, or the appreciation-only fallback marked « (hors div.) » (#189), « — » when
/// neither is stated.
pub fn potential_facts(
    returns: &ReturnOutputs,
    incomplete: bool,
    format: crate::viewmodel::format::NumberFormat,
) -> (String, Option<Decimal>) {
    unless_withheld(incomplete, || {
        (
            crate::viewmodel::engine::fmt_total_return(returns, format),
            returns
                .projected_total_annualized_return_pct
                .or_else(|| returns.appreciation_only_potential()),
        )
    })
}

/// Map one summary row into the Slint `StudyRow` (id stringified, date trimmed to the day, status
/// verbatim, the pre-formatted potential-return string — "—" when the study withholds it — the
/// issue #148 `incomplete` flag driving the "à compléter" marker, the present-price zone key, the
/// company name shown after the ticker, and the issue #294 U/D emoticon with its greyed flag; the
/// created date is the local day of `day_zone`).
pub fn to_row(summary: &StudySummary, facts: &StudyReturn, day_zone: DayZone) -> StudyRow {
    StudyRow {
        id: summary.id.to_string().into(),
        ticker: summary.security_ticker.clone().into(),
        created_at: created_at_date(&summary.created_at, day_zone).into(),
        status: summary.status.clone().into(),
        potential_return: facts.display.as_str().into(),
        ud_ratio: facts.ud.as_str().into(),
        incomplete: facts.incomplete,
        zone: facts.zone.into(),
        company_name: facts.company_name.as_str().into(),
        // Issue #294: the emoticon key ("" = none) and its greyed flag — the flag only matters
        // when there is an emoticon, so it is cleared with it.
        ud_mood: facts.ud_mood.map(UdMood::key).unwrap_or_default().into(),
        ud_mood_greyed: facts.provisional && facts.ud_mood.is_some(),
        // Story 8.5a: filled after curation from the draft inbox's counts
        // (`wiring::drafts::apply_row_counts`).
        pending_drafts: 0,
    }
}

/// Which lifecycle states the dashboard list shows (Story 2.12, FR54). `Active` is the default view;
/// `Archived` surfaces hidden studies (to re-open or un-archive); `All` shows everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    Active,
    Archived,
    All,
}

impl StatusFilter {
    /// Map the Slint wire string; anything unrecognized falls back to the safe default (`Active`).
    pub fn from_wire(s: &str) -> Self {
        match s {
            "archived" => Self::Archived,
            "all" => Self::All,
            _ => Self::Active,
        }
    }

    fn admits(self, status: &str) -> bool {
        match self {
            Self::Active => status == "active",
            Self::Archived => status == "archived",
            Self::All => true,
        }
    }
}

/// The dashboard sort key (Story 2.12, FR54): created-date, ticker, potential or U/D ratio. `id` is
/// always the deterministic tiebreaker so the list never jitters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Date,
    Ticker,
    /// Issue #107: the estimated potential (§5 projected total annualized return). Studies whose
    /// potential is not stated — or whose verdict is withheld (Guy, 2026-10-03, decision B,
    /// [`potential_facts`]) — sort LAST in both directions.
    PotentialReturn,
    /// Guy, 2026-10-01/03: the upside/downside ratio (`StudyReturn::ud_value`, [`UdRank`]: exact
    /// decimals, an undefined ratio as +∞ — FIRST descending, last ascending). A study whose
    /// verdict is withheld or whose ratio is unknown sorts LAST in both directions.
    UpsideDownside,
}

impl SortKey {
    /// Map the Slint wire string; anything unrecognized falls back to `Date`.
    pub fn from_wire(s: &str) -> Self {
        match s {
            "ticker" => Self::Ticker,
            "potential" => Self::PotentialReturn,
            "ud" => Self::UpsideDownside,
            _ => Self::Date,
        }
    }
}

/// Pure dashboard curation (Story 2.12, FR54): filter by lifecycle status + case-insensitive ticker
/// substring, then **stable**-sort by date, ticker, potential or U/D ratio (asc/desc) with `id` as
/// the deterministic tiebreaker. No I/O, no calculation (Cardinal Rule) — the testable heart of the
/// dashboard. The caller passes the persistence `created_at, id`-ordered summaries; tickers/search
/// text are user data (never posture-scanned).
pub fn curate(
    summaries: &[StudySummary],
    query: &str,
    sort_key: SortKey,
    descending: bool,
    status_filter: StatusFilter,
    returns: &HashMap<Uuid, StudyReturn>,
    zone: DayZone,
) -> Vec<StudyRow> {
    use std::cmp::Ordering;
    let needle = query.trim().to_lowercase();
    let mut kept: Vec<&StudySummary> = summaries
        .iter()
        .filter(|s| status_filter.admits(&s.status))
        .filter(|s| needle.is_empty() || s.security_ticker.to_lowercase().contains(&needle))
        .collect();
    kept.sort_by(|a, b| {
        // The primary comparison; the `id` tiebreak + the descending flip are applied after. A
        // PotentialReturn / UpsideDownside study with NO value (withheld verdict, unknown figure)
        // sorts LAST in both directions (an early return, bypassing the flip) — never a top pick.
        // An undefined U/D ratio HAS a value (`UdRank::Unbounded`, +∞): it flips like any ratio.
        fn known_first<T: Ord>(va: Option<T>, vb: Option<T>) -> Result<Ordering, Ordering> {
            match (va, vb) {
                (Some(x), Some(y)) => Ok(x.cmp(&y)),
                (Some(_), None) => Err(Ordering::Less), // known before unknown, always
                (None, Some(_)) => Err(Ordering::Greater),
                (None, None) => Ok(Ordering::Equal),
            }
        }
        let ord = match sort_key {
            SortKey::Date => a.created_at.0.cmp(&b.created_at.0),
            SortKey::Ticker => a
                .security_ticker
                .to_lowercase()
                .cmp(&b.security_ticker.to_lowercase()),
            SortKey::PotentialReturn => {
                let va = returns.get(&a.id).and_then(|r| r.value);
                let vb = returns.get(&b.id).and_then(|r| r.value);
                match known_first(va, vb) {
                    Ok(ord) => ord,
                    Err(sunk) => return sunk,
                }
            }
            SortKey::UpsideDownside => {
                let va = returns.get(&a.id).and_then(|r| r.ud_value);
                let vb = returns.get(&b.id).and_then(|r| r.ud_value);
                match known_first(va, vb) {
                    Ok(ord) => ord,
                    Err(sunk) => return sunk,
                }
            }
        }
        .then(a.id.cmp(&b.id));
        if descending { ord.reverse() } else { ord }
    });
    let empty = StudyReturn::default();
    kept.iter()
        .map(|s| {
            let facts = returns.get(&s.id).unwrap_or(&empty);
            to_row(s, facts, zone)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::Timestamp;
    use uuid::Uuid;

    fn sm(id: u128, ticker: &str, date: &str, status: &str) -> StudySummary {
        StudySummary {
            id: Uuid::from_u128(id),
            security_ticker: ticker.to_string(),
            created_at: Timestamp(date.to_string()),
            status: status.to_string(),
        }
    }

    fn tickers(rows: &[StudyRow]) -> Vec<String> {
        rows.iter().map(|r| r.ticker.to_string()).collect()
    }

    fn sample() -> Vec<StudySummary> {
        vec![
            sm(1, "NESN", "2026-01-10T00:00:00Z", "active"),
            sm(2, "ROG", "2026-03-02T00:00:00Z", "active"),
            sm(3, "ABBN", "2026-02-15T00:00:00Z", "archived"),
        ]
    }

    /// No computed potentials — the non-PotentialReturn tests don't depend on them.
    fn no_returns() -> HashMap<Uuid, StudyReturn> {
        HashMap::new()
    }

    /// A potential value keyed by the `Uuid::from_u128(id)` the `sm` helper mints.
    fn ret(value: &str, display: &str) -> StudyReturn {
        StudyReturn {
            value: Some(Decimal::from_str_exact(value).unwrap()),
            display: display.to_string(),
            ud: "—".to_string(),
            ud_value: None,
            incomplete: false,
            zone: "",
            company_name: String::new(),
            ud_mood: None,
            provisional: false,
        }
    }

    #[test]
    fn status_filter_active_hides_archived() {
        let rows = curate(
            &sample(),
            "",
            SortKey::Date,
            false,
            StatusFilter::Active,
            &no_returns(),
            DayZone::UTC,
        );
        assert_eq!(
            tickers(&rows),
            vec!["NESN", "ROG"],
            "archived ABBN is hidden"
        );
    }

    #[test]
    fn status_filter_archived_and_all() {
        let archived = curate(
            &sample(),
            "",
            SortKey::Date,
            false,
            StatusFilter::Archived,
            &no_returns(),
            DayZone::UTC,
        );
        assert_eq!(tickers(&archived), vec!["ABBN"]);
        let all = curate(
            &sample(),
            "",
            SortKey::Date,
            false,
            StatusFilter::All,
            &no_returns(),
            DayZone::UTC,
        );
        assert_eq!(all.len(), 3, "all shows active + archived");
    }

    #[test]
    fn search_is_case_insensitive_ticker_substring() {
        let rows = curate(
            &sample(),
            "bb",
            SortKey::Date,
            false,
            StatusFilter::All,
            &no_returns(),
            DayZone::UTC,
        );
        assert_eq!(
            tickers(&rows),
            vec!["ABBN"],
            "substring match, case-insensitive"
        );
        let none = curate(
            &sample(),
            "ZZZ",
            SortKey::Date,
            false,
            StatusFilter::All,
            &no_returns(),
            DayZone::UTC,
        );
        assert!(none.is_empty(), "no match → empty");
    }

    #[test]
    fn sort_by_date_and_ticker_both_directions() {
        let by_date = curate(
            &sample(),
            "",
            SortKey::Date,
            false,
            StatusFilter::All,
            &no_returns(),
            DayZone::UTC,
        );
        assert_eq!(
            tickers(&by_date),
            vec!["NESN", "ABBN", "ROG"],
            "date ascending"
        );
        let by_date_desc = curate(
            &sample(),
            "",
            SortKey::Date,
            true,
            StatusFilter::All,
            &no_returns(),
            DayZone::UTC,
        );
        assert_eq!(
            tickers(&by_date_desc),
            vec!["ROG", "ABBN", "NESN"],
            "date descending"
        );
        let by_ticker = curate(
            &sample(),
            "",
            SortKey::Ticker,
            false,
            StatusFilter::All,
            &no_returns(),
            DayZone::UTC,
        );
        assert_eq!(
            tickers(&by_ticker),
            vec!["ABBN", "NESN", "ROG"],
            "ticker A→Z"
        );
    }

    #[test]
    fn sort_tiebreaks_on_id_deterministically() {
        // Two studies, same date + same ticker — `id` breaks the tie, so the order is stable.
        let same = vec![
            sm(2, "AAA", "2026-01-01T00:00:00Z", "active"),
            sm(1, "AAA", "2026-01-01T00:00:00Z", "active"),
        ];
        let rows = curate(
            &same,
            "",
            SortKey::Date,
            false,
            StatusFilter::All,
            &no_returns(),
            DayZone::UTC,
        );
        let ids: Vec<String> = rows.iter().map(|r| r.id.to_string()).collect();
        assert_eq!(
            ids[0],
            Uuid::from_u128(1).to_string(),
            "lower id first (deterministic)"
        );
    }

    #[test]
    fn sort_by_potential_orders_by_value_and_sinks_the_unknown() {
        // NESN 12 %, ROG 8 %, ABBN(active here) has NO computed potential.
        let studies = vec![
            sm(1, "NESN", "2026-01-10T00:00:00Z", "active"),
            sm(2, "ROG", "2026-03-02T00:00:00Z", "active"),
            sm(3, "ABBN", "2026-02-15T00:00:00Z", "active"),
        ];
        let mut returns = HashMap::new();
        returns.insert(Uuid::from_u128(1), ret("12", "12,0 %"));
        returns.insert(Uuid::from_u128(2), ret("8", "8,0 %"));
        // id 3 (ABBN) deliberately absent → unknown potential.

        // Descending (the default): highest potential first, the unknown always LAST.
        let desc = curate(
            &studies,
            "",
            SortKey::PotentialReturn,
            true,
            StatusFilter::All,
            &returns,
            DayZone::UTC,
        );
        assert_eq!(tickers(&desc), vec!["NESN", "ROG", "ABBN"]);
        // The formatted potential rides along on the row.
        assert_eq!(desc[0].potential_return.to_string(), "12,0 %");
        assert_eq!(
            desc[2].potential_return.to_string(),
            "—",
            "unknown potential → the faithful em-dash"
        );

        // Ascending: lowest known first, but the unknown STILL sinks last (never a top pick).
        let asc = curate(
            &studies,
            "",
            SortKey::PotentialReturn,
            false,
            StatusFilter::All,
            &returns,
            DayZone::UTC,
        );
        assert_eq!(tickers(&asc), vec!["ROG", "NESN", "ABBN"]);
    }

    /// A U/D ratio keyed by the `Uuid::from_u128(id)` the `sm` helper mints (`None` → « — »).
    fn ud(value: Option<&str>, display: &str) -> StudyReturn {
        StudyReturn {
            ud: display.to_string(),
            ud_value: value.map(|v| UdRank::Ratio(Decimal::from_str_exact(v).unwrap())),
            ..StudyReturn::default()
        }
    }

    /// An undefined U/D ratio (price ≤ forecast low): « ∞ », ranked +∞ (decision A).
    fn ud_unbounded() -> StudyReturn {
        StudyReturn {
            ud: steadyinvest_report::UD_UNBOUNDED.to_string(),
            ud_value: Some(UdRank::Unbounded),
            ..StudyReturn::default()
        }
    }

    #[test]
    fn ud_facts_dash_a_withheld_verdict_and_an_unknown_ratio_but_not_an_undefined_one() {
        let format = crate::viewmodel::format::NumberFormat::default();
        let ratio = UpsideDownside::Ratio(Decimal::from_str_exact("3.4").unwrap());
        let (shown, value) = ud_facts(&ratio, false, format);
        assert_eq!(
            value,
            Some(UdRank::Ratio(Decimal::from_str_exact("3.4").unwrap()))
        );
        assert_eq!(shown, crate::viewmodel::engine::fmt_ud(&ratio, format));
        assert!(
            shown.ends_with(":1"),
            "a stated ratio reads « …:1 », got {shown}"
        );
        // Decision A: an undefined ratio (price ≤ forecast low) is the most favourable case —
        // « ∞ », ranked above every ratio; never a dash.
        assert_eq!(
            ud_facts(&UpsideDownside::Undefined, false, format),
            ("∞".to_string(), Some(UdRank::Unbounded))
        );
        assert!(UdRank::Unbounded > UdRank::Ratio(Decimal::MAX));
        // Unknown: « — », no value.
        assert_eq!(
            ud_facts(&UpsideDownside::Unknown, false, format),
            ("—".to_string(), None)
        );
        // Withheld verdict: whatever the engine has, the list states none.
        for any in [ratio, UpsideDownside::Undefined, UpsideDownside::Unknown] {
            assert_eq!(ud_facts(&any, true, format), ("—".to_string(), None));
        }
    }

    #[test]
    fn potential_facts_dash_a_withheld_verdict_through_the_shared_rule() {
        // Decision B: the potentiel column applies the SAME `unless_withheld` rule as the U/D
        // column — a withheld verdict lists « — » and no sort value, even over a stated total.
        let format = crate::viewmodel::format::NumberFormat::default();
        let dec = |s: &str| Decimal::from_str_exact(s).unwrap();
        let returns = ReturnOutputs {
            present_yield_pct: None,
            avg_annual_eps: Some(dec("2")),
            avg_annual_dividend: None,
            avg_yield_pct: None,
            projected_appreciation_pct: None,
            projected_annualized_appreciation_pct: Some(dec("20")),
            projected_total_annualized_return_pct: Some(dec("23.28")),
        };
        let (shown, value) = potential_facts(&returns, false, format);
        assert_eq!(value, Some(dec("23.28")));
        assert_eq!(
            shown,
            crate::viewmodel::engine::fmt_total_return(&returns, format)
        );
        assert_eq!(
            potential_facts(&returns, true, format),
            ("—".to_string(), None)
        );
        // #189's appreciation-only fallback still feeds a complete study…
        let no_div = ReturnOutputs {
            projected_total_annualized_return_pct: None,
            ..returns.clone()
        };
        assert_eq!(potential_facts(&no_div, false, format).1, Some(dec("20")));
        // …and is withheld like the rest when the verdict is.
        assert_eq!(
            potential_facts(&no_div, true, format),
            ("—".to_string(), None)
        );
        // The shared rule itself: the stated facts are not even consulted when withheld.
        let (shown, value): (String, Option<u8>) =
            unless_withheld(true, || unreachable!("withheld → never stated"));
        assert_eq!((shown.as_str(), value), ("—", None));
    }

    #[test]
    fn sort_by_ud_puts_an_undefined_ratio_first_descending_and_last_ascending() {
        // Decision A: « ∞ » (undefined: price ≤ forecast low) ranks as +∞ — FIRST descending, LAST
        // among the known ratios ascending — while the dashes (withheld/unknown) stay last in both
        // orders. Two « ∞ » tie on `id`, flipped with the direction like any tie.
        let studies = vec![
            sm(1, "NESN", "2026-01-10T00:00:00Z", "active"),
            sm(2, "ROG", "2026-03-02T00:00:00Z", "active"),
            sm(3, "ABBN", "2026-02-15T00:00:00Z", "active"),
            sm(4, "UBSG", "2026-04-01T00:00:00Z", "active"),
            sm(5, "ZURN", "2026-05-01T00:00:00Z", "active"),
        ];
        let mut returns = HashMap::new();
        returns.insert(Uuid::from_u128(1), ud(Some("9.8"), "9,8:1"));
        returns.insert(Uuid::from_u128(2), ud_unbounded());
        returns.insert(Uuid::from_u128(3), ud(None, "—"));
        returns.insert(Uuid::from_u128(4), ud(Some("1000"), "1 000,0:1"));
        returns.insert(Uuid::from_u128(5), ud_unbounded());

        let desc = curate(
            &studies,
            "",
            SortKey::UpsideDownside,
            true,
            StatusFilter::All,
            &returns,
            DayZone::UTC,
        );
        assert_eq!(
            tickers(&desc),
            vec!["ZURN", "ROG", "UBSG", "NESN", "ABBN"],
            "descending: the two « ∞ » first (id flipped), then the ratios, the dash last"
        );
        assert_eq!(desc[0].ud_ratio.to_string(), "∞");
        assert_eq!(desc[4].ud_ratio.to_string(), "—");

        let asc = curate(
            &studies,
            "",
            SortKey::UpsideDownside,
            false,
            StatusFilter::All,
            &returns,
            DayZone::UTC,
        );
        assert_eq!(
            tickers(&asc),
            vec!["NESN", "UBSG", "ROG", "ZURN", "ABBN"],
            "ascending: the ratios, then the two « ∞ » (id order), the dash STILL last"
        );
    }

    #[test]
    fn sort_by_potential_sinks_a_withheld_verdict_in_both_orders() {
        // Decision B: a withheld verdict has no potentiel value (`potential_facts`), so it sorts
        // last in both orders under « Tri : potentiel », even though the engine stated a total.
        let format = crate::viewmodel::format::NumberFormat::default();
        let dec = |s: &str| Decimal::from_str_exact(s).unwrap();
        let outputs = |total: &str| ReturnOutputs {
            present_yield_pct: None,
            avg_annual_eps: Some(dec("2")),
            avg_annual_dividend: None,
            avg_yield_pct: None,
            projected_appreciation_pct: None,
            projected_annualized_appreciation_pct: None,
            projected_total_annualized_return_pct: Some(dec(total)),
        };
        let fact = |total: &str, incomplete: bool| {
            let (display, value) = potential_facts(&outputs(total), incomplete, format);
            StudyReturn {
                value,
                display,
                incomplete,
                ..StudyReturn::default()
            }
        };
        let studies = vec![
            sm(1, "NESN", "2026-01-10T00:00:00Z", "active"),
            sm(2, "ROG", "2026-03-02T00:00:00Z", "active"),
            sm(3, "ABBN", "2026-02-15T00:00:00Z", "active"),
            sm(4, "UBSG", "2026-04-01T00:00:00Z", "active"),
        ];
        let mut returns = HashMap::new();
        returns.insert(Uuid::from_u128(1), fact("12", false));
        returns.insert(Uuid::from_u128(2), fact("99", true)); // the highest total, but withheld
        returns.insert(Uuid::from_u128(3), fact("5", false));
        returns.insert(Uuid::from_u128(4), fact("1", true)); // withheld too
        let sorted = |descending| {
            curate(
                &studies,
                "",
                SortKey::PotentialReturn,
                descending,
                StatusFilter::All,
                &returns,
                DayZone::UTC,
            )
        };
        let desc = sorted(true);
        assert_eq!(tickers(&desc), vec!["NESN", "ABBN", "UBSG", "ROG"]);
        assert_eq!(desc[2].potential_return.to_string(), "—");
        assert_eq!(desc[3].potential_return.to_string(), "—");
        assert!(desc[3].incomplete, "the « à compléter » marker still rides");
        assert_eq!(
            tickers(&sorted(false)),
            vec!["ABBN", "NESN", "ROG", "UBSG"],
            "ascending: the withheld pair STILL last (id order)"
        );
    }

    #[test]
    fn sort_by_ud_compares_exact_decimals_and_sinks_the_dash() {
        // Exact decimals, not strings: « 10,5:1 » > « 9,8:1 » > « 3,05:1 » > « 3,04:1 » (a string sort
        // would put « 10,5 » before « 3,… » ascending). ABBN's ratio is withheld/unknown (« — »)
        // and a study absent from the map is unknown — both sink LAST in both directions.
        let studies = vec![
            sm(1, "NESN", "2026-01-10T00:00:00Z", "active"),
            sm(2, "ROG", "2026-03-02T00:00:00Z", "active"),
            sm(3, "ABBN", "2026-02-15T00:00:00Z", "active"),
            sm(4, "UBSG", "2026-04-01T00:00:00Z", "active"),
            sm(5, "ZURN", "2026-05-01T00:00:00Z", "active"),
            sm(6, "SREN", "2026-06-01T00:00:00Z", "active"),
        ];
        let mut returns = HashMap::new();
        returns.insert(Uuid::from_u128(1), ud(Some("9.8"), "9,8:1"));
        returns.insert(Uuid::from_u128(2), ud(Some("10.5"), "10,5:1"));
        returns.insert(Uuid::from_u128(3), ud(None, "—"));
        returns.insert(Uuid::from_u128(4), ud(Some("3.05"), "3,05:1"));
        returns.insert(Uuid::from_u128(5), ud(Some("3.04"), "3,04:1"));
        // id 6 (SREN) deliberately absent → unknown ratio.

        let desc = curate(
            &studies,
            "",
            SortKey::UpsideDownside,
            true,
            StatusFilter::All,
            &returns,
            DayZone::UTC,
        );
        assert_eq!(
            tickers(&desc),
            vec!["ROG", "NESN", "UBSG", "ZURN", "SREN", "ABBN"],
            "descending: highest first; the dashes last, their `id` tiebreak flipped like the rest"
        );
        assert_eq!(desc[0].ud_ratio.to_string(), "10,5:1");
        assert_eq!(desc[4].ud_ratio.to_string(), "—");
        assert_eq!(desc[5].ud_ratio.to_string(), "—");

        let asc = curate(
            &studies,
            "",
            SortKey::UpsideDownside,
            false,
            StatusFilter::All,
            &returns,
            DayZone::UTC,
        );
        assert_eq!(
            tickers(&asc),
            vec!["ZURN", "UBSG", "NESN", "ROG", "ABBN", "SREN"],
            "ascending: lowest known first, the dashes STILL last"
        );
    }

    #[test]
    fn sort_by_ud_ties_break_on_id_like_the_other_sorts() {
        // Equal ratios (« 2:1 » == « 2,0:1 » as decimals) and two dashes: `id` decides, flipped with
        // the direction for the known ratios; the dashes stay last, in `id` order flipped likewise.
        let studies = vec![
            sm(4, "DDD", "2026-01-01T00:00:00Z", "active"),
            sm(3, "CCC", "2026-01-01T00:00:00Z", "active"),
            sm(2, "BBB", "2026-01-01T00:00:00Z", "active"),
            sm(1, "AAA", "2026-01-01T00:00:00Z", "active"),
        ];
        let mut returns = HashMap::new();
        returns.insert(Uuid::from_u128(1), ud(Some("2"), "2:1"));
        returns.insert(Uuid::from_u128(2), ud(Some("2.0"), "2,0:1"));
        returns.insert(Uuid::from_u128(3), ud(None, "—"));
        returns.insert(Uuid::from_u128(4), ud(None, "—"));
        let asc = curate(
            &studies,
            "",
            SortKey::UpsideDownside,
            false,
            StatusFilter::All,
            &returns,
            DayZone::UTC,
        );
        assert_eq!(tickers(&asc), vec!["AAA", "BBB", "CCC", "DDD"]);
        let desc = curate(
            &studies,
            "",
            SortKey::UpsideDownside,
            true,
            StatusFilter::All,
            &returns,
            DayZone::UTC,
        );
        assert_eq!(tickers(&desc), vec!["BBB", "AAA", "DDD", "CCC"]);
    }

    #[test]
    fn incomplete_flag_rides_onto_the_row() {
        // Issue #148: the per-study `incomplete` fact threads through curation onto the row (and a
        // study absent from the map defaults to NOT incomplete — "unknown" never shouts "à compléter").
        let studies = vec![
            sm(1, "NESN", "2026-01-10T00:00:00Z", "active"),
            sm(2, "ROG", "2026-03-02T00:00:00Z", "active"),
        ];
        let mut returns = HashMap::new();
        returns.insert(
            Uuid::from_u128(1),
            StudyReturn {
                value: None,
                display: "—".to_string(),
                ud: "—".to_string(),
                ud_value: None,
                incomplete: true,
                zone: "",
                company_name: String::new(),
                ud_mood: None,
                provisional: false,
            },
        );
        // id 2 (ROG) deliberately absent → defaults to not incomplete.
        let rows = curate(
            &studies,
            "",
            SortKey::Ticker,
            false,
            StatusFilter::All,
            &returns,
            DayZone::UTC,
        );
        assert_eq!(tickers(&rows), vec!["NESN", "ROG"]);
        assert!(rows[0].incomplete, "NESN is flagged à compléter");
        // The U/D ratio rides onto the row too; an absent study reads « — », never blank.
        assert_eq!(rows[0].ud_ratio.to_string(), "—");
        assert_eq!(rows[1].ud_ratio.to_string(), "—");
        assert!(
            !rows[1].incomplete,
            "ROG (absent from the map) defaults to not incomplete"
        );
    }

    #[test]
    fn ud_threshold_reads_a_decimal_or_a_whole_fraction_strictly_positive() {
        let dec = |s: &str| Decimal::from_str_exact(s).unwrap();
        assert_eq!(parse_ud_threshold("3"), Some(dec("3")));
        assert_eq!(parse_ud_threshold("0.33"), Some(dec("0.33")));
        assert_eq!(
            parse_ud_threshold("1/3"),
            Decimal::ONE.checked_div(Decimal::from(3u8))
        );
        assert_eq!(parse_ud_threshold(" 2 / 4 "), Some(dec("0.5")));
        for refused in [
            "", "0", "-1", "0/3", "1/0", "1/", "/3", "1,5/3", "1.5/3", "a", "1/3/4", "-1/3",
        ] {
            assert_eq!(
                parse_ud_threshold(refused),
                None,
                "{refused:?} must be refused"
            );
        }
        let defaults = UdThresholds::default();
        assert_eq!(defaults.high, dec("3"));
        assert!(defaults.low < defaults.high);
    }

    #[test]
    fn ud_mood_classifies_on_exact_edges() {
        let dec = |s: &str| Decimal::from_str_exact(s).unwrap();
        let t = UdThresholds::default();
        let mood = |r: &str| ud_mood(Some(UdRank::Ratio(dec(r))), t);
        // ≥ high → 🙂, exactly 3 included.
        assert_eq!(mood("3"), Some(UdMood::High));
        assert_eq!(mood("3.0000"), Some(UdMood::High));
        assert_eq!(mood("10.5"), Some(UdMood::High));
        assert_eq!(mood("2.9999999999"), Some(UdMood::Middle));
        assert_eq!(mood("1"), Some(UdMood::Middle));
        // < low → 🙁; exactly one third is NOT below one third → 😐.
        assert_eq!(mood("0.33"), Some(UdMood::Low));
        assert_eq!(mood("0.3333333333"), Some(UdMood::Low));
        assert_eq!(mood("0.34"), Some(UdMood::Middle));
        // A ratio of exactly one third, built the way the core builds it — `(high − price) /
        // (price − low)` with `checked_div` — lands ON the « 1/3 » threshold, whatever the terms.
        for (up, down) in [
            ("1", "3"),
            ("2", "6"),
            ("10.5", "31.5"),
            ("33.3", "99.9"),
            ("7", "21"),
        ] {
            let r = dec(up).checked_div(dec(down)).unwrap();
            assert_eq!(
                ud_mood(Some(UdRank::Ratio(r)), t),
                Some(UdMood::Middle),
                "{up}/{down} is exactly one third → 😐"
            );
        }
        // Undefined (price ≤ forecast low, « ∞ ») → 🙂; no value (« — ») → no emoticon.
        assert_eq!(ud_mood(Some(UdRank::Unbounded), t), Some(UdMood::Unbounded));
        assert_eq!(ud_mood(None, t), None);
        // A user pair: « 0,5 » and « 2 ».
        let custom = UdThresholds {
            low: dec("0.5"),
            high: dec("2"),
        };
        assert_eq!(
            ud_mood(Some(UdRank::Ratio(dec("2"))), custom),
            Some(UdMood::High)
        );
        assert_eq!(
            ud_mood(Some(UdRank::Ratio(dec("0.5"))), custom),
            Some(UdMood::Middle)
        );
        assert_eq!(
            ud_mood(Some(UdRank::Ratio(dec("0.49"))), custom),
            Some(UdMood::Low)
        );
    }

    #[test]
    fn ud_mood_follows_the_ud_column_withheld_unknown_and_undefined() {
        // The emoticon is classified off the U/D column's own facts: a withheld verdict and an
        // unknown ratio read « — » there and show no emoticon; an undefined ratio reads « ∞ » → 🙂.
        let format = crate::viewmodel::format::NumberFormat::default();
        let t = UdThresholds::default();
        let ratio = UpsideDownside::Ratio(Decimal::from(5u8));
        let mood = |ud: &UpsideDownside, incomplete| ud_mood(ud_facts(ud, incomplete, format).1, t);
        assert_eq!(mood(&ratio, false), Some(UdMood::High));
        assert_eq!(
            mood(&UpsideDownside::Undefined, false),
            Some(UdMood::Unbounded)
        );
        assert_eq!(mood(&UpsideDownside::Unknown, false), None);
        for any in [ratio, UpsideDownside::Undefined, UpsideDownside::Unknown] {
            assert_eq!(mood(&any, true), None, "withheld → no emoticon");
        }
    }

    #[test]
    fn the_row_carries_the_emoticon_key_and_greys_it_only_when_provisional() {
        let study = sm(1, "NESN", "2026-01-10T00:00:00Z", "active");
        let row = |ud_mood, provisional| {
            to_row(
                &study,
                &StudyReturn {
                    ud_mood,
                    provisional,
                    ..StudyReturn::default()
                },
                DayZone::UTC,
            )
        };
        let r = row(Some(UdMood::High), false);
        assert_eq!((r.ud_mood.as_str(), r.ud_mood_greyed), ("high", false));
        let r = row(Some(UdMood::Unbounded), true);
        assert_eq!((r.ud_mood.as_str(), r.ud_mood_greyed), ("unbounded", true));
        let r = row(Some(UdMood::Middle), true);
        assert_eq!((r.ud_mood.as_str(), r.ud_mood_greyed), ("middle", true));
        let r = row(Some(UdMood::Low), false);
        assert_eq!((r.ud_mood.as_str(), r.ud_mood_greyed), ("low", false));
        // No emoticon: nothing to grey either.
        let r = row(None, true);
        assert_eq!((r.ud_mood.as_str(), r.ud_mood_greyed), ("", false));
    }

    #[test]
    fn from_wire_defaults_are_safe() {
        assert_eq!(StatusFilter::from_wire("nonsense"), StatusFilter::Active);
        assert_eq!(StatusFilter::from_wire("all"), StatusFilter::All);
        assert_eq!(SortKey::from_wire("nonsense"), SortKey::Date);
        assert_eq!(SortKey::from_wire("ticker"), SortKey::Ticker);
        assert_eq!(SortKey::from_wire("potential"), SortKey::PotentialReturn);
        assert_eq!(SortKey::from_wire("ud"), SortKey::UpsideDownside);
    }
}
