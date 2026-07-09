//! FR51 durable-history timeline (issue #34, PR 2) — turn the `judgments` snapshot series into
//! the « Historique » panel's rows: per-entry a neutral SUMMARY (what changed, compactly) and,
//! on demand, the DETAIL lines (`label : avant → après`).
//!
//! Pure view logic over already-loaded [`Study`] states: no IO, no decision math (Cardinal Rule).
//! The diff is computed between **consecutive** snapshots — nothing is stored twice (the payloads
//! are the single source of truth; a diff format on disk would be a second one). The vocabulary
//! lives in Rust (the composed lines cross to Slint as data, the `verdict_trace` precedent) and is
//! inventoried in [`HISTORY_USER_FACING_LABELS`] for the posture gate (FR13).

use steadyinvest_contract::{Judgment, Money, Study, YearData};
use steadyinvest_core::rounding::DisplayField;
use uuid::Uuid;

use crate::viewmodel::engine::{
    LBL_CURRENT_PRICE, LBL_EPS, LBL_EST_HIGH_EPS, LBL_EST_LOW_EPS, LBL_HIGH_PE, LBL_HIGH_PRICE,
    LBL_LOW_PE, LBL_LOW_PRICE, LBL_SALES,
};
use crate::viewmodel::entry;
use crate::viewmodel::format::{NumberFormat, format_amount, format_scaled};

// ── Vocabulary (posture-inventoried; the study-screen spellings where one exists) ──

pub const HIST_CREATED: &str = "Étude créée";
pub const HIST_STATUS_CHANGED: &str = "statut modifié";
pub const HIST_JUDGMENT: &str = "jugement";
pub const HIST_RATIONALE_CHANGED: &str = "raison consignée modifiée";
pub const HIST_YEAR_ADDED: &str = "année ajoutée";
pub const HIST_OTHER: &str = "autres champs modifiés";
pub const HIST_CELLS_CHANGED: &str = "cellule(s) modifiée(s)";
/// The empty display slot — the same faithful em-dash the form uses for an absent figure.
pub const HIST_EMPTY_SLOT: &str = "—";
pub const LBL_DIVIDEND_PS: &str = "Dividende par action";
pub const LBL_PRETAX_PROFIT: &str = "Bénéfice avant impôt";
pub const LBL_BOOK_VALUE: &str = "Valeur comptable par action";
pub const LBL_SALES_GROWTH: &str = "Croissance projetée des ventes";
pub const LBL_EPS_GROWTH: &str = "Croissance projetée du BPA";
pub const LBL_FORECAST_LOW_OPTION: &str = "Prix bas prévu (sélection)";
pub const LBL_SEVERE_LOW: &str = "Plus bas sévère récent";
pub const LBL_DIVIDEND_YEAR: &str = "Dividende annuel courant";
pub const LBL_TTM_EPS: &str = "BPA 12 derniers mois";

/// Every history label, exposed so the crate-local posture gate (FR13) scans them for banned
/// verbs alongside the `@tr()` literals — the `engine::USER_FACING_LABELS` precedent.
#[cfg(test)]
pub const HISTORY_USER_FACING_LABELS: &[&str] = &[
    HIST_CREATED,
    HIST_STATUS_CHANGED,
    HIST_JUDGMENT,
    HIST_RATIONALE_CHANGED,
    HIST_YEAR_ADDED,
    HIST_OTHER,
    HIST_CELLS_CHANGED,
    HIST_EMPTY_SLOT,
    LBL_DIVIDEND_PS,
    LBL_PRETAX_PROFIT,
    LBL_BOOK_VALUE,
    LBL_SALES_GROWTH,
    LBL_EPS_GROWTH,
    LBL_FORECAST_LOW_OPTION,
    LBL_SEVERE_LOW,
    LBL_DIVIDEND_YEAR,
    LBL_TTM_EPS,
];

/// One timeline entry (newest first): the snapshot's identity + day/time + the neutral summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntryView {
    pub id: Uuid,
    /// `AAAA-MM-JJ` — the day-group key.
    pub day: String,
    /// `true` on the first entry of each day going down the (newest-first) list — the UI renders
    /// the day header there.
    pub first_of_day: bool,
    /// `HH:MM` (empty when the stamp is malformed — display only, never a hard error).
    pub time: String,
    pub summary: String,
}

/// The diff facets between two consecutive states — the shared source of both the summary and the
/// detail lines (one derivation, two renderings).
struct Diff {
    /// `(label année, detail line)` per changed cell.
    cells: Vec<(String, String)>,
    /// `(label, detail line)` per changed judgment input.
    judgment: Vec<(String, String)>,
    rationale_changed: bool,
    years_added: Vec<i32>,
    /// A change none of the named facets caught (defensive — the dedup guarantees the states
    /// differ, so an empty diff must still say *something* honest).
    other: bool,
}

/// The canonical + optional cell fields, with their display labels — the diff walks these.
const CELL_FIELDS: [(&str, &str); 7] = [
    (entry::FIELD_SALES, LBL_SALES),
    (entry::FIELD_EPS, LBL_EPS),
    (entry::FIELD_HIGH, LBL_HIGH_PRICE),
    (entry::FIELD_LOW, LBL_LOW_PRICE),
    (entry::FIELD_DIVIDEND, LBL_DIVIDEND_PS),
    (entry::FIELD_PRETAX, LBL_PRETAX_PROFIT),
    (entry::FIELD_BOOK, LBL_BOOK_VALUE),
];

/// A cell value's display spelling: the grid's own path (millions scaling + locale grouping);
/// the faithful em-dash for an absent figure.
fn cell_value_display(value: Option<Money>, field: &str, format: NumberFormat) -> String {
    match value {
        Some(money) => format_amount(&entry::stored_to_display(money, field).to_string(), format),
        None => HIST_EMPTY_SLOT.to_string(),
    }
}

/// A judgment value's display spelling (the `judgment_fields` scales), em-dash when unset.
fn judgment_value_display(
    value: Option<Money>,
    display: DisplayField,
    format: NumberFormat,
) -> String {
    match value {
        Some(money) => format_scaled(money.as_decimal(), display, format),
        None => HIST_EMPTY_SLOT.to_string(),
    }
}

fn diff_judgment(prev: &Judgment, next: &Judgment, format: NumberFormat) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut field =
        |label: &str, before: Option<Money>, after: Option<Money>, display: DisplayField| {
            if before != after {
                out.push((
                    label.to_string(),
                    format!(
                        "{label} : {} → {}",
                        judgment_value_display(before, display, format),
                        judgment_value_display(after, display, format)
                    ),
                ));
            }
        };
    field(
        LBL_SALES_GROWTH,
        prev.projected_sales_growth_pct,
        next.projected_sales_growth_pct,
        DisplayField::Percent,
    );
    field(
        LBL_EPS_GROWTH,
        prev.projected_eps_growth_pct,
        next.projected_eps_growth_pct,
        DisplayField::Percent,
    );
    field(
        LBL_EST_HIGH_EPS,
        prev.estimated_high_eps,
        next.estimated_high_eps,
        DisplayField::PerShare,
    );
    field(
        LBL_EST_LOW_EPS,
        prev.estimated_low_eps,
        next.estimated_low_eps,
        DisplayField::PerShare,
    );
    field(
        LBL_HIGH_PE,
        prev.judged_avg_high_pe,
        next.judged_avg_high_pe,
        DisplayField::PeRatio,
    );
    field(
        LBL_LOW_PE,
        prev.judged_avg_low_pe,
        next.judged_avg_low_pe,
        DisplayField::PeRatio,
    );
    field(
        LBL_SEVERE_LOW,
        prev.recent_severe_low,
        next.recent_severe_low,
        DisplayField::Price,
    );
    field(
        LBL_CURRENT_PRICE,
        prev.current_price,
        next.current_price,
        DisplayField::Price,
    );
    field(
        LBL_DIVIDEND_YEAR,
        prev.present_full_year_dividend,
        next.present_full_year_dividend,
        DisplayField::PerShare,
    );
    field(
        LBL_TTM_EPS,
        prev.ttm_eps,
        next.ttm_eps,
        DisplayField::PerShare,
    );
    if prev.forecast_low_option != next.forecast_low_option {
        // The option crosses as its stable key (data, translated where the §4 picker already
        // does) — the transition itself is the fact stated here.
        out.push((
            LBL_FORECAST_LOW_OPTION.to_string(),
            format!(
                "{LBL_FORECAST_LOW_OPTION} : {} → {}",
                crate::viewmodel::engine::forecast_low_option_key(prev.forecast_low_option),
                crate::viewmodel::engine::forecast_low_option_key(next.forecast_low_option),
            ),
        ));
    }
    out
}

fn diff_states(prev: &Study, next: &Study, format: NumberFormat) -> Diff {
    let mut cells = Vec::new();
    let mut years_added = Vec::new();
    let by_year = |years: &[YearData], y: i32| -> Option<YearData> {
        years.iter().find(|yd| yd.year == y).cloned()
    };
    for year in &next.years {
        let Some(prev_year) = by_year(&prev.years, year.year) else {
            years_added.push(year.year);
            continue;
        };
        for (field, label) in CELL_FIELDS {
            let before = entry::get_cell(&prev_year, field);
            let after = entry::get_cell(year, field);
            if before == after {
                continue;
            }
            let name = format!("{label} {}", year.year);
            let before_value = before.as_ref().and_then(|c| c.value);
            let after_value = after.as_ref().and_then(|c| c.value);
            let line = if before_value != after_value {
                format!(
                    "{name} : {} → {}",
                    cell_value_display(before_value, field, format),
                    cell_value_display(after_value, field, format)
                )
            } else {
                // Same figure, different cell state (review tag, source, pending divergence,
                // provenance) — stated as a status change, not a phantom value change.
                format!("{name} : {HIST_STATUS_CHANGED}")
            };
            cells.push((name, line));
        }
    }
    let judgment = diff_judgment(&prev.judgment, &next.judgment, format);
    let rationale_changed = prev.rationale != next.rationale;
    let other = cells.is_empty()
        && judgment.is_empty()
        && !rationale_changed
        && years_added.is_empty()
        && prev != next;
    Diff {
        cells,
        judgment,
        rationale_changed,
        years_added,
        other,
    }
}

/// Compact enumeration for a summary clause: up to three names, then an honest ellipsis.
fn named(names: &[String]) -> String {
    let mut shown: Vec<&str> = names.iter().take(3).map(String::as_str).collect();
    let text = shown.join(", ");
    if names.len() > shown.len() {
        format!("{text}, …")
    } else {
        shown.drain(..);
        text
    }
}

/// The one-line neutral summary of a diff (the timeline entry's face).
fn summary_of(diff: &Diff, created: bool) -> String {
    if created {
        return HIST_CREATED.to_string();
    }
    let mut parts = Vec::new();
    for year in &diff.years_added {
        parts.push(format!("{HIST_YEAR_ADDED} ({year})"));
    }
    if !diff.cells.is_empty() {
        let names: Vec<String> = diff.cells.iter().map(|(n, _)| n.clone()).collect();
        parts.push(format!(
            "{} {HIST_CELLS_CHANGED} : {}",
            diff.cells.len(),
            named(&names)
        ));
    }
    if !diff.judgment.is_empty() {
        let names: Vec<String> = diff.judgment.iter().map(|(n, _)| n.clone()).collect();
        parts.push(format!("{HIST_JUDGMENT} : {}", named(&names)));
    }
    if diff.rationale_changed {
        parts.push(HIST_RATIONALE_CHANGED.to_string());
    }
    if diff.other || parts.is_empty() {
        parts.push(HIST_OTHER.to_string());
    }
    parts.join(" · ")
}

/// The detail lines of a diff (revealed on demand — `label : avant → après` each).
fn detail_of(diff: &Diff, created: bool) -> Vec<String> {
    if created {
        return vec![HIST_CREATED.to_string()];
    }
    let mut lines = Vec::new();
    for year in &diff.years_added {
        lines.push(format!("{HIST_YEAR_ADDED} ({year})"));
    }
    lines.extend(diff.cells.iter().map(|(_, line)| line.clone()));
    lines.extend(diff.judgment.iter().map(|(_, line)| line.clone()));
    if diff.rationale_changed {
        lines.push(HIST_RATIONALE_CHANGED.to_string());
    }
    if diff.other || lines.is_empty() {
        lines.push(HIST_OTHER.to_string());
    }
    lines
}

fn day_of(stamp: &str) -> String {
    stamp.get(..10).unwrap_or(stamp).to_string()
}

fn time_of(stamp: &str) -> String {
    stamp.get(11..16).unwrap_or_default().to_string()
}

/// Build the timeline (NEWEST first) from the study's snapshot series (OLDEST first, as
/// [`list_judgment_snapshots`](steadyinvest_persistence::Journal::list_judgment_snapshots)
/// returns it): each entry summarizes the diff against its predecessor; the very first snapshot
/// is the creation. `first_of_day` marks where the UI draws the day header.
pub fn history_entries(
    snapshots: &[(Uuid, String, Study)],
    format: NumberFormat,
) -> Vec<HistoryEntryView> {
    let mut entries: Vec<HistoryEntryView> = Vec::with_capacity(snapshots.len());
    for index in (0..snapshots.len()).rev() {
        let (id, stamp, next) = &snapshots[index];
        let (created, summary) = match index.checked_sub(1).map(|i| &snapshots[i].2) {
            Some(prev) => (false, summary_of(&diff_states(prev, next, format), false)),
            None => (true, summary_of(&empty_diff(), true)),
        };
        let _ = created;
        entries.push(HistoryEntryView {
            id: *id,
            day: day_of(stamp),
            first_of_day: false, // filled below
            time: time_of(stamp),
            summary,
        });
    }
    let mut last_day: Option<String> = None;
    for entry in &mut entries {
        entry.first_of_day = last_day.as_deref() != Some(entry.day.as_str());
        last_day = Some(entry.day.clone());
    }
    entries
}

fn empty_diff() -> Diff {
    Diff {
        cells: Vec::new(),
        judgment: Vec::new(),
        rationale_changed: false,
        years_added: Vec::new(),
        other: false,
    }
}

/// The detail lines for ONE snapshot (`next`) against its predecessor (`prev`; `None` = the
/// creation entry) — computed on demand when the user expands the entry.
pub fn history_detail(prev: Option<&Study>, next: &Study, format: NumberFormat) -> Vec<String> {
    match prev {
        Some(prev) => detail_of(&diff_states(prev, next, format), false),
        None => detail_of(&empty_diff(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::{
        Cell, Coverage, ForecastLowOption, Freshness, Provenance, Review, Source, Timestamp,
    };

    fn money(s: &str) -> Money {
        Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
    }

    fn prov() -> Provenance {
        Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: Timestamp("2026-07-09T10:00:00Z".to_string()),
            hash_of_dependencies: "manual".to_string(),
        }
    }

    fn cell(value: Option<&str>) -> Cell {
        Cell {
            value: value.map(money),
            source: Source::Manual,
            freshness: Freshness::Current,
            review: Review::Validated,
            coverage: if value.is_some() {
                Coverage::Present
            } else {
                Coverage::ToFill
            },
            provenance: prov(),
            pending: None,
        }
    }

    fn year(y: i32, sales: Option<&str>) -> YearData {
        YearData {
            year: y,
            sales: cell(sales),
            eps: cell(Some("5")),
            high_price: cell(Some("100")),
            low_price: cell(Some("50")),
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
        }
    }

    fn judgment() -> Judgment {
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
            ttm_eps: None,
        }
    }

    fn study(years: Vec<YearData>, judgment: Judgment, rationale: Option<&str>) -> Study {
        let mut s = Study::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "NESN",
            "CHF",
            judgment,
            Timestamp("2026-07-09T09:00:00Z".to_string()),
        );
        s.years = years;
        s.rationale = rationale.map(str::to_string);
        s
    }

    #[test]
    fn the_timeline_is_newest_first_with_day_headers_and_a_creation_entry() {
        let created = study(vec![], judgment(), None);
        let edited = study(vec![year(2024, Some("383000000000"))], judgment(), None);
        let snapshots = vec![
            (
                Uuid::from_u128(0xA),
                "2026-07-08T09:00:00Z".to_string(),
                created,
            ),
            (
                Uuid::from_u128(0xB),
                "2026-07-09T10:30:00Z".to_string(),
                edited,
            ),
        ];
        let entries = history_entries(&snapshots, NumberFormat::Comma);
        assert_eq!(entries.len(), 2);
        // Newest first; each new day carries the header flag.
        assert_eq!(entries[0].id, Uuid::from_u128(0xB));
        assert_eq!(entries[0].day, "2026-07-09");
        assert_eq!(entries[0].time, "10:30");
        assert!(entries[0].first_of_day);
        assert!(entries[1].first_of_day, "a different day re-flags");
        assert_eq!(entries[1].summary, HIST_CREATED);
        // The edit entry names its changed cells — the year 2024 appeared with its figures.
        assert!(
            entries[0].summary.contains(HIST_YEAR_ADDED),
            "the appearing year is named: {}",
            entries[0].summary
        );
    }

    #[test]
    fn a_cell_value_change_details_before_and_after_in_display_units() {
        let before = study(vec![year(2024, Some("383000000000"))], judgment(), None);
        let after = study(vec![year(2024, Some("400000000000"))], judgment(), None);
        let lines = history_detail(Some(&before), &after, NumberFormat::Comma);
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].starts_with(&format!("{LBL_SALES} 2024 : ")) && lines[0].contains(" → "),
            "a value line names the cell and the transition: {}",
            lines[0]
        );
        assert!(
            lines[0].contains("383") && lines[0].contains("400"),
            "both figures show in millions display units: {}",
            lines[0]
        );
    }

    #[test]
    fn a_status_only_change_is_stated_as_status_never_a_phantom_value() {
        let before = study(vec![year(2024, Some("383000000000"))], judgment(), None);
        let mut after = before.clone();
        after.years[0].sales.review = Review::ToReview;
        let lines = history_detail(Some(&before), &after, NumberFormat::Comma);
        assert_eq!(
            lines,
            vec![format!("{LBL_SALES} 2024 : {HIST_STATUS_CHANGED}")]
        );
    }

    #[test]
    fn judgment_and_rationale_changes_are_named() {
        let before = study(vec![year(2024, Some("383000000000"))], judgment(), None);
        let mut after = before.clone();
        after.judgment.estimated_high_eps = Some(money("9"));
        after.rationale = Some("Marge en hausse.".to_string());
        let lines = history_detail(Some(&before), &after, NumberFormat::Comma);
        assert_eq!(lines.len(), 2);
        assert!(
            lines[0].starts_with(&format!("{LBL_EST_HIGH_EPS} : {HIST_EMPTY_SLOT} → ")),
            "the judgment line shows unset → value: {}",
            lines[0]
        );
        assert_eq!(lines[1], HIST_RATIONALE_CHANGED);
        let d = diff_states(&before, &after, NumberFormat::Comma);
        let summary = summary_of(&d, false);
        assert!(
            summary.contains(HIST_JUDGMENT) && summary.contains(HIST_RATIONALE_CHANGED),
            "the summary names both facets: {summary}"
        );
    }

    #[test]
    fn an_uncategorized_change_states_other_never_an_empty_entry() {
        let before = study(vec![], judgment(), None);
        let mut after = before.clone();
        after.security_ticker = "ROG".to_string();
        let lines = history_detail(Some(&before), &after, NumberFormat::Comma);
        assert_eq!(lines, vec![HIST_OTHER.to_string()]);
    }

    #[test]
    fn a_long_change_list_ellipsizes_the_summary_but_not_the_detail() {
        let before = study(
            vec![
                year(2021, Some("1000000")),
                year(2022, Some("1000000")),
                year(2023, Some("1000000")),
                year(2024, Some("1000000")),
            ],
            judgment(),
            None,
        );
        let mut after = before.clone();
        for y in &mut after.years {
            y.sales = cell(Some("2000000"));
        }
        let d = diff_states(&before, &after, NumberFormat::Comma);
        let summary = summary_of(&d, false);
        assert!(
            summary.contains(", …"),
            "more than three names ellipsize: {summary}"
        );
        assert_eq!(
            detail_of(&d, false).len(),
            4,
            "the detail stays complete — the ellipsis is summary-only"
        );
    }
}
