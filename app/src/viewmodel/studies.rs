//! Study list view-model adapter (Story 2.2): map `persistence::StudySummary` into the Slint
//! `StudyRow` struct for the dashboard list. Presentation only — nothing here calculates (Cardinal
//! Rule). The reopened-study **form** adapter (header + §3 P/E rows) lives in [`crate::viewmodel::form`]
//! (Story 2.3); the 2.2 minimal restore view it replaced — and its `detail()` string builder — are
//! gone now that the faithful §1–§5 form renders the open study.

use std::collections::HashMap;

use rust_decimal::Decimal;
use steadyinvest_persistence::StudySummary;
use uuid::Uuid;

use crate::StudyRow;
use crate::state::created_at_date;

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
    /// 2026-07-12 — the study's user-entered company name (empty when unset), shown after the
    /// ticker on the list row. Owned (read off the full study during refresh, not the summary).
    pub company_name: String,
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
            incomplete: false,
            zone: "",
            company_name: String::new(),
        }
    }
}

/// Map one summary row into the Slint `StudyRow` (id stringified, date trimmed to the day, status
/// verbatim, the pre-formatted potential-return string — "—" when the study withholds it — the
/// issue #148 `incomplete` flag driving the "à compléter" marker, the present-price zone key, and
/// the company name shown after the ticker).
pub fn to_row(
    summary: &StudySummary,
    potential_return: &str,
    incomplete: bool,
    zone: &str,
    company_name: &str,
) -> StudyRow {
    StudyRow {
        id: summary.id.to_string().into(),
        ticker: summary.security_ticker.clone().into(),
        created_at: created_at_date(&summary.created_at).into(),
        status: summary.status.clone().into(),
        potential_return: potential_return.into(),
        incomplete,
        zone: zone.into(),
        company_name: company_name.into(),
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

/// The dashboard sort key (Story 2.12, FR54): created-date or ticker. `id` is always the
/// deterministic tiebreaker so the list never jitters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Date,
    Ticker,
    /// Issue #107: the estimated potential (§5 projected total annualized return). Studies whose
    /// potential is withheld sort LAST in both directions.
    PotentialReturn,
}

impl SortKey {
    /// Map the Slint wire string; anything unrecognized falls back to `Date`.
    pub fn from_wire(s: &str) -> Self {
        match s {
            "ticker" => Self::Ticker,
            "potential" => Self::PotentialReturn,
            _ => Self::Date,
        }
    }
}

/// Pure dashboard curation (Story 2.12, FR54): filter by lifecycle status + case-insensitive ticker
/// substring, then **stable**-sort by date or ticker (asc/desc) with `id` as the deterministic
/// tiebreaker. No I/O, no calculation (Cardinal Rule) — the testable heart of the dashboard. The
/// caller passes the persistence `created_at, id`-ordered summaries; tickers/search text are user
/// data (never posture-scanned).
pub fn curate(
    summaries: &[StudySummary],
    query: &str,
    sort_key: SortKey,
    descending: bool,
    status_filter: StatusFilter,
    returns: &HashMap<Uuid, StudyReturn>,
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
        // PotentialReturn study with NO computed value sorts LAST in both directions (an early return,
        // bypassing the flip) — an unknown potential is never a top pick.
        let ord = match sort_key {
            SortKey::Date => a.created_at.0.cmp(&b.created_at.0),
            SortKey::Ticker => a
                .security_ticker
                .to_lowercase()
                .cmp(&b.security_ticker.to_lowercase()),
            SortKey::PotentialReturn => {
                let va = returns.get(&a.id).and_then(|r| r.value);
                let vb = returns.get(&b.id).and_then(|r| r.value);
                match (va, vb) {
                    (Some(x), Some(y)) => x.cmp(&y),
                    (Some(_), None) => return Ordering::Less, // known before unknown, always
                    (None, Some(_)) => return Ordering::Greater,
                    (None, None) => Ordering::Equal,
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
            to_row(
                s,
                facts.display.as_str(),
                facts.incomplete,
                facts.zone,
                facts.company_name.as_str(),
            )
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
            incomplete: false,
            zone: "",
            company_name: String::new(),
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
        );
        assert_eq!(tickers(&archived), vec!["ABBN"]);
        let all = curate(
            &sample(),
            "",
            SortKey::Date,
            false,
            StatusFilter::All,
            &no_returns(),
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
        );
        assert_eq!(tickers(&asc), vec!["ROG", "NESN", "ABBN"]);
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
                incomplete: true,
                zone: "",
                company_name: String::new(),
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
        );
        assert_eq!(tickers(&rows), vec!["NESN", "ROG"]);
        assert!(rows[0].incomplete, "NESN is flagged à compléter");
        assert!(
            !rows[1].incomplete,
            "ROG (absent from the map) defaults to not incomplete"
        );
    }

    #[test]
    fn from_wire_defaults_are_safe() {
        assert_eq!(StatusFilter::from_wire("nonsense"), StatusFilter::Active);
        assert_eq!(StatusFilter::from_wire("all"), StatusFilter::All);
        assert_eq!(SortKey::from_wire("nonsense"), SortKey::Date);
        assert_eq!(SortKey::from_wire("ticker"), SortKey::Ticker);
    }
}
