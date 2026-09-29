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

use std::collections::{HashMap, HashSet};

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
/// The ⊘ inbox's cause when one draft cannot be shown (G3 — owner-pending wording).
pub const INBOX_CAUSE_UNREADABLE_DRAFT: &str = "une proposition est illisible";

/// Every app string of the inbox built in Rust, scanned by the posture gate (FR13).
#[cfg(test)]
pub const DRAFTS_USER_FACING_LABELS: &[&str] = &[
    OPT_AVG_LOW_PE_TIMES_EPS,
    OPT_AVG_LOW_PRICE_5Y,
    OPT_RECENT_SEVERE_LOW,
    OPT_DIVIDEND_SUPPORTED,
    STUDY_FILTER_ALL,
    INBOX_CAUSE_UNREADABLE_DRAFT,
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
pub(crate) fn judgment_label_display(
    field: DraftField,
) -> Option<(&'static str, Option<DisplayField>)> {
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
    /// The draft's study is archived (G3): listed, marked, left out of the counts.
    Archived,
}

impl RowState {
    /// The Slint wire value (the words are `@tr` literals on the Slint side).
    pub fn wire(self) -> i32 {
        match self {
            RowState::Fresh => 0,
            RowState::Stale => 1,
            RowState::TargetGone => 2,
            RowState::Archived => 3,
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
    /// prefixes « Nouvelle étude : »). A draft study's ticker / currency pass the identifier rule
    /// (enforced on submission, import and read — `persistence::is_ticker`).
    pub target: String,
    /// « Actuel » / « Proposé », app-formatted; empty for a note or a draft study.
    pub current: String,
    pub proposed: String,
    pub state: RowState,
    /// JJ/MM/AAAA, local time.
    pub submitted: String,
    /// AI-written (reach Slint only through `AiFrame`), each on ONE line (compact rows).
    pub ai_client: String,
    pub ai_model: String,
    /// What the draft proposes before its comment: a note text, a company name ("" = none).
    pub ai_lead: String,
    /// The comment.
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
    /// Every pending draft before the kind / study filters (the empty text vs the filter line).
    pub unfiltered: usize,
}

/// A draft the inbox cannot show — its payload does not parse, or its proposed value is no value
/// of its field (the reads validate both, so this is a defect or a concurrent change); the whole
/// inbox then reads « indisponible » with the cause [`INBOX_CAUSE_UNREADABLE_DRAFT`] (no partial
/// list with guessed values — decision 4 of the story; G3: never « Proposé — »).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unshowable {
    pub draft_id: Uuid,
    pub detail: String,
}

/// Collapse line breaks (`\n`, `\r`, U+2028, U+2029), tabs and runs of spaces to single spaces —
/// AI text in a compact, one-line row (G3).
pub fn one_line(text: &str) -> String {
    text.split(|c: char| c.is_whitespace())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The AI-written fields of a draft — the ONE place the view model reads a draft's comment, origin,
/// proposed note text and proposed company name (the posture guard pins it). `compact` (list rows):
/// each made one line; the decision dialog (8.5b) shows the comment and the note text whole, their
/// line breaks kept (the origin is always one line). Returns `(client, model, lead, comment)`.
fn ai_fields(
    d: &DraftRecord,
    payload: &DraftPayload,
    compact: bool,
) -> (String, String, String, String) {
    let lead = match d.kind {
        DraftKind::Note => payload.note_text.as_deref().unwrap_or_default(),
        DraftKind::Study => payload.company_name.as_deref().unwrap_or_default(),
        _ => "",
    };
    let text = |t: &str| {
        if compact {
            one_line(t)
        } else {
            t.trim().to_string()
        }
    };
    (
        one_line(&d.origin_client),
        one_line(&d.origin_model),
        text(lead),
        text(&d.comment),
    )
}

fn group_title(study: &Study) -> String {
    format!("{} ({})", study.security_ticker, study.native_currency)
}

/// The ticker a draft of a study carries, shown only when its study is gone (a note / cell /
/// judgment draft copies its study's ticker at submission — app data). Shown when it reads as a
/// ticker (upper-cased), else the em-dash — never arbitrary text outside an AiFrame.
fn fallback_ticker(d: &DraftRecord) -> String {
    let t = d.security_ticker.trim().to_ascii_uppercase();
    if steadyinvest_persistence::is_ticker(&t) {
        t
    } else {
        HIST_EMPTY_SLOT.to_string()
    }
}

fn payload_of(record: &DraftRecord) -> Result<DraftPayload, Unshowable> {
    serde_json::from_str(&record.payload).map_err(|e| Unshowable {
        draft_id: record.id,
        detail: format!("payload does not parse: {e}"),
    })
}

/// The row's target and values (app text) for a draft of an existing study; a proposed value that
/// is no value of its field is [`Unshowable`].
fn target_and_values(
    record: &DraftRecord,
    payload: &DraftPayload,
    study: Option<&Study>,
    format: NumberFormat,
) -> Result<(String, String, String), Unshowable> {
    let ticker = study.map_or_else(|| fallback_ticker(record), |s| s.security_ticker.clone());
    if record.kind == DraftKind::Note {
        return Ok((ticker, String::new(), String::new()));
    }
    let Some(target) = payload.target.as_ref() else {
        return Ok((ticker, String::new(), String::new()));
    };
    let Some((field, year)) = DraftField::of_target(target) else {
        // An unknown field (a malformed import): named by its ticker only — the raw key is never
        // shown (8.2b G3 F4); the row reads « cible disparue ».
        return Ok((
            ticker,
            HIST_EMPTY_SLOT.to_string(),
            HIST_EMPTY_SLOT.to_string(),
        ));
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
    let proposed = match payload.proposed_value.as_deref() {
        Some(text) => Some(field.parse_value(text).map_err(|problem| Unshowable {
            draft_id: record.id,
            detail: format!("proposed value is no value of {}: {problem:?}", field.key()),
        })?),
        None => None,
    };
    let current_text = value_display(field, current, format);
    let proposed_text = value_display(field, proposed, format);
    Ok((
        target_text,
        current_text.clone(),
        same_decimals(&current_text, &proposed_text, format),
    ))
}

/// The proposed figure spelled with at least the current one's decimals (G3 F12): « 2,10 → 2,50 »,
/// never « 2,10 → 2,5 » — the same value, read side by side. Only between two plain figures.
pub(crate) fn same_decimals(current: &str, proposed: &str, format: NumberFormat) -> String {
    let sep = match format {
        NumberFormat::Comma => ',',
        NumberFormat::Point => '.',
    };
    let figure = |t: &str| {
        !t.is_empty()
            && t.chars().any(|c| c.is_ascii_digit())
            && t.chars().all(|c| {
                c.is_ascii_digit()
                    || c == ','
                    || c == '.'
                    || c == '-'
                    || c == '\u{2212}'
                    || c == '\u{a0}'
                    || c == '\u{202f}'
                    || c == ' '
                    || c == '\''
            })
    };
    if !figure(current) || !figure(proposed) {
        return proposed.to_string();
    }
    let decimals = |t: &str| t.rsplit_once(sep).map_or(0, |(_, d)| d.len());
    let (want, have) = (decimals(current), decimals(proposed));
    if have >= want {
        return proposed.to_string();
    }
    let mut out = proposed.to_string();
    if have == 0 {
        out.push(sep);
    }
    out.extend(std::iter::repeat_n('0', want - have));
    out
}

/// Build « À traiter »: PENDING drafts only, grouped by study (header « {TICKER} ({DEV}) »,
/// newest group first), the draft studies last under « Nouvelles études », rows newest first
/// (the read is oldest first — reversed). `studies` holds today's study of every pending draft
/// that has one (absent = deleted: the row reads « cible disparue »); `archived` the studies
/// archived today (their drafts read « étude archivée » — G3).
pub fn inbox_rows(
    drafts: &[DraftRecord],
    studies: &HashMap<Uuid, Study>,
    archived: &HashSet<Uuid>,
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
                        .map(|d| fallback_ticker(d))
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
            // Every row is BUILT (a defect anywhere makes the inbox unavailable, whatever the
            // filters); the filters only decide what is listed.
            let payload = payload_of(d)?;
            let study = group.and_then(|id| studies.get(&id));
            let state = if group.is_some_and(|id| archived.contains(&id)) {
                RowState::Archived
            } else {
                match draft_freshness(study, d.kind, &payload) {
                    DraftFreshness::Fresh => RowState::Fresh,
                    DraftFreshness::Stale => RowState::Stale,
                    DraftFreshness::TargetGone(_) => RowState::TargetGone,
                }
            };
            let (target, current, proposed) = if d.kind == DraftKind::Study {
                let currency = d.native_currency.clone().unwrap_or_default();
                (
                    format!("{} ({currency})", d.security_ticker),
                    String::new(),
                    String::new(),
                )
            } else {
                target_and_values(d, &payload, study, format)?
            };
            if !kind.admits(d.kind) || study_filter.is_some_and(|f| d.study_id != Some(f)) {
                continue;
            }
            let (ai_client, ai_model, ai_lead, ai_text) = ai_fields(d, &payload, true);
            rows.push(InboxRow {
                id: d.id,
                kind: d.kind,
                group_start: first,
                group_title: study.map(group_title).unwrap_or_else(|| {
                    if group.is_some() {
                        fallback_ticker(d)
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
                ai_client,
                ai_model,
                ai_lead,
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
        unfiltered: pending.len(),
    })
}

// ── The drafts record (Story 8.7 — UX spec §3.3, §5.2 « Registre »; FR77) ──

/// The Registre's outcome chips (spec §3.3): none selected = every outcome (Decision 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutcomeFilter {
    #[default]
    All,
    Pending,
    Validated,
    Undone,
    Rejected,
}

impl OutcomeFilter {
    /// The Slint wire key; anything unknown falls back to every outcome.
    pub fn from_wire(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "validated" => Self::Validated,
            "undone" => Self::Undone,
            "rejected" => Self::Rejected,
            _ => Self::All,
        }
    }

    fn admits(self, status: DraftStatus) -> bool {
        match self {
            Self::All => true,
            Self::Pending => status == DraftStatus::Pending,
            Self::Validated => status == DraftStatus::Validated,
            Self::Undone => status == DraftStatus::ValidatedUndone,
            Self::Rejected => status == DraftStatus::Rejected,
        }
    }
}

/// A record row's outcome (spec §3.2 words, spelled on the Slint side).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// « en attente » (+ its [`RowState`] word: « périmée », « cible disparue », « étude archivée »).
    Pending,
    /// « validée ».
    Validated,
    /// « validée puis annulée ».
    Undone,
    /// « rejetée ».
    Rejected,
}

impl Outcome {
    pub fn wire(self) -> i32 {
        match self {
            Outcome::Pending => 0,
            Outcome::Validated => 1,
            Outcome::Undone => 2,
            Outcome::Rejected => 3,
        }
    }

    fn of(status: DraftStatus) -> Self {
        match status {
            DraftStatus::Pending => Outcome::Pending,
            DraftStatus::Validated => Outcome::Validated,
            DraftStatus::ValidatedUndone => Outcome::Undone,
            DraftStatus::Rejected => Outcome::Rejected,
        }
    }
}

/// One row of the « Registre »: every draft, whatever its outcome — read-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRow {
    pub id: Uuid,
    pub kind: DraftKind,
    /// App text, as « À traiter » spells it (a note: « {TICKER} », Slint adds « · nouvelle note »;
    /// a draft study: « {TICKER} ({DEV}) », Slint prefixes « Nouvelle étude : »).
    pub target: String,
    /// « Actuel » / « Proposé », app-formatted; empty for a note or a draft study.
    pub current: String,
    pub proposed: String,
    pub outcome: Outcome,
    /// A pending draft's state word (fresh / stale / target gone / archived study).
    pub state: RowState,
    /// The decision facts (spec §3.3 « périmée à la décision », « modifiée avant validation »).
    pub stale_at_decision: bool,
    pub edited: bool,
    /// JJ/MM/AAAA, local time; `decided` is "" while pending.
    pub submitted: String,
    pub decided: String,
    /// AI-written, whole (the « Détail » AiFrame).
    pub ai_client: String,
    pub ai_model: String,
    pub ai_lead: String,
    pub ai_text: String,
}

/// The Registre, read side.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecordView {
    pub rows: Vec<RecordRow>,
    /// Every study some draft is about or became — the « Étude : » drop-down.
    pub study_choices: Vec<StudyChoice>,
    /// Every draft before the filters (the empty text vs the filter line).
    pub unfiltered: usize,
}

/// Build the « Registre » (Story 8.7, FR77): every draft, newest submission first, filtered by
/// kind, study (its `study_id` or the `created_study_id` of a validated draft study — Decision 4)
/// and outcome. A draft that cannot be shown makes the whole record unavailable (the inbox rule).
pub fn record_rows(
    drafts: &[DraftRecord],
    studies: &HashMap<Uuid, Study>,
    archived: &HashSet<Uuid>,
    format: NumberFormat,
    kind: KindFilter,
    study_filter: Option<Uuid>,
    outcome: OutcomeFilter,
) -> Result<RecordView, Unshowable> {
    let mut all: Vec<&DraftRecord> = drafts.iter().collect();
    all.sort_by(|a, b| {
        b.created_at
            .0
            .cmp(&a.created_at.0)
            .then_with(|| b.id.cmp(&a.id))
    });
    let mut study_ids: Vec<Uuid> = Vec::new();
    for d in &all {
        for id in [d.study_id, d.created_study_id].into_iter().flatten() {
            if studies.contains_key(&id) && !study_ids.contains(&id) {
                study_ids.push(id);
            }
        }
    }
    let study_choices = study_ids
        .iter()
        .filter_map(|id| {
            studies.get(id).map(|s| StudyChoice {
                study_id: *id,
                label: group_title(s),
            })
        })
        .collect();
    let mut rows = Vec::new();
    for d in &all {
        let payload = payload_of(d)?;
        let study = d.study_id.and_then(|id| studies.get(&id));
        let (target, current, proposed) = if d.kind == DraftKind::Study {
            let currency = d.native_currency.clone().unwrap_or_default();
            (
                format!("{} ({currency})", d.security_ticker),
                String::new(),
                String::new(),
            )
        } else {
            target_and_values(d, &payload, study, format)?
        };
        let state = if d.status != DraftStatus::Pending {
            RowState::Fresh
        } else if d.study_id.is_some_and(|id| archived.contains(&id)) {
            RowState::Archived
        } else {
            match draft_freshness(study, d.kind, &payload) {
                DraftFreshness::Fresh => RowState::Fresh,
                DraftFreshness::Stale => RowState::Stale,
                DraftFreshness::TargetGone(_) => RowState::TargetGone,
            }
        };
        let about = |f: Uuid| d.study_id == Some(f) || d.created_study_id == Some(f);
        if !kind.admits(d.kind)
            || study_filter.is_some_and(|f| !about(f))
            || !outcome.admits(d.status)
        {
            continue;
        }
        let (ai_client, ai_model, ai_lead, ai_text) = ai_fields(d, &payload, false);
        rows.push(RecordRow {
            id: d.id,
            kind: d.kind,
            target,
            current,
            proposed,
            outcome: Outcome::of(d.status),
            state,
            stale_at_decision: d.stale_at_decision == Some(true),
            edited: d.edited_before_validation == Some(true),
            submitted: date_fr(&d.created_at),
            decided: d.decided_at.as_ref().map(date_fr).unwrap_or_default(),
            ai_client,
            ai_model,
            ai_lead,
            ai_text,
        });
    }
    Ok(RecordView {
        rows,
        study_choices,
        unfiltered: all.len(),
    })
}

// ── A study's processed drafts in its history (Story 8.7 — UX spec §5.7; arch A12) ──

/// A processed draft's ★ history entry: its target without the ticker (the history is the
/// study's own), its outcome, its decision time. A pending draft has no entry (`None`).
pub fn history_draft(
    record: &DraftRecord,
) -> Result<Option<crate::viewmodel::history::HistoryDraft>, Unshowable> {
    use crate::viewmodel::history::{
        HIST_DRAFT_NOTE, HIST_DRAFT_REJECTED, HIST_DRAFT_STUDY, HIST_DRAFT_UNDONE,
        HIST_DRAFT_VALIDATED, HIST_DRAFT_VALIDATED_EDITED, HistoryDraft,
    };
    let Some(decided) = record.decided_at.as_ref() else {
        return Ok(None);
    };
    let payload = payload_of(record)?;
    let target = match record.kind {
        DraftKind::Study => HIST_DRAFT_STUDY
            .replacen("{}", record.security_ticker.trim(), 1)
            .replacen(
                "{}",
                record.native_currency.as_deref().unwrap_or("").trim(),
                1,
            ),
        DraftKind::Note => HIST_DRAFT_NOTE.to_string(),
        _ => match payload.target.as_ref().and_then(DraftField::of_target) {
            Some((field, Some(year))) => format!("{} · {year}", field_label(field)),
            Some((field, None)) => field_label(field).to_string(),
            None => HIST_EMPTY_SLOT.to_string(),
        },
    };
    let template = match record.status {
        DraftStatus::Pending => return Ok(None),
        DraftStatus::Validated if record.edited_before_validation == Some(true) => {
            HIST_DRAFT_VALIDATED_EDITED
        }
        DraftStatus::Validated => HIST_DRAFT_VALIDATED,
        DraftStatus::ValidatedUndone => HIST_DRAFT_UNDONE,
        DraftStatus::Rejected => HIST_DRAFT_REJECTED,
    };
    Ok(Some(HistoryDraft {
        id: record.id,
        decided_at: decided.0.clone(),
        summary: template.replacen("{}", &target, 1),
    }))
}

/// The Détail of a ★ history entry: the proposed value (app-formatted, « Proposé {} ») for a cell
/// or judgment draft, and the AI-written parts for the AiFrame (origin, comment, note text or
/// company name, submission date).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryDraftDetail {
    pub proposed: String,
    pub submitted: String,
    pub ai_client: String,
    pub ai_model: String,
    pub ai_lead: String,
    pub ai_text: String,
}

pub fn history_draft_detail(
    record: &DraftRecord,
    study: Option<&Study>,
    format: NumberFormat,
) -> Result<HistoryDraftDetail, Unshowable> {
    let payload = payload_of(record)?;
    let proposed = match record.kind {
        DraftKind::Cell | DraftKind::Judgment => {
            target_and_values(record, &payload, study, format)?.2
        }
        _ => String::new(),
    };
    let (ai_client, ai_model, ai_lead, ai_text) = ai_fields(record, &payload, false);
    Ok(HistoryDraftDetail {
        proposed,
        submitted: date_fr(&record.created_at),
        ai_client,
        ai_model,
        ai_lead,
        ai_text,
    })
}

// ── The decision dialog (Story 8.5b — UX spec §3.3 « Decision dialog », §4.2) ──

/// The context line when the draft's study is not the open one (spec §3.3).
pub const DECISION_CONTEXT_OTHER_STUDY: &str = "Valider ou rejeter ouvre l'étude {ticker} ({dev}).";
/// The ◦ band of a stale draft (spec §3.3).
pub const DECISION_BAND_STALE: &str =
    "La cible {cible} a changé depuis la proposition. Valeur actuelle : {maintenant}.";
/// The ⊘ band of a draft whose target is gone (spec §3.3).
pub const DECISION_BAND_GONE: &str =
    "Cible disparue : {raison} ; la proposition ne peut qu'être rejetée.";
/// The stale confirmation's body (spec §3.3).
pub const DECISION_CONFIRM_STALE: &str = "La cible {cible} a changé depuis la proposition (valeur actuelle : {maintenant}). La valeur proposée la remplacera.";

/// Every app string of the decision dialog built in Rust, scanned by the posture gate (FR13).
#[cfg(test)]
pub const DECISION_USER_FACING_LABELS: &[&str] = &[
    DECISION_CONTEXT_OTHER_STUDY,
    DECISION_BAND_STALE,
    DECISION_BAND_GONE,
    DECISION_CONFIRM_STALE,
];

/// The decision dialog's state band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionState {
    Fresh,
    Stale,
    /// Rejection only.
    TargetGone,
}

impl DecisionState {
    /// The Slint wire value: 0 fresh · 1 stale · 2 target gone.
    pub fn wire(self) -> i32 {
        match self {
            DecisionState::Fresh => 0,
            DecisionState::Stale => 1,
            DecisionState::TargetGone => 2,
        }
    }
}

/// Which field « Modifier avant de valider… » shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    Number,
    Option,
    Note,
}

impl EditKind {
    pub fn wire(self) -> &'static str {
        match self {
            EditKind::Number => "number",
            EditKind::Option => "option",
            EditKind::Note => "note",
        }
    }
}

/// What the decision dialog shows for one pending draft (pure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionView {
    pub kind: DraftKind,
    /// App text, as the inbox row spells it (Slint composes « · nouvelle note » for a note).
    pub target: String,
    /// « Valider ou rejeter ouvre l'étude … » when the draft's study is not the open one, else "".
    pub context: String,
    /// « Actuel » / « Proposé », app-formatted ("" for a note: its « Proposé » is AI text).
    pub current: String,
    pub proposed: String,
    pub state: DecisionState,
    /// The stale / gone band ("" when fresh).
    pub band: String,
    /// The stale confirmation's body ("" unless stale).
    pub confirm_body: String,
    /// Q5: the initial focus on « Annuler » (a `✓` target), else on « Valider ».
    pub focus_cancel: bool,
    pub edit_kind: EditKind,
    /// The edit field's prefill, spelled like the grid (owner number format, millions for sales /
    /// pre-tax profit, the option's chip label). A note's prefill is AI text: `ai_lead`.
    pub edit_prefill: String,
    /// An option field's choices, as the chips spell them.
    pub edit_options: Vec<String>,
    /// JJ/MM/AAAA.
    pub submitted: String,
    /// AI-written, whole (reach Slint only through `AiFrame`).
    pub ai_client: String,
    pub ai_model: String,
    /// The proposed note text (a note draft) or company name (a draft study), "" otherwise.
    pub ai_lead: String,
    pub ai_text: String,
    /// A draft study's proposed ticker and currency (identifiers, app-checked at submission and on
    /// read — exempt from the AiFrame, spec §4.1) — the create form's prefill (Story 8.7); "" else.
    pub study_ticker: String,
    pub study_currency: String,
}

/// A number's edit prefill: the exact value (never display-rounded — an untouched prefill equals
/// the proposal, a plain validation), spelled in the owner's format, in the grid's units.
fn edit_number(field: DraftField, value: Option<DraftValue>, format: NumberFormat) -> String {
    match value {
        Some(DraftValue::Number(m)) => match cell_wire(field) {
            Some(wire) => cell_value_display(Some(m), wire, format),
            None => crate::viewmodel::format::format_amount(&m.to_string(), format),
        },
        _ => String::new(),
    }
}

/// Build the decision dialog of a pending cell, judgment, note (Story 8.5b) or study (8.7) draft. `open_study`:
/// the study the undo history belongs to now (the context line says when deciding opens another).
/// A proposed value that is no value of its field is [`Unshowable`] (no dialog with a guessed value).
pub fn dialog_view(
    d: &crate::state::DialogDraft,
    open_study: Option<Uuid>,
    format: NumberFormat,
) -> Result<DecisionView, Unshowable> {
    let record = &d.record;
    let (target, current, proposed) =
        target_and_values(record, &d.payload, d.study.as_ref(), format)?;
    let context = match (record.study_id, d.study.as_ref()) {
        (Some(id), Some(study)) if open_study != Some(id) => DECISION_CONTEXT_OTHER_STUDY
            .replace("{ticker}", &study.security_ticker)
            .replace("{dev}", &study.native_currency),
        _ => String::new(),
    };
    let (state, band) = match &d.freshness {
        DraftFreshness::Fresh => (DecisionState::Fresh, String::new()),
        DraftFreshness::Stale => (
            DecisionState::Stale,
            DECISION_BAND_STALE
                .replace("{cible}", &target)
                .replace("{maintenant}", &current),
        ),
        DraftFreshness::TargetGone(why) => (
            DecisionState::TargetGone,
            DECISION_BAND_GONE.replace("{raison}", &why.text()),
        ),
    };
    let confirm_body = if state == DecisionState::Stale {
        DECISION_CONFIRM_STALE
            .replace("{cible}", &target)
            .replace("{maintenant}", &current)
    } else {
        String::new()
    };
    let field = d
        .payload
        .target
        .as_ref()
        .and_then(DraftField::of_target)
        .map(|(f, _)| f);
    let proposed_value = match (field, d.payload.proposed_value.as_deref()) {
        (Some(f), Some(text)) => f.parse_value(text).ok(),
        _ => None,
    };
    let (edit_kind, edit_prefill, edit_options) = match field {
        _ if record.kind == DraftKind::Note => (EditKind::Note, String::new(), Vec::new()),
        Some(f) if f.unit() == steadyinvest_contract::DraftUnit::Option => (
            EditKind::Option,
            match proposed_value {
                Some(DraftValue::Option(o)) => option_label(o).to_string(),
                _ => String::new(),
            },
            f.options()
                .iter()
                .map(|o| option_label(*o).to_string())
                .collect(),
        ),
        Some(f) => (
            EditKind::Number,
            edit_number(f, proposed_value, format),
            Vec::new(),
        ),
        None => (EditKind::Number, String::new(), Vec::new()),
    };
    let (ai_client, ai_model, ai_lead, ai_text) = ai_fields(record, &d.payload, false);
    if record.kind == DraftKind::Study {
        // Story 8.7 (Decision 1): a draft study is validated through the prefilled create form
        // (« Valider… ») or rejected here; « Actuel » « — », « Proposé » its identifier. The initial
        // focus is « Annuler » — a rejection is final for a draft study.
        let ticker = record.security_ticker.trim().to_string();
        let currency = record
            .native_currency
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_string();
        let identifier = format!("{ticker} ({currency})");
        return Ok(DecisionView {
            kind: record.kind,
            target: identifier.clone(),
            context: String::new(),
            current: String::new(),
            proposed: identifier,
            state: DecisionState::Fresh,
            band: String::new(),
            confirm_body: String::new(),
            focus_cancel: true,
            edit_kind: EditKind::Number,
            edit_prefill: String::new(),
            edit_options: Vec::new(),
            submitted: date_fr(&record.created_at),
            ai_client,
            ai_model,
            ai_lead,
            ai_text,
            study_ticker: ticker,
            study_currency: currency,
        });
    }
    Ok(DecisionView {
        kind: record.kind,
        target,
        context,
        current: if record.kind == DraftKind::Note {
            String::new()
        } else {
            current
        },
        proposed: if record.kind == DraftKind::Note {
            String::new()
        } else {
            proposed
        },
        state,
        band,
        confirm_body,
        focus_cancel: d.target_validated,
        edit_kind,
        edit_prefill,
        edit_options,
        submitted: date_fr(&record.created_at),
        ai_client,
        ai_model,
        ai_lead,
        ai_text,
        study_ticker: String::new(),
        study_currency: String::new(),
    })
}

/// The option's wire name for a label the edit form's drop-down picked (the owner's edit is read by
/// `state::owner_edit`, which matches wire names). An unknown label is passed through unchanged —
/// `owner_edit` then refuses it by name.
pub fn option_wire_for_label(label: &str) -> String {
    [
        ForecastLowOption::AvgLowPeTimesEps,
        ForecastLowOption::AvgLowPriceLast5y,
        ForecastLowOption::RecentSevereLow,
        ForecastLowOption::DividendSupported,
    ]
    .into_iter()
    .find(|o| option_label(*o) == label.trim())
    .map_or_else(
        || label.to_string(),
        |o| steadyinvest_contract::option_name(o).to_string(),
    )
}

/// The pending counts the shell shows: per study (the reminder band, the Études rows' « ★ {n} »)
/// and the draft studies (the Études band), plus the total (the rail).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PendingCounts {
    pub by_study: HashMap<Uuid, usize>,
    pub draft_studies: usize,
    pub total: usize,
}

/// Drafts of an ARCHIVED study are left out (they stay listed, marked « étude archivée » — G3).
pub fn pending_counts(drafts: &[DraftRecord], archived: &HashSet<Uuid>) -> PendingCounts {
    let mut counts = PendingCounts::default();
    for d in drafts.iter().filter(|d| {
        d.status == DraftStatus::Pending && !d.study_id.is_some_and(|id| archived.contains(&id))
    }) {
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

    /// A re-read after this tick FAILED (the pragma read, then the inbox read did not): the next
    /// tick re-reads even if the version did not move — the ⊘ is never stuck (G3, arch A9).
    pub fn mark_read_failed(&mut self) {
        self.failed = true;
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

    /// The fixture with outcomes: 1 validated (edited), 2 rejected (stale at decision), 3 still
    /// pending, 4 a draft study validated into study 0xC, 5 validated then undone, 6 pending.
    fn record_fixture() -> (Vec<DraftRecord>, HashMap<Uuid, Study>) {
        let (mut drafts, mut studies) = fixture();
        let decided = |d: &mut DraftRecord, status: DraftStatus, at: &str| {
            d.status = status;
            d.decided_at = Some(Timestamp(at.to_string()));
        };
        decided(
            &mut drafts[0],
            DraftStatus::Validated,
            "2026-09-28T09:00:00Z",
        );
        drafts[0].edited_before_validation = Some(true);
        drafts[0].stale_at_decision = Some(false);
        decided(
            &mut drafts[1],
            DraftStatus::Rejected,
            "2026-09-28T10:00:00Z",
        );
        drafts[1].stale_at_decision = Some(true);
        decided(
            &mut drafts[3],
            DraftStatus::Validated,
            "2026-09-28T11:00:00Z",
        );
        drafts[3].created_study_id = Some(Uuid::from_u128(0xC));
        drafts[3].edited_before_validation = Some(false);
        decided(
            &mut drafts[4],
            DraftStatus::ValidatedUndone,
            "2026-09-28T12:00:00Z",
        );
        let c = study(0xC, "AAPL");
        studies.insert(c.id, c);
        (drafts, studies)
    }

    fn record_view(kind: KindFilter, study: Option<u128>, outcome: OutcomeFilter) -> RecordView {
        let (drafts, studies) = record_fixture();
        record_rows(
            &drafts,
            &studies,
            &HashSet::new(),
            NumberFormat::Comma,
            kind,
            study.map(Uuid::from_u128),
            outcome,
        )
        .expect("builds")
    }

    #[test]
    fn a_processed_draft_reads_as_a_star_history_entry() {
        let (drafts, _) = record_fixture();
        let summary = |n: usize| {
            history_draft(&drafts[n])
                .expect("builds")
                .map(|h| h.summary)
        };
        assert_eq!(
            summary(0).as_deref(),
            Some("★ Proposition validée (modifiée) : Ventes · 2024")
        );
        assert_eq!(
            summary(1).as_deref(),
            Some("★ Proposition rejetée : Croissance projetée des ventes")
        );
        assert_eq!(
            summary(3).as_deref(),
            Some("★ Proposition validée : Nouvelle étude : AAPL (USD)")
        );
        assert_eq!(
            summary(4).as_deref(),
            Some("★ Proposition validée puis annulée : BPA · 2019")
        );
        assert_eq!(summary(2), None, "a pending draft has no history entry");
        let mut note = drafts[2].clone();
        note.status = DraftStatus::Rejected;
        note.decided_at = Some(Timestamp("2026-09-29T08:00:00Z".to_string()));
        assert_eq!(
            history_draft(&note).unwrap().map(|h| h.summary).as_deref(),
            Some("★ Proposition rejetée : nouvelle note")
        );
    }

    #[test]
    fn the_record_lists_every_draft_newest_first_with_its_outcome() {
        let view = record_view(KindFilter::All, None, OutcomeFilter::All);
        let ids: Vec<u128> = view.rows.iter().map(|r| r.id.as_u128()).collect();
        assert_eq!(
            ids,
            vec![4, 3, 6, 5, 2, 1],
            "every draft, newest submission first"
        );
        assert_eq!(view.unfiltered, 6);
        let by_id = |n: u128| view.rows.iter().find(|r| r.id.as_u128() == n).unwrap();
        assert_eq!(by_id(1).outcome, Outcome::Validated);
        assert!(by_id(1).edited && !by_id(1).stale_at_decision);
        assert_eq!(by_id(2).outcome, Outcome::Rejected);
        assert!(by_id(2).stale_at_decision, "« périmée à la décision »");
        assert_eq!(by_id(5).outcome, Outcome::Undone);
        assert_eq!(by_id(3).outcome, Outcome::Pending);
        assert_eq!(by_id(5).decided, "28/09/2026");
        assert_eq!(by_id(3).decided, "", "pending: no decision date");
        assert_eq!(by_id(4).target, "AAPL (USD)");
        assert_eq!(
            by_id(4).ai_lead,
            "Apple Inc.",
            "the whole AI text, for « Détail »"
        );
        // A pending draft keeps its state word; a decided one has none.
        assert_eq!(by_id(6).state, RowState::Fresh);
        assert_eq!(by_id(1).state, RowState::Fresh);
    }

    #[test]
    fn the_record_filters_by_kind_study_and_outcome() {
        let ids = |v: RecordView| v.rows.iter().map(|r| r.id.as_u128()).collect::<Vec<_>>();
        assert_eq!(
            ids(record_view(KindFilter::All, None, OutcomeFilter::Validated)),
            vec![4, 1]
        );
        assert_eq!(
            ids(record_view(KindFilter::All, None, OutcomeFilter::Undone)),
            vec![5]
        );
        assert_eq!(
            ids(record_view(KindFilter::All, None, OutcomeFilter::Pending)),
            vec![3, 6]
        );
        // Decision 4: a study's record includes the draft study it was created from.
        assert_eq!(
            ids(record_view(KindFilter::All, Some(0xC), OutcomeFilter::All)),
            vec![4]
        );
        assert_eq!(
            ids(record_view(KindFilter::Studies, None, OutcomeFilter::All)),
            vec![4]
        );
        let view = record_view(KindFilter::All, None, OutcomeFilter::All);
        let labels: Vec<&str> = view
            .study_choices
            .iter()
            .map(|c| c.label.as_str())
            .collect();
        assert_eq!(labels, vec!["AAPL (CHF)", "ROG (CHF)", "NESN (CHF)"]);
        // Filters never change `unfiltered`.
        assert_eq!(
            record_view(KindFilter::Notes, None, OutcomeFilter::Rejected).unfiltered,
            6
        );
    }

    #[test]
    fn groups_newest_first_draft_studies_last_rows_newest_first() {
        let (drafts, studies) = fixture();
        let view = inbox_rows(
            &drafts,
            &studies,
            &HashSet::new(),
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
            &HashSet::new(),
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
        assert_eq!(note.ai_lead, "Texte de note");
        assert_eq!(note.ai_text, "Commentaire de l'IA");
        // A draft study: ticker + currency as app text; the name before the comment, AI side.
        let dstudy = row(4);
        assert_eq!(dstudy.target, "AAPL (USD)");
        assert_eq!(dstudy.ai_lead, "Apple Inc.");
        assert_eq!(dstudy.ai_text, "Commentaire de l'IA");
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
            &HashSet::new(),
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
            inbox_rows(
                &drafts,
                &studies,
                &HashSet::new(),
                NumberFormat::default(),
                k,
                s,
            )
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
            &HashSet::new(),
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
            &HashSet::new(),
            NumberFormat::default(),
            KindFilter::All,
            None,
        )
        .expect("builds");
        assert!(view.rows.iter().all(|r| r.id.as_u128() != 1));
        let counts = pending_counts(&drafts, &HashSet::new());
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
            &HashSet::new(),
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

    #[test]
    fn compact_ai_text_is_one_line() {
        assert_eq!(
            one_line("a\nb\r\nc\u{2028}d\u{2029}e\tf   g "),
            "a b c d e f g"
        );
        let (mut drafts, studies) = fixture();
        drafts[0].comment = "Ligne un\nligne\tdeux".into();
        drafts[0].origin_model = "opus\r\n5".into();
        let view = inbox_rows(
            &drafts,
            &studies,
            &HashSet::new(),
            NumberFormat::default(),
            KindFilter::All,
            None,
        )
        .expect("builds");
        let row = view.rows.iter().find(|r| r.id.as_u128() == 1).expect("row");
        assert_eq!(row.ai_text, "Ligne un ligne deux");
        assert_eq!(row.ai_model, "opus 5");
    }

    #[test]
    fn an_unparsable_proposed_value_makes_the_inbox_unshowable_never_proposed_dash() {
        let (mut drafts, studies) = fixture();
        let mut p: DraftPayload = serde_json::from_str(&drafts[0].payload).expect("payload");
        p.proposed_value = Some("douze".into());
        drafts[0].payload = serde_json::to_string(&p).expect("json");
        // Even when the filter would hide it: no partial list.
        let err = inbox_rows(
            &drafts,
            &studies,
            &HashSet::new(),
            NumberFormat::default(),
            KindFilter::Notes,
            None,
        )
        .expect_err("indisponible");
        assert_eq!(err.draft_id, Uuid::from_u128(1));
    }

    #[test]
    fn an_archived_studys_drafts_are_listed_marked_and_left_out_of_the_counts() {
        let (drafts, studies) = fixture();
        let archived: HashSet<Uuid> = [Uuid::from_u128(0xB)].into_iter().collect();
        let view = inbox_rows(
            &drafts,
            &studies,
            &archived,
            NumberFormat::default(),
            KindFilter::All,
            None,
        )
        .expect("builds");
        let rog: Vec<RowState> = view
            .rows
            .iter()
            .filter(|r| r.group_title == "ROG (CHF)")
            .map(|r| r.state)
            .collect();
        assert_eq!(rog, vec![RowState::Archived; 3]);
        assert_eq!(view.unfiltered, 6, "still listed");
        let counts = pending_counts(&drafts, &archived);
        assert_eq!(counts.total, 3);
        assert_eq!(counts.by_study.get(&Uuid::from_u128(0xB)), None);
    }

    #[test]
    fn the_unfiltered_count_tells_an_empty_inbox_from_an_empty_filter() {
        let (drafts, studies) = fixture();
        let view = inbox_rows(
            &drafts,
            &studies,
            &HashSet::new(),
            NumberFormat::default(),
            KindFilter::Notes,
            Some(Uuid::from_u128(0xA)),
        )
        .expect("builds");
        assert!(view.rows.is_empty());
        assert_eq!(view.unfiltered, 6);
        let empty = inbox_rows(
            &[],
            &studies,
            &HashSet::new(),
            NumberFormat::default(),
            KindFilter::All,
            None,
        )
        .expect("builds");
        assert_eq!(empty.unfiltered, 0);
    }

    #[test]
    fn a_study_gone_shows_its_ticker_only_when_it_reads_as_one() {
        let (mut drafts, mut studies) = fixture();
        studies.remove(&Uuid::from_u128(0xA));
        drafts[0].security_ticker = " nesn.sw ".into();
        drafts[1].security_ticker = "texte libre\nde l'IA".into();
        let view = inbox_rows(
            &drafts,
            &studies,
            &HashSet::new(),
            NumberFormat::default(),
            KindFilter::All,
            None,
        )
        .expect("builds");
        let t = |n: u128| {
            view.rows
                .iter()
                .find(|r| r.id.as_u128() == n)
                .expect("row")
                .target
                .clone()
        };
        assert!(t(1).starts_with("NESN.SW · "), "{}", t(1));
        assert!(t(2).starts_with("— · "), "{}", t(2));
    }

    #[test]
    fn a_failed_reread_is_retried_on_the_next_tick() {
        let mut p = PollState::default();
        assert_eq!(p.on_tick(Ok(Some(5))), PollAction::Reread);
        // The inbox read after that tick failed: the version did not move, the tick re-reads.
        p.mark_read_failed();
        assert_eq!(p.on_tick(Ok(Some(5))), PollAction::Reread);
        assert_eq!(p.on_tick(Ok(Some(5))), PollAction::Nothing, "recovered");
    }

    // ── Story 8.5b — the decision dialog ──

    fn dialog_of(
        record: DraftRecord,
        study: Option<Study>,
        freshness: DraftFreshness,
        validated: bool,
    ) -> crate::state::DialogDraft {
        let payload: DraftPayload = serde_json::from_str(&record.payload).expect("payload");
        crate::state::DialogDraft {
            draft: crate::state::DraftRef {
                draft_id: record.id,
                study_id: record.study_id,
                ticker: record.security_ticker.clone(),
            },
            payload,
            study,
            archived: false,
            freshness,
            target_validated: validated,
            seen_fingerprint: None,
            record,
        }
    }

    #[test]
    fn a_cell_dialog_prefills_in_the_grids_millions_and_the_owners_format() {
        let (drafts, studies) = fixture();
        let a = studies.get(&Uuid::from_u128(0xA)).cloned();
        let mut sales = drafts[0].clone();
        let mut p: DraftPayload = serde_json::from_str(&sales.payload).expect("payload");
        p.proposed_value = Some("1234500000".into());
        sales.payload = serde_json::to_string(&p).expect("json");
        let d = dialog_of(sales, a, DraftFreshness::Fresh, false);
        let comma = NumberFormat::parse("comma").unwrap_or_default();
        let v = dialog_view(&d, Some(Uuid::from_u128(0xA)), comma).expect("view");
        assert_eq!(v.edit_kind, EditKind::Number);
        assert_eq!(
            v.edit_prefill,
            cell_value_display(Some(money("1234500000")), entry::FIELD_SALES, comma)
        );
        assert_eq!(v.proposed, v.edit_prefill, "shown like the grid");
        // The prefill reads back as the proposal (an untouched edit is a plain validation).
        let back = crate::state::owner_edit(d.record.kind, &d.payload, &v.edit_prefill, comma)
            .expect("reads back");
        assert_eq!(back, crate::state::EditedValue::Number(money("1234500000")));
        assert_eq!(v.state, DecisionState::Fresh);
        assert!(v.band.is_empty() && v.confirm_body.is_empty());
        assert!(v.context.is_empty(), "its study is the open one");
        assert!(!v.focus_cancel, "Q5: « Valider » focused");
        // AI text is whole in the dialog (never one-lined).
        assert_eq!(v.ai_text, "Commentaire de l'IA");
    }

    #[test]
    fn another_studys_draft_says_that_deciding_opens_it_and_a_validated_target_focuses_cancel() {
        let (drafts, studies) = fixture();
        let a = studies.get(&Uuid::from_u128(0xA)).cloned();
        let d = dialog_of(drafts[0].clone(), a, DraftFreshness::Fresh, true);
        let v = dialog_view(&d, Some(Uuid::from_u128(0xB)), NumberFormat::default()).expect("view");
        assert_eq!(v.context, "Valider ou rejeter ouvre l'étude NESN (CHF).");
        assert!(v.focus_cancel, "Q5: a ✓ target focuses « Annuler »");
        let none = dialog_view(&d, None, NumberFormat::default()).expect("view");
        assert!(
            !none.context.is_empty(),
            "no open study: deciding opens it too"
        );
    }

    #[test]
    fn stale_and_gone_drafts_carry_their_bands_and_the_confirmation_body() {
        let (drafts, studies) = fixture();
        let a = studies.get(&Uuid::from_u128(0xA)).cloned();
        let stale = dialog_of(drafts[1].clone(), a, DraftFreshness::Stale, false);
        let v =
            dialog_view(&stale, Some(Uuid::from_u128(0xA)), NumberFormat::default()).expect("view");
        assert_eq!(v.state, DecisionState::Stale);
        assert!(v.band.starts_with("La cible NESN · "), "{}", v.band);
        assert!(
            v.band
                .contains(&format!("Valeur actuelle : {}.", v.current))
        );
        assert!(v.confirm_body.contains("La valeur proposée la remplacera."));
        let b = studies.get(&Uuid::from_u128(0xB)).cloned();
        let gone = dialog_of(
            drafts[4].clone(),
            b,
            DraftFreshness::TargetGone(crate::state::GoneReason::YearRemoved(2019)),
            false,
        );
        let v =
            dialog_view(&gone, Some(Uuid::from_u128(0xB)), NumberFormat::default()).expect("view");
        assert_eq!(v.state, DecisionState::TargetGone);
        assert_eq!(
            v.band,
            "Cible disparue : l'année 2019 n'existe plus dans l'étude ; la proposition ne peut qu'être rejetée."
        );
        assert!(v.confirm_body.is_empty());
    }

    #[test]
    fn an_option_dialog_lists_the_chip_labels_and_maps_a_label_back_to_its_wire_name() {
        let (drafts, studies) = fixture();
        let b = studies.get(&Uuid::from_u128(0xB)).cloned();
        let d = dialog_of(drafts[5].clone(), b, DraftFreshness::Fresh, false);
        let v = dialog_view(&d, Some(Uuid::from_u128(0xB)), NumberFormat::default()).expect("view");
        assert_eq!(v.edit_kind, EditKind::Option);
        assert_eq!(v.edit_prefill, OPT_RECENT_SEVERE_LOW);
        assert_eq!(v.edit_options.len(), 4);
        assert!(v.edit_options.contains(&OPT_DIVIDEND_SUPPORTED.to_string()));
        assert_eq!(
            option_wire_for_label(OPT_DIVIDEND_SUPPORTED),
            steadyinvest_contract::option_name(ForecastLowOption::DividendSupported)
        );
        assert_eq!(option_wire_for_label("inconnu"), "inconnu");
    }

    #[test]
    fn a_note_dialog_shows_the_note_whole_in_the_ai_fields_and_no_values() {
        let (mut drafts, studies) = fixture();
        let mut p: DraftPayload = serde_json::from_str(&drafts[2].payload).expect("payload");
        p.note_text = Some("Ligne un\nligne deux".into());
        drafts[2].payload = serde_json::to_string(&p).expect("json");
        let b = studies.get(&Uuid::from_u128(0xB)).cloned();
        let d = dialog_of(drafts[2].clone(), b, DraftFreshness::Fresh, false);
        let v = dialog_view(&d, Some(Uuid::from_u128(0xB)), NumberFormat::default()).expect("view");
        assert_eq!(v.edit_kind, EditKind::Note);
        assert!(v.current.is_empty() && v.proposed.is_empty());
        assert_eq!(
            v.ai_lead, "Ligne un\nligne deux",
            "line breaks kept in the dialog"
        );
        assert!(
            v.edit_prefill.is_empty(),
            "a note's prefill is AI text (ai_lead)"
        );
    }

    #[test]
    fn a_proposed_figure_takes_the_current_ones_decimals() {
        let comma = NumberFormat::Comma;
        assert_eq!(same_decimals("2,10", "2,5", comma), "2,50");
        assert_eq!(same_decimals("2,10", "3", comma), "3,00");
        assert_eq!(same_decimals("2,1", "2,555", comma), "2,555");
        assert_eq!(
            same_decimals("1\u{a0}234,5", "1\u{a0}300", comma),
            "1\u{a0}300,0"
        );
        assert_eq!(same_decimals("—", "2,5", comma), "2,5");
        assert_eq!(
            same_decimals("PER bas × BPA bas", "Plus bas sévère récent", comma),
            "Plus bas sévère récent"
        );
        assert_eq!(same_decimals("2.10", "2.5", NumberFormat::Point), "2.50");
    }
}
