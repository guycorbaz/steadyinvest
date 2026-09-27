//! Study notes card (Story 8.1, UX 8.0 §5.6): the rows the « Notes » card shows, newest first.
//!
//! Pure view logic over the already-loaded [`Study`]: no IO. The note texts are the owner's own
//! words and cross as data; the only app strings are the meta line's date wording (UX §3.3).

use steadyinvest_contract::{Study, Timestamp};
use uuid::Uuid;

/// « · modifiée le » — the meta suffix of an edited note (UX 8.0 §3.3 « Notes »).
pub const NOTE_EDITED_ON: &str = "modifiée le";

/// Every app string of the notes card built in Rust, scanned by the posture gate (FR13).
#[cfg(test)]
pub const NOTES_USER_FACING_LABELS: &[&str] = &[NOTE_EDITED_ON];

/// One row of the « Notes » card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteRowView {
    pub id: Uuid,
    /// « JJ/MM/AAAA », plus « · modifiée le JJ/MM/AAAA » once the text was edited.
    pub meta: String,
    pub text: String,
}

/// `JJ/MM/AAAA` of an RFC3339 UTC stamp in the machine's LOCAL time zone (a note written at
/// 00:30 in Zurich reads that day, not the day before); the raw stamp when malformed — display
/// only, never a hard error. The history's day headers stay UTC (app-wide, unchanged).
pub fn date_fr(stamp: &Timestamp) -> String {
    date_fr_in(stamp, &chrono::Local)
}

/// [`date_fr`] in a given zone — the pure core, tested with fixed offsets.
fn date_fr_in<Tz: chrono::TimeZone>(stamp: &Timestamp, zone: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    match chrono::DateTime::parse_from_rfc3339(&stamp.0) {
        Ok(at) => at.with_timezone(zone).format("%d/%m/%Y").to_string(),
        Err(_) => stamp.0.clone(),
    }
}

/// The card's rows, newest first (by `created_at`, then id — a stable order for equal stamps);
/// the stored order stays insertion order.
pub fn note_rows(study: &Study) -> Vec<NoteRowView> {
    let mut notes: Vec<_> = study.notes.iter().collect();
    notes.sort_by(|a, b| b.created_at.0.cmp(&a.created_at.0).then(b.id.cmp(&a.id)));
    notes
        .into_iter()
        .map(|n| {
            let created = date_fr(&n.created_at);
            let meta = if n.updated_at != n.created_at {
                format!("{created} · {NOTE_EDITED_ON} {}", date_fr(&n.updated_at))
            } else {
                created
            };
            NoteRowView {
                id: n.id,
                meta,
                text: n.text.clone(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::{ForecastLowOption, Judgment, Note};

    fn study_with(notes: Vec<Note>) -> Study {
        let mut s = Study::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "NESN",
            "CHF",
            Judgment {
                ai_placed: Default::default(),
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
                ttm_eps: None,
            },
            Timestamp("2026-09-01T00:00:00Z".to_string()),
        );
        s.notes = notes;
        s
    }

    fn note(id: u128, created: &str, updated: &str) -> Note {
        Note {
            id: Uuid::from_u128(id),
            text: format!("note {id}"),
            created_at: Timestamp(created.to_string()),
            updated_at: Timestamp(updated.to_string()),
            ai_origin: None,
        }
    }

    #[test]
    fn rows_are_newest_first_with_the_edit_date_when_edited() {
        // Midday stamps: the same calendar day in any real time zone.
        let rows = note_rows(&study_with(vec![
            note(1, "2026-09-20T12:00:00Z", "2026-09-20T12:00:00Z"),
            note(2, "2026-09-27T12:00:00Z", "2026-09-28T12:00:00Z"),
        ]));
        assert_eq!(rows[0].id, Uuid::from_u128(2));
        assert_eq!(rows[0].meta, "27/09/2026 · modifiée le 28/09/2026");
        assert_eq!(rows[1].meta, "20/09/2026");
        assert_eq!(rows[1].text, "note 1");
    }

    #[test]
    fn a_note_date_is_the_local_calendar_day() {
        let late = Timestamp("2026-09-26T22:30:00Z".to_string());
        let zurich = chrono::FixedOffset::east_opt(2 * 3600).unwrap();
        let new_york = chrono::FixedOffset::west_opt(4 * 3600).unwrap();
        assert_eq!(
            date_fr_in(&late, &zurich),
            "27/09/2026",
            "00:30 in Zurich is the 27th"
        );
        assert_eq!(date_fr_in(&late, &new_york), "26/09/2026");
    }

    #[test]
    fn a_malformed_stamp_shows_raw_never_panics() {
        assert_eq!(date_fr(&Timestamp("x".to_string())), "x");
    }
}
