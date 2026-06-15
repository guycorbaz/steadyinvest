//! Study list view-model adapter (Story 2.2): map `persistence::StudySummary` into the Slint
//! `StudyRow` struct for the dashboard list. Presentation only — nothing here calculates (Cardinal
//! Rule). The reopened-study **form** adapter (header + §3 P/E rows) lives in [`crate::viewmodel::form`]
//! (Story 2.3); the 2.2 minimal restore view it replaced — and its `detail()` string builder — are
//! gone now that the faithful §1–§5 form renders the open study.

use steadyinvest_persistence::StudySummary;

use crate::state::created_at_date;
use crate::StudyRow;

/// Map one summary row into the Slint `StudyRow` (id stringified, date trimmed to the day, status
/// verbatim).
pub fn to_row(summary: &StudySummary) -> StudyRow {
    StudyRow {
        id: summary.id.to_string().into(),
        ticker: summary.security_ticker.clone().into(),
        created_at: created_at_date(&summary.created_at).into(),
        status: summary.status.clone().into(),
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
}

impl SortKey {
    /// Map the Slint wire string; anything unrecognized falls back to `Date`.
    pub fn from_wire(s: &str) -> Self {
        match s {
            "ticker" => Self::Ticker,
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
) -> Vec<StudyRow> {
    let needle = query.trim().to_lowercase();
    let mut kept: Vec<&StudySummary> = summaries
        .iter()
        .filter(|s| status_filter.admits(&s.status))
        .filter(|s| needle.is_empty() || s.security_ticker.to_lowercase().contains(&needle))
        .collect();
    kept.sort_by(|a, b| {
        let ord = match sort_key {
            SortKey::Date => a.created_at.0.cmp(&b.created_at.0).then(a.id.cmp(&b.id)),
            SortKey::Ticker => a
                .security_ticker
                .to_lowercase()
                .cmp(&b.security_ticker.to_lowercase())
                .then(a.id.cmp(&b.id)),
        };
        if descending {
            ord.reverse()
        } else {
            ord
        }
    });
    kept.iter().map(|s| to_row(s)).collect()
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

    #[test]
    fn status_filter_active_hides_archived() {
        let rows = curate(&sample(), "", SortKey::Date, false, StatusFilter::Active);
        assert_eq!(
            tickers(&rows),
            vec!["NESN", "ROG"],
            "archived ABBN is hidden"
        );
    }

    #[test]
    fn status_filter_archived_and_all() {
        let archived = curate(&sample(), "", SortKey::Date, false, StatusFilter::Archived);
        assert_eq!(tickers(&archived), vec!["ABBN"]);
        let all = curate(&sample(), "", SortKey::Date, false, StatusFilter::All);
        assert_eq!(all.len(), 3, "all shows active + archived");
    }

    #[test]
    fn search_is_case_insensitive_ticker_substring() {
        let rows = curate(&sample(), "bb", SortKey::Date, false, StatusFilter::All);
        assert_eq!(
            tickers(&rows),
            vec!["ABBN"],
            "substring match, case-insensitive"
        );
        let none = curate(&sample(), "ZZZ", SortKey::Date, false, StatusFilter::All);
        assert!(none.is_empty(), "no match → empty");
    }

    #[test]
    fn sort_by_date_and_ticker_both_directions() {
        let by_date = curate(&sample(), "", SortKey::Date, false, StatusFilter::All);
        assert_eq!(
            tickers(&by_date),
            vec!["NESN", "ABBN", "ROG"],
            "date ascending"
        );
        let by_date_desc = curate(&sample(), "", SortKey::Date, true, StatusFilter::All);
        assert_eq!(
            tickers(&by_date_desc),
            vec!["ROG", "ABBN", "NESN"],
            "date descending"
        );
        let by_ticker = curate(&sample(), "", SortKey::Ticker, false, StatusFilter::All);
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
        let rows = curate(&same, "", SortKey::Date, false, StatusFilter::All);
        let ids: Vec<String> = rows.iter().map(|r| r.id.to_string()).collect();
        assert_eq!(
            ids[0],
            Uuid::from_u128(1).to_string(),
            "lower id first (deterministic)"
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
