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
