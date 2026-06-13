//! Study view-model adapter (Story 2.2): map `persistence::StudySummary` and `contract::Study` into
//! the Slint `StudyRow` struct and a minimal restore-view string. Presentation only — nothing here
//! calculates (Cardinal Rule), and money/decimals surface as **formatted strings** via the 2.1
//! `format` helper, never as floats.

use steadyinvest_contract::Study;
use steadyinvest_persistence::StudySummary;

use crate::state::created_at_date;
use crate::viewmodel::format::NumberFormat;
use crate::StudyRow;

// ── Static user-facing labels for the restore view (French). Scanned by the crate-local posture
//    gate (FR13) via `USER_FACING_MESSAGES`. Plain nouns/participles — no imperative verbs. ──

const DETAIL_CURRENCY: &str = "Devise";
const DETAIL_CREATED: &str = "Créée le";
const DETAIL_YEARS: &str = "Années renseignées";
const DETAIL_RATIONALE_PRESENT: &str = "Note de décision : présente";
const DETAIL_RATIONALE_ABSENT: &str = "Note de décision : aucune";
const DETAIL_JUDGMENT_INPUTS: &str = "Éléments de jugement renseignés";

/// The static restore-view labels — exposed so the posture gate scans them alongside the `@tr()`
/// literals and `state::USER_FACING_MESSAGES`. Test-only (the gate's sole consumer).
#[cfg(test)]
pub const USER_FACING_MESSAGES: &[&str] = &[
    DETAIL_CURRENCY,
    DETAIL_CREATED,
    DETAIL_YEARS,
    DETAIL_RATIONALE_PRESENT,
    DETAIL_RATIONALE_ABSENT,
    DETAIL_JUDGMENT_INPUTS,
];

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

/// Count the populated `Option<Money>` judgment inputs (the issue-#14 fields included) — a compact,
/// float-free way for the restore view to show that judgment state survives a reopen.
fn populated_judgment_inputs(study: &Study) -> usize {
    let j = &study.judgment;
    [
        j.estimated_high_eps,
        j.estimated_low_eps,
        j.projected_sales_growth_pct,
        j.projected_eps_growth_pct,
        j.judged_avg_high_pe,
        j.judged_avg_low_pe,
        j.recent_severe_low,
        j.current_price,
        j.present_full_year_dividend,
    ]
    .iter()
    .filter(|v| v.is_some())
    .count()
}

/// Build the reopened-study restore view: a title (`TICKER · CUR`) and a multi-line body proving
/// the full state came back (currency, created-at, year count, judgment-input count, rationale
/// presence). Minimal by design — the faithful SSG form is Story 2.3.
pub fn detail(study: &Study, _format: NumberFormat) -> (String, String) {
    let title = format!("{} · {}", study.security_ticker, study.native_currency);
    let body = format!(
        "{DETAIL_CURRENCY} : {}\n{DETAIL_CREATED} : {}\n{DETAIL_YEARS} : {}\n\
         {DETAIL_JUDGMENT_INPUTS} : {}\n{}",
        study.native_currency,
        created_at_date(&study.created_at),
        study.years.len(),
        populated_judgment_inputs(study),
        if study.rationale.is_some() {
            DETAIL_RATIONALE_PRESENT
        } else {
            DETAIL_RATIONALE_ABSENT
        },
    );
    (title, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::{ForecastLowOption, Judgment, Money, Timestamp};
    use uuid::Uuid;

    fn money(s: &str) -> Money {
        serde_json::from_str(&format!("\"{s}\"")).unwrap()
    }

    fn study_with(years: usize, rationale: Option<&str>, judgment: Judgment) -> Study {
        let mut s = Study::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "NESN",
            "CHF",
            judgment,
            Timestamp("2026-06-13T09:30:00Z".to_string()),
        );
        s.rationale = rationale.map(str::to_string);
        // Years content is irrelevant to the count; push empty markers is not possible (YearData
        // is rich), so this test drives the year count via a direct push of a minimal year only
        // when needed. Here we keep `years` empty and assert on 0 unless caller wants otherwise.
        let _ = years;
        s
    }

    fn empty_judgment() -> Judgment {
        Judgment {
            estimated_high_eps: None,
            estimated_low_eps: None,
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: None,
            forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
            recent_severe_low: None,
            current_price: None,
            present_full_year_dividend: None,
        }
    }

    #[test]
    fn detail_title_is_ticker_and_currency() {
        let s = study_with(0, None, empty_judgment());
        let (title, body) = detail(&s, NumberFormat::Comma);
        assert_eq!(title, "NESN · CHF");
        assert!(body.contains("2026-06-13"), "created date shown: {body}");
        assert!(body.contains(DETAIL_RATIONALE_ABSENT));
    }

    #[test]
    fn populated_judgment_inputs_counts_all_optionals_including_issue_14_fields() {
        let mut j = empty_judgment();
        assert_eq!(
            populated_judgment_inputs(&study_with(0, None, j.clone())),
            0
        );
        j.projected_sales_growth_pct = Some(money("8.5"));
        j.recent_severe_low = Some(money("72.40"));
        j.present_full_year_dividend = Some(money("2.95"));
        assert_eq!(
            populated_judgment_inputs(&study_with(0, None, j)),
            3,
            "the issue-#14 fields are counted in the restore view"
        );
    }
}
