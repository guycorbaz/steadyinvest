//! The draft inbox « Propositions », read side (Story 8.5a — FR72, FR73; UX spec §5.1–5.4).
//!
//! Pure view logic over already-read drafts and studies: no IO, no Slint, no decision (8.5b). The
//! rows carry app text (targets, formatted values, dates) apart from the AI-written strings —
//! comment, proposed note text, proposed company name, origin client and model — which travel in
//! their own `ai_*` fields and reach Slint only through `AiFrame` (spec §4.1, the structural test
//! in `posture.rs`).
//!
//! Values reuse the history view's labels and display (`viewmodel::history`): « Actuel » and
//! « Proposé » are spelled by the grid's own number path (millions + locale for sales / pre-tax
//! profit, #117), never re-derived here. A proposed value was validated at submission (8.3); it is
//! parsed again with the contract's registry only to be formatted.
//!
//! Also here: the counts behind the rail label, the study reminder and the Études signals, and the
//! poller's pure decision (arch A9).

use std::collections::HashMap;

use steadyinvest_contract::{
    DraftField, DraftFieldKind, DraftKind, DraftPayload, DraftStatus, DraftValue,
    ForecastLowOption, Study,
};
use steadyinvest_core::rounding::DisplayField;
use steadyinvest_persistence::DraftRecord;
use uuid::Uuid;

use crate::state::{DraftFreshness, draft_freshness};
use crate::viewmodel::engine::{LBL_EST_HIGH_EPS, LBL_EST_LOW_EPS, LBL_HIGH_PE, LBL_LOW_PE};
use crate::viewmodel::entry;
use crate::viewmodel::format::NumberFormat;
use crate::viewmodel::history::{
    CELL_FIELDS, HIST_EMPTY_SLOT, LBL_DIVIDEND_YEAR, LBL_EPS_GROWTH, LBL_FORECAST_LOW_OPTION,
    LBL_SALES_GROWTH, LBL_SEVERE_LOW, cell_value_display, judgment_value_display,
};
use crate::viewmodel::notes::date_fr;

// ── Vocabulary built in Rust (posture-inventoried) — the forecast-low options as the §4 chips
//    spell them (`study_screen.slint`), so « Actuel » / « Proposé » read like the form. ──

pub const OPT_AVG_LOW_PE_TIMES_EPS: &str = "PER bas × BPA bas";
pub const OPT_AVG_LOW_PRICE_5Y: &str = "Prix bas moyen 5 ans";
pub const OPT_RECENT_SEVERE_LOW: &str = "Plus bas sévère récent";
pub const OPT_DIVIDEND_SUPPORTED: &str = "Soutenu par dividende";
/// The « Étude : » filter's first option (spec §3.3).
pub const STUDY_FILTER_ALL: &str = "Toutes les études";

/// Every app string of the inbox built in Rust, scanned by the posture gate (FR13).
#[cfg(test)]
pub const DRAFTS_USER_FACING_LABELS: &[&str] = &[
    OPT_AVG_LOW_PE_TIMES_EPS,
    OPT_AVG_LOW_PRICE_5Y,
    OPT_RECENT_SEVERE_LOW,
    OPT_DIVIDEND_SUPPORTED,
    STUDY_FILTER_ALL,
];

/// The French label of a forecast-low option, as the §4 chips show it.
pub fn option_label(option: ForecastLowOption) -> &'static str {
    match option {
        ForecastLowOption::AvgLowPeTimesEps => OPT_AVG_LOW_PE_TIMES_EPS,
        ForecastLowOption::AvgLowPriceLast5y => OPT_AVG_LOW_PRICE_5Y,
        ForecastLowOption::RecentSevereLow => OPT_RECENT_SEVERE_LOW,
        ForecastLowOption::DividendSupported => OPT_DIVIDEND_SUPPORTED,
    }
}

/// The grid's entry wire key of a draftable CELL field (`viewmodel::entry::FIELD_*`) — the
/// field→wire direction, once (the wire→field direction of judgments is
/// `state::cells::judgment_draft_field`). `None` for a judgment field.
pub fn cell_wire(field: DraftField) -> Option<&'static str> {
    Some(match field {
        DraftField::Sales => entry::FIELD_SALES,
        DraftField::Eps => entry::FIELD_EPS,
        DraftField::HighPrice => entry::FIELD_HIGH,
        DraftField::LowPrice => entry::FIELD_LOW,
        DraftField::DividendPerShare => entry::FIELD_DIVIDEND,
        DraftField::PreTaxProfit => entry::FIELD_PRETAX,
        DraftField::BookValuePerShare => entry::FIELD_BOOK,
        _ => return None,
    })
}

/// A judgment field's history label and display scale; `None` for a cell field.
fn judgment_label_display(field: DraftField) -> Option<(&'static str, Option<DisplayField>)> {
    Some(match field {
        DraftField::EstimatedHighEps => (LBL_EST_HIGH_EPS, Some(DisplayField::PerShare)),
        DraftField::EstimatedLowEps => (LBL_EST_LOW_EPS, Some(DisplayField::PerShare)),
        DraftField::ProjectedSalesGrowthPct => (LBL_SALES_GROWTH, Some(DisplayField::Percent)),
        DraftField::ProjectedEpsGrowthPct => (LBL_EPS_GROWTH, Some(DisplayField::Percent)),
        DraftField::JudgedAvgHighPe => (LBL_HIGH_PE, Some(DisplayField::PeRatio)),
        DraftField::JudgedAvgLowPe => (LBL_LOW_PE, Some(DisplayField::PeRatio)),
        DraftField::RecentSevereLow => (LBL_SEVERE_LOW, Some(DisplayField::Price)),
        DraftField::PresentFullYearDividend => (LBL_DIVIDEND_YEAR, Some(DisplayField::PerShare)),
        DraftField::ForecastLowOption => (LBL_FORECAST_LOW_OPTION, None),
        _ => return None,
    })
}

/// The French label of a draftable field (the history's spelling).
pub fn field_label(field: DraftField) -> &'static str {
    if let Some(wire) = cell_wire(field) {
        return CELL_FIELDS
            .iter()
            .find(|(w, _)| *w == wire)
            .map(|(_, label)| *label)
            .unwrap_or(HIST_EMPTY_SLOT);
    }
    judgment_label_display(field).map_or(HIST_EMPTY_SLOT, |(label, _)| label)
}

/// A draftable field's value spelled like the grid / the history: millions + locale for the
/// amount cells, the judgment scales, an option by its chip label, the em-dash when absent.
pub fn value_display(field: DraftField, value: Option<DraftValue>, format: NumberFormat) -> String {
    match (cell_wire(field), value) {
        (Some(wire), Some(DraftValue::Number(m))) => cell_value_display(Some(m), wire, format),
        (Some(wire), None) => cell_value_display(None, wire, format),
        (None, Some(DraftValue::Option(o))) => option_label(o).to_string(),
        (None, Some(DraftValue::Number(m))) => match judgment_label_display(field) {
            Some((_, Some(display))) => judgment_value_display(Some(m), display, format),
            _ => HIST_EMPTY_SLOT.to_string(),
        },
        _ => HIST_EMPTY_SLOT.to_string(),
    }
}

/// The kind chips of « À traiter » (spec §3.3: « Toutes · Valeurs · Jugements · Notes · Études »).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KindFilter {
    #[default]
    All,
    Values,
    Judgments,
    Notes,
    Studies,
}

impl KindFilter {
    /// The Slint wire key; anything unknown falls back to « Toutes ».
    pub fn from_wire(s: &str) -> Self {
        match s {
            "values" => Self::Values,
            "judgments" => Self::Judgments,
            "notes" => Self::Notes,
            "studies" => Self::Studies,
            _ => Self::All,
        }
    }

    fn admits(self, kind: DraftKind) -> bool {
        match self {
            Self::All => true,
            Self::Values => kind == DraftKind::Cell,
            Self::Judgments => kind == DraftKind::Judgment,
            Self::Notes => kind == DraftKind::Note,
            Self::Studies => kind == DraftKind::Study,
        }
    }
}

/// A row's state word (spec §3.3): fresh (none), « périmée », « cible disparue ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Fresh,
    Stale,
    TargetGone,
}

impl RowState {
    /// The Slint wire value (the words are `@tr` literals on the Slint side).
    pub fn wire(self) -> i32 {
        match self {
            RowState::Fresh => 0,
            RowState::Stale => 1,
            RowState::TargetGone => 2,
        }
    }
}

/// One row of « À traiter ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxRow {
    pub id: Uuid,
    pub kind: DraftKind,
    /// The first row of its group carries the group header.
    pub group_start: bool,
    /// « {TICKER} ({DEV}) » — empty for the draft-study group (its header is the Slint
    /// « Nouvelles études »).
    pub group_title: String,
    /// The row belongs to the draft-study group.
    pub group_is_studies: bool,
    /// App text: « {TICKER} · {champ} · {année} », « {TICKER} · {champ} », « {TICKER} » for a
    /// note (Slint adds « · nouvelle note »), « {TICKER} ({DEV}) » for a draft study (Slint
    /// prefixes « Nouvelle étude : »). Ticker / currency passed `identifier_invalid` (8.3).
    pub target: String,
    /// « Actuel » / « Proposé », app-formatted; empty for a note or a draft study.
    pub current: String,
    pub proposed: String,
    pub state: RowState,
    /// JJ/MM/AAAA, local time.
    pub submitted: String,
    /// AI-written (reach Slint only through `AiFrame`).
    pub ai_client: String,
    pub ai_model: String,
    /// The comment — for a note draft, the proposed note text then the comment; for a draft study,
    /// « {nom} — {commentaire} » (no name: the comment alone).
    pub ai_text: String,
}

/// The « Étude : » filter: every study with pending drafts (the draft-study group is reached by
/// the kind chip « Études »).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyChoice {
    pub study_id: Uuid,
    pub label: String,
}

/// The inbox, read side.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InboxView {
    pub rows: Vec<InboxRow>,
    pub study_choices: Vec<StudyChoice>,
}

/// A draft the inbox cannot show — its payload does not parse (the read validated it, so this is
/// a defect or a concurrent change); the whole inbox then reads « indisponible » (no partial list
/// with guessed values — decision 4 of the story).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unshowable {
    pub draft_id: Uuid,
    pub detail: String,
}

fn group_title(study: &Study) -> String {
    format!("{} ({})", study.security_ticker, study.native_currency)
}

fn payload_of(record: &DraftRecord) -> Result<DraftPayload, Unshowable> {
    serde_json::from_str(&record.payload).map_err(|e| Unshowable {
        draft_id: record.id,
        detail: format!("payload does not parse: {e}"),
    })
}

/// The row's target and values (app text) for a draft of an existing study.
fn target_and_values(
    record: &DraftRecord,
    payload: &DraftPayload,
    study: Option<&Study>,
    format: NumberFormat,
) -> (String, String, String) {
    let ticker = study.map_or(record.security_ticker.as_str(), |s| {
        s.security_ticker.as_str()
    });
    if record.kind == DraftKind::Note {
        return (ticker.to_string(), String::new(), String::new());
    }
    let Some(target) = payload.target.as_ref() else {
        return (ticker.to_string(), String::new(), String::new());
    };
    let Some((field, year)) = DraftField::of_target(target) else {
        // An unknown field (a malformed import): named by its ticker only — the raw key is never
        // shown (8.2b G3 F4); the row reads « cible disparue ».
        return (
            ticker.to_string(),
            HIST_EMPTY_SLOT.to_string(),
            HIST_EMPTY_SLOT.to_string(),
        );
    };
    let label = field_label(field);
    let target_text = match year {
        Some(y) => format!("{ticker} · {label} · {y}"),
        None => format!("{ticker} · {label}"),
    };
    let current = study.and_then(|s| match (field.kind(), year) {
        (DraftFieldKind::Cell, Some(y)) => s
            .years
            .iter()
            .find(|yd| yd.year == y)
            .and_then(|yd| field.cell_in(yd))
            .and_then(|c| c.value)
            .map(DraftValue::Number),
        (DraftFieldKind::Judgment, _) => field.judgment_value(&s.judgment),
        _ => None,
    });
    let proposed = payload
        .proposed_value
        .as_deref()
        .and_then(|t| field.parse_value(t).ok());
    (
        target_text,
        value_display(field, current, format),
        value_display(field, proposed, format),
    )
}

/// Build « À traiter »: PENDING drafts only, grouped by study (header « {TICKER} ({DEV}) »,
/// newest group first), the draft studies last under « Nouvelles études », rows newest first
/// (`list_drafts` is oldest first — reversed). `studies` holds today's study of every pending
/// draft that has one (absent = deleted: the row reads « cible disparue »).
pub fn inbox_rows(
    drafts: &[DraftRecord],
    studies: &HashMap<Uuid, Study>,
    format: NumberFormat,
    kind: KindFilter,
    study_filter: Option<Uuid>,
) -> Result<InboxView, Unshowable> {
    // Newest first: the timestamps are RFC 3339 UTC (sortable as text); ties keep the id order.
    let mut pending: Vec<&DraftRecord> = drafts
        .iter()
        .filter(|d| d.status == DraftStatus::Pending)
        .collect();
    pending.sort_by(|a, b| {
        b.created_at
            .0
            .cmp(&a.created_at.0)
            .then_with(|| b.id.cmp(&a.id))
    });

    // Groups in order of their newest draft; the draft-study group last.
    let mut group_order: Vec<Uuid> = Vec::new();
    for d in &pending {
        if let Some(id) = d.study_id
            && !group_order.contains(&id)
        {
            group_order.push(id);
        }
    }
    let study_choices = group_order
        .iter()
        .map(|id| StudyChoice {
            study_id: *id,
            label: studies.get(id).map_or_else(
                || {
                    pending
                        .iter()
                        .find(|d| d.study_id == Some(*id))
                        .map(|d| d.security_ticker.clone())
                        .unwrap_or_default()
                },
                group_title,
            ),
        })
        .collect();

    let mut rows = Vec::new();
    let push_group = |group: Option<Uuid>, rows: &mut Vec<InboxRow>| -> Result<(), Unshowable> {
        let mut first = true;
        for d in pending.iter().filter(|d| d.study_id == group) {
            if !kind.admits(d.kind) {
                continue;
            }
            if let Some(filter) = study_filter
                && d.study_id != Some(filter)
            {
                continue;
            }
            let payload = payload_of(d)?;
            let study = group.and_then(|id| studies.get(&id));
            let state = match draft_freshness(study, d.kind, &payload) {
                DraftFreshness::Fresh => RowState::Fresh,
                DraftFreshness::Stale => RowState::Stale,
                DraftFreshness::TargetGone(_) => RowState::TargetGone,
            };
            let (target, current, proposed, ai_text) = if d.kind == DraftKind::Study {
                let currency = d.native_currency.clone().unwrap_or_default();
                let ai_text = match payload.company_name.as_deref().filter(|n| !n.is_empty()) {
                    Some(name) => format!("{name} — {}", d.comment),
                    None => d.comment.clone(),
                };
                (
                    format!("{} ({currency})", d.security_ticker),
                    String::new(),
                    String::new(),
                    ai_text,
                )
            } else {
                let (target, current, proposed) = target_and_values(d, &payload, study, format);
                let ai_text = match (d.kind, payload.note_text.as_deref()) {
                    (DraftKind::Note, Some(text)) => format!("{text} — {}", d.comment),
                    _ => d.comment.clone(),
                };
                (target, current, proposed, ai_text)
            };
            rows.push(InboxRow {
                id: d.id,
                kind: d.kind,
                group_start: first,
                group_title: study.map(group_title).unwrap_or_else(|| {
                    if group.is_some() {
                        d.security_ticker.clone()
                    } else {
                        String::new()
                    }
                }),
                group_is_studies: group.is_none(),
                target,
                current,
                proposed,
                state,
                submitted: date_fr(&d.created_at),
                ai_client: d.origin_client.clone(),
                ai_model: d.origin_model.clone(),
                ai_text,
            });
            first = false;
        }
        Ok(())
    };
    for id in &group_order {
        push_group(Some(*id), &mut rows)?;
    }
    push_group(None, &mut rows)?;
    Ok(InboxView {
        rows,
        study_choices,
    })
}

/// The pending counts the shell shows: per study (the reminder band, the Études rows' « ★ {n} »)
/// and the draft studies (the Études band), plus the total (the rail).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PendingCounts {
    pub by_study: HashMap<Uuid, usize>,
    pub draft_studies: usize,
    pub total: usize,
}

pub fn pending_counts(drafts: &[DraftRecord]) -> PendingCounts {
    let mut counts = PendingCounts::default();
    for d in drafts.iter().filter(|d| d.status == DraftStatus::Pending) {
        counts.total += 1;
        match d.study_id {
            Some(id) => *counts.by_study.entry(id).or_default() += 1,
            None => counts.draft_studies += 1,
        }
    }
    counts
}

/// What a poll tick asks for (arch A9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollAction {
    /// Another connection committed (or first tick, or recovering from a failure): re-read.
    Reread,
    /// Nothing changed.
    Nothing,
    /// The pragma could not be read: the inbox reads « indisponible » with this cause.
    Failed(String),
}

/// The poller's memory: the last `data_version` seen and whether the last tick failed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PollState {
    last: Option<i64>,
    failed: bool,
}

impl PollState {
    /// Decide one tick from the pragma read (`Ok(None)` = no dossier open). Pure: fed by the timer
    /// in `wiring::drafts`, unit-tested here without a clock.
    pub fn on_tick(&mut self, read: Result<Option<i64>, String>) -> PollAction {
        match read {
            Err(cause) => {
                self.failed = true;
                PollAction::Failed(cause)
            }
            Ok(None) => {
                // No dossier: nothing to watch; a later dossier re-reads on its first tick.
                let changed = self.last.is_some() || self.failed;
                self.last = None;
                self.failed = false;
                if changed {
                    PollAction::Reread
                } else {
                    PollAction::Nothing
                }
            }
            Ok(Some(v)) => {
                let reread = self.failed || self.last != Some(v);
                self.last = Some(v);
                self.failed = false;
                if reread {
                    PollAction::Reread
                } else {
                    PollAction::Nothing
                }
            }
        }
    }

    /// Forget the last version (a dossier switch, an explicit push): the next tick re-reads.
    pub fn reset(&mut self) {
        *self = PollState::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::{
        Cell, Coverage, DRAFT_PAYLOAD_VERSION, DraftTarget, Freshness, Judgment, Money, Provenance,
        Review, SCHEMA_VERSION, Source, Timestamp, YearData, draft_fingerprint,
    };

    fn money(s: &str) -> Money {
        serde_json::from_str(&format!("\"{s}\"")).expect("decimal")
    }

    fn cell(v: &str) -> Cell {
        Cell {
            value: Some(money(v)),
            source: Source::Manual,
            freshness: Freshness::Current,
            review: Review::Validated,
            coverage: Coverage::Present,
            provenance: Provenance {
                ai_origin: None,
                source: Source::Manual,
                logical_version: 1,
                timestamp: Timestamp("2026-09-01T00:00:00Z".to_string()),
                hash_of_dependencies: "manual".to_string(),
            },
            pending: None,
        }
    }

    fn study(id: u128, ticker: &str) -> Study {
        let mut years = Vec::new();
        for y in 2022..=2024 {
            years.push(YearData {
                year: y,
                sales: cell("1500000000"),
                eps: cell("3.10"),
                high_price: cell("120"),
                low_price: cell("90"),
                dividend_per_share: None,
                pre_tax_profit: None,
                book_value_per_share: None,
            });
        }
        Study {
            id: Uuid::from_u128(id),
            journal_id: Uuid::from_u128(1),
            security_ticker: ticker.to_string(),
            native_currency: "CHF".to_string(),
            years,
            judgment: Judgment {
                ai_placed: Default::default(),
                estimated_high_eps: Some(money("5.20")),
                estimated_low_eps: None,
                projected_sales_growth_pct: Some(money("7")),
                projected_eps_growth_pct: None,
                judged_avg_high_pe: None,
                judged_avg_low_pe: None,
                forecast_low_option: ForecastLowOption::AvgLowPriceLast5y,
                recent_severe_low: None,
                current_price: None,
                present_full_year_dividend: None,
                ttm_eps: None,
            },
            rationale: None,
            company_name: None,
            notes: Vec::new(),
            created_at: Timestamp("2026-09-01T00:00:00Z".into()),
            schema_version: SCHEMA_VERSION,
        }
    }

    fn record(
        id: u128,
        kind: DraftKind,
        study_id: Option<u128>,
        ticker: &str,
        created: &str,
        payload: &DraftPayload,
    ) -> DraftRecord {
        DraftRecord {
            id: Uuid::from_u128(id),
            kind,
            study_id: study_id.map(Uuid::from_u128),
            security_ticker: ticker.to_string(),
            native_currency: (kind == DraftKind::Study).then(|| "USD".to_string()),
            status: DraftStatus::Pending,
            created_at: Timestamp(created.to_string()),
            decided_at: None,
            comment: "Commentaire de l'IA".to_string(),
            origin_client: "claude-code".to_string(),
            origin_model: "opus".to_string(),
            stale_at_decision: None,
            edited_before_validation: None,
            created_study_id: None,
            payload: serde_json::to_string(payload).expect("payload json"),
        }
    }

    fn payload(
        target: Option<DraftTarget>,
        value: Option<&str>,
        base: Option<String>,
    ) -> DraftPayload {
        DraftPayload {
            version: DRAFT_PAYLOAD_VERSION,
            target,
            proposed_value: value.map(str::to_string),
            note_text: None,
            company_name: None,
            base_fingerprint: base,
        }
    }

    fn fp(s: &Study, t: &DraftTarget) -> Option<String> {
        draft_fingerprint(s, t, steadyinvest_core::METHOD_VERSION)
    }

    fn fixture() -> (Vec<DraftRecord>, HashMap<Uuid, Study>) {
        let a = study(0xA, "NESN");
        let b = study(0xB, "ROG");
        let sales = DraftTarget::Cell {
            fiscal_year: 2024,
            field: "sales".into(),
        };
        let growth = DraftTarget::Judgment {
            field: "projected_sales_growth_pct".into(),
        };
        let option = DraftTarget::Judgment {
            field: "forecast_low_option".into(),
        };
        let gone = DraftTarget::Cell {
            fiscal_year: 2019,
            field: "eps".into(),
        };
        let mut note = payload(None, None, None);
        note.note_text = Some("Texte de note".into());
        let mut dstudy = payload(None, None, None);
        dstudy.company_name = Some("Apple Inc.".into());
        let drafts = vec![
            record(
                1,
                DraftKind::Cell,
                Some(0xA),
                "NESN",
                "2026-09-20T10:00:00Z",
                &payload(Some(sales.clone()), Some("1800000000"), fp(&a, &sales)),
            ),
            // Stale: its base does not match today's.
            record(
                2,
                DraftKind::Judgment,
                Some(0xA),
                "NESN",
                "2026-09-21T10:00:00Z",
                &payload(
                    Some(growth.clone()),
                    Some("9"),
                    Some("fp1:not-today".into()),
                ),
            ),
            record(
                3,
                DraftKind::Note,
                Some(0xB),
                "ROG",
                "2026-09-25T10:00:00Z",
                &note,
            ),
            record(
                4,
                DraftKind::Study,
                None,
                "AAPL",
                "2026-09-27T10:00:00Z",
                &dstudy,
            ),
            record(
                5,
                DraftKind::Cell,
                Some(0xB),
                "ROG",
                "2026-09-22T10:00:00Z",
                &payload(Some(gone), Some("4"), Some("fp1:x".into())),
            ),
            record(
                6,
                DraftKind::Judgment,
                Some(0xB),
                "ROG",
                "2026-09-23T10:00:00Z",
                &payload(
                    Some(option.clone()),
                    Some("recent_severe_low"),
                    fp(&b, &option),
                ),
            ),
        ];
        let mut studies = HashMap::new();
        studies.insert(a.id, a);
        studies.insert(b.id, b);
        (drafts, studies)
    }

    #[test]
    fn groups_newest_first_draft_studies_last_rows_newest_first() {
        let (drafts, studies) = fixture();
        let view = inbox_rows(
            &drafts,
            &studies,
            NumberFormat::default(),
            KindFilter::All,
            None,
        )
        .expect("builds");
        let ids: Vec<u128> = view.rows.iter().map(|r| r.id.as_u128()).collect();
        // ROG's newest (25/09) beats NESN's newest (21/09); the draft study (27/09) still last.
        assert_eq!(ids, vec![3, 6, 5, 2, 1, 4]);
        let starts: Vec<bool> = view.rows.iter().map(|r| r.group_start).collect();
        assert_eq!(starts, vec![true, false, false, true, false, true]);
        assert_eq!(view.rows[0].group_title, "ROG (CHF)");
        assert_eq!(view.rows[3].group_title, "NESN (CHF)");
        assert!(view.rows[5].group_is_studies && view.rows[5].group_title.is_empty());
        assert_eq!(
            view.study_choices
                .iter()
                .map(|c| c.label.as_str())
                .collect::<Vec<_>>(),
            vec!["ROG (CHF)", "NESN (CHF)"]
        );
    }

    #[test]
    fn every_kind_has_its_target_values_and_state() {
        let (drafts, studies) = fixture();
        let view = inbox_rows(
            &drafts,
            &studies,
            NumberFormat::default(),
            KindFilter::All,
            None,
        )
        .expect("builds");
        let row = |n: u128| view.rows.iter().find(|r| r.id.as_u128() == n).expect("row");
        // A millions cell: shown in the grid's millions, like « Actuel ».
        let sales = row(1);
        assert_eq!(
            sales.target,
            format!("NESN · {} · 2024", field_label(DraftField::Sales))
        );
        assert_eq!(
            sales.current,
            cell_value_display(
                Some(money("1500000000")),
                entry::FIELD_SALES,
                NumberFormat::default()
            )
        );
        assert_eq!(
            sales.proposed,
            cell_value_display(
                Some(money("1800000000")),
                entry::FIELD_SALES,
                NumberFormat::default()
            )
        );
        assert_eq!(sales.state, RowState::Fresh);
        // A percent judgment, stale.
        let growth = row(2);
        assert_eq!(growth.target, format!("NESN · {LBL_SALES_GROWTH}"));
        assert_eq!(
            growth.proposed,
            judgment_value_display(
                Some(money("9")),
                DisplayField::Percent,
                NumberFormat::default()
            )
        );
        assert_eq!(growth.state, RowState::Stale);
        // An option, by its chip label.
        let option = row(6);
        assert_eq!(option.current, OPT_AVG_LOW_PRICE_5Y);
        assert_eq!(option.proposed, OPT_RECENT_SEVERE_LOW);
        // A year removed: target gone, today's value absent.
        let gone = row(5);
        assert_eq!(gone.state, RowState::TargetGone);
        assert_eq!(gone.current, HIST_EMPTY_SLOT);
        // A note: no values, its text in the AI field with the comment.
        let note = row(3);
        assert_eq!(note.target, "ROG");
        assert!(note.current.is_empty() && note.proposed.is_empty());
        assert_eq!(note.ai_text, "Texte de note — Commentaire de l'IA");
        // A draft study: ticker + currency as app text; the name before the comment, AI side.
        let dstudy = row(4);
        assert_eq!(dstudy.target, "AAPL (USD)");
        assert_eq!(dstudy.ai_text, "Apple Inc. — Commentaire de l'IA");
        assert_eq!(dstudy.ai_client, "claude-code");
        assert_eq!(dstudy.ai_model, "opus");
        assert_eq!(
            dstudy.submitted,
            date_fr(&Timestamp("2026-09-27T10:00:00Z".into()))
        );
    }

    #[test]
    fn a_deleted_study_reads_target_gone_and_is_named_by_its_ticker() {
        let (drafts, mut studies) = fixture();
        studies.remove(&Uuid::from_u128(0xA));
        let view = inbox_rows(
            &drafts,
            &studies,
            NumberFormat::default(),
            KindFilter::All,
            None,
        )
        .expect("builds");
        let sales = view.rows.iter().find(|r| r.id.as_u128() == 1).expect("row");
        assert_eq!(sales.state, RowState::TargetGone);
        assert_eq!(sales.group_title, "NESN");
    }

    #[test]
    fn filters_by_kind_and_by_study() {
        let (drafts, studies) = fixture();
        let only = |k, s| {
            inbox_rows(&drafts, &studies, NumberFormat::default(), k, s)
                .expect("builds")
                .rows
                .iter()
                .map(|r| r.id.as_u128())
                .collect::<Vec<_>>()
        };
        assert_eq!(only(KindFilter::Values, None), vec![5, 1]);
        assert_eq!(only(KindFilter::Judgments, None), vec![6, 2]);
        assert_eq!(only(KindFilter::Notes, None), vec![3]);
        assert_eq!(only(KindFilter::Studies, None), vec![4]);
        assert_eq!(
            only(KindFilter::All, Some(Uuid::from_u128(0xA))),
            vec![2, 1]
        );
        // The group header moves to the first row the filter keeps.
        let view = inbox_rows(
            &drafts,
            &studies,
            NumberFormat::default(),
            KindFilter::Values,
            None,
        )
        .expect("builds");
        assert!(view.rows.iter().all(|r| r.group_start));
        assert_eq!(KindFilter::from_wire("??"), KindFilter::All);
    }

    #[test]
    fn decided_drafts_are_not_listed_and_counts_are_per_study() {
        let (mut drafts, studies) = fixture();
        drafts[0].status = DraftStatus::Validated;
        drafts[0].decided_at = Some(Timestamp("2026-09-28T00:00:00Z".into()));
        let view = inbox_rows(
            &drafts,
            &studies,
            NumberFormat::default(),
            KindFilter::All,
            None,
        )
        .expect("builds");
        assert!(view.rows.iter().all(|r| r.id.as_u128() != 1));
        let counts = pending_counts(&drafts);
        assert_eq!(counts.total, 5);
        assert_eq!(counts.draft_studies, 1);
        assert_eq!(counts.by_study.get(&Uuid::from_u128(0xA)), Some(&1));
        assert_eq!(counts.by_study.get(&Uuid::from_u128(0xB)), Some(&3));
    }

    #[test]
    fn an_unparsable_payload_makes_the_whole_inbox_unshowable() {
        let (mut drafts, studies) = fixture();
        drafts[2].payload = "{not json".into();
        let err = inbox_rows(
            &drafts,
            &studies,
            NumberFormat::default(),
            KindFilter::All,
            None,
        )
        .expect_err("no partial list");
        assert_eq!(err.draft_id, Uuid::from_u128(3));
    }

    #[test]
    fn every_draftable_field_has_a_label_and_a_display() {
        for field in DraftField::ALL {
            assert_ne!(
                field_label(field),
                HIST_EMPTY_SLOT,
                "{field:?} has no label"
            );
            let sample = if field == DraftField::ForecastLowOption {
                Some(DraftValue::Option(ForecastLowOption::DividendSupported))
            } else {
                Some(DraftValue::Number(money("12.5")))
            };
            assert_ne!(
                value_display(field, sample, NumberFormat::default()),
                HIST_EMPTY_SLOT,
                "{field:?} has no display"
            );
            assert_eq!(
                cell_wire(field).is_some(),
                field.kind() == DraftFieldKind::Cell
            );
        }
    }

    #[test]
    fn the_poller_rereads_on_change_first_tick_and_recovery_only() {
        let mut p = PollState::default();
        assert_eq!(p.on_tick(Ok(Some(3))), PollAction::Reread, "first tick");
        assert_eq!(p.on_tick(Ok(Some(3))), PollAction::Nothing);
        assert_eq!(
            p.on_tick(Ok(Some(4))),
            PollAction::Reread,
            "another connection committed"
        );
        assert_eq!(
            p.on_tick(Err("verrouillé".into())),
            PollAction::Failed("verrouillé".into())
        );
        assert_eq!(
            p.on_tick(Ok(Some(4))),
            PollAction::Reread,
            "recovery re-reads"
        );
        assert_eq!(p.on_tick(Ok(Some(4))), PollAction::Nothing);
        assert_eq!(p.on_tick(Ok(None)), PollAction::Reread, "dossier closed");
        assert_eq!(p.on_tick(Ok(None)), PollAction::Nothing);
        p.on_tick(Ok(Some(9)));
        p.reset();
        assert_eq!(p.on_tick(Ok(Some(9))), PollAction::Reread, "after a reset");
    }
}
