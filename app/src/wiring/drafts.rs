//! The draft inbox « Propositions », read side (Story 8.5a — FR73, arch A9; UX spec §5.1–5.4).
//!
//! - [`push_drafts`] reads the drafts and their studies through the app's own journal (never
//!   `McpAccess`), builds the view model and sets the `Drafts` global, the rail count, the open
//!   study's reminder and the Études rows' « ★ {n} ». ANY read failure shows « indisponible » with
//!   its cause — rows cleared, counts zero, the rail « · ⊘ » — never an empty-looking inbox, never
//!   the last good count.
//! - The poller ([`start_poller`]) reads `PRAGMA data_version` every 2.5 s on the app's own
//!   connection: it moves only when ANOTHER connection commits (the MCP server's inserts), so a
//!   draft arriving while the app is open shows within ~3 s. The app's own draft-affecting writes
//!   (a study delete's cascade, an import, a restore, a dossier switch) all end in
//!   `studies::refresh_studies`, which pushes the inbox explicitly.
//! - There is no window-focus trigger: Slint 1.17 has no public window-activation callback (story
//!   decision 1); the timer bounds the latency.
//!
//! - Deciding (Story 8.5b): a row opens the decision dialog ([`open_decision`], read afresh — never
//!   the row's cached text); « Valider » / « Rejeter » / the edit form's « Enregistrer » /
//!   the stale confirmation's verb decide through `JournalState::decide_draft`, after opening the
//!   draft's study when it is not the open one (the undo history belongs to the open study, arch
//!   A8). Every decision, undo and redo pushes the inbox explicitly: the app's own writes never move
//!   `PRAGMA data_version`.
//!
//! This is the ONLY place the `DraftRow.ai_*` and `DraftDecision.ai_*` fields are set: AI-written
//! text enters no other model (the guard test in `posture.rs`).

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use steadyinvest_contract::DraftKind;
use uuid::Uuid;

use crate::state::{
    self, Decision, DialogDraft, DraftFreshness, JournalState, MSG_DRAFT_REJECTED,
    MSG_DRAFT_VALIDATED, MSG_DRAFT_VALIDATED_EDITED, MSG_READ_FAILED, owner_edit,
};
use crate::viewmodel::ai_lines::{
    AiJudgmentsView, AiLineView, JudgmentOverlay, ai_judgments, pending_judgment_overlays,
};
use crate::viewmodel::drafts::{
    DecisionView, INBOX_CAUSE_UNREADABLE_DRAFT, InboxRow, InboxView, KindFilter, OutcomeFilter,
    PendingCounts, PollAction, PollState, RecordView, STUDY_FILTER_ALL, StudyChoice, dialog_view,
    inbox_rows, option_wire_for_label, pending_counts, record_rows,
};
use crate::viewmodel::engine::StudyFrame;
use crate::viewmodel::format::NumberFormat;
use crate::wiring::Session;
use crate::wiring::dialog;
use crate::wiring::push::push_form;
use crate::{
    AiChip, AiJudgments, AiLine, Dialog, DraftDecision, DraftRow, Drafts, MainWindow, Prefs,
    RecordRow, Studies,
};

/// The poll period (arch A9: new drafts within ~3 s).
const POLL_PERIOD_MS: u64 = 2500;

/// The inbox's session memory: the poller's state, the last counts (applied to rows rebuilt
/// elsewhere), the « Étude : » choices and pick, and the open study.
#[derive(Default)]
struct InboxCache {
    poll: PollState,
    counts: PendingCounts,
    failed: bool,
    choices: Vec<StudyChoice>,
    study_filter: Option<Uuid>,
    open_study: Option<Uuid>,
    /// The rows and « Étude : » labels last pushed — an unchanged re-read rebuilds no model (G3).
    shown: Option<(Vec<InboxRow>, Vec<String>)>,
    /// Story 8.6: the pending JUDGMENT drafts of every study (from the last read — never a query per
    /// edit), for the open study's AI lines and chips.
    judgments: Vec<steadyinvest_persistence::DraftRecord>,
    /// The open study's pending judgment drafts as last drawn (ids + payloads): a read that changes
    /// them re-draws the open study's overlay.
    drawn: Vec<(Uuid, String)>,
    /// Story 8.7: the « Étude : » choices of the Registre (every study some draft is about or
    /// became) — the drop-down lists the active view's.
    record_choices: Vec<StudyChoice>,
}

thread_local! {
    static INBOX: RefCell<InboxCache> = RefCell::new(InboxCache::default());
}

fn kind_wire(kind: DraftKind) -> &'static str {
    match kind {
        DraftKind::Study => "study",
        DraftKind::Note => "note",
        DraftKind::Cell => "cell",
        DraftKind::Judgment => "judgment",
    }
}

fn to_slint(view: &InboxView) -> Vec<DraftRow> {
    view.rows
        .iter()
        .map(|r| DraftRow {
            id: r.id.to_string().into(),
            kind: kind_wire(r.kind).into(),
            group_start: r.group_start,
            group_title: r.group_title.clone().into(),
            group_is_studies: r.group_is_studies,
            target: r.target.clone().into(),
            current: r.current.clone().into(),
            proposed: r.proposed.clone().into(),
            state: r.state.wire(),
            submitted: r.submitted.clone().into(),
            // AI-written: read in Slint ONLY inside an AiFrame (UX spec §4.1).
            ai_client: r.ai_client.clone().into(),
            ai_model: r.ai_model.clone().into(),
            ai_lead: r.ai_lead.clone().into(),
            ai_text: r.ai_text.clone().into(),
        })
        .collect()
}

/// The decision dialog's draft, as Slint shows it — the ONE setter of `DraftDecision.ai_*`.
fn to_dialog(v: &DecisionView, id: Uuid, live: bool) -> DraftDecision {
    let options: Vec<SharedString> = v.edit_options.iter().map(|o| o.as_str().into()).collect();
    DraftDecision {
        id: id.to_string().into(),
        kind: kind_wire(v.kind).into(),
        target: v.target.clone().into(),
        context: v.context.clone().into(),
        current: v.current.clone().into(),
        proposed: v.proposed.clone().into(),
        state: v.state.wire(),
        band: v.band.clone().into(),
        focus_cancel: v.focus_cancel,
        edit_kind: v.edit_kind.wire().into(),
        edit_prefill: v.edit_prefill.clone().into(),
        edit_options: ModelRc::new(VecModel::from(options)),
        live,
        submitted: v.submitted.clone().into(),
        // AI-written: read in Slint ONLY inside an AiFrame (UX spec §4.1).
        ai_client: v.ai_client.clone().into(),
        ai_model: v.ai_model.clone().into(),
        ai_lead: v.ai_lead.clone().into(),
        ai_text: v.ai_text.clone().into(),
        study_ticker: v.study_ticker.clone().into(),
        study_currency: v.study_currency.clone().into(),
    }
}

/// The draft the decision dialog shows, and what a stale confirmation will decide.
struct OpenDecision {
    draft: DialogDraft,
    confirm_body: String,
    /// Set when « Valider » / « Enregistrer » raised the stale confirmation: the edited text
    /// (`None` = validate as proposed).
    stale_edit: Option<Option<String>>,
}

thread_local! {
    static DECISION: RefCell<Option<OpenDecision>> = const { RefCell::new(None) };
}

/// Read a pending draft and show its decision dialog (Story 8.5b). `field_error`: a refusal to keep
/// shown (a re-read after a refused decision). Returns the reason when the draft cannot be shown.
fn show_decision(
    ui: &MainWindow,
    state: &JournalState,
    id: Uuid,
    field_error: &str,
) -> Result<(), String> {
    let draft = state.draft_for_dialog(id)?;
    if draft.archived {
        // Archived since the inbox was read (its rows are not activatable): refused by name.
        return Err(state::MSG_DECISION_STUDY_ARCHIVED.replace("{ticker}", &draft.draft.ticker));
    }
    let view =
        dialog_view(&draft, state.decision_study(), number_format(ui)).map_err(|problem| {
            tracing::warn!(
                "draft {} cannot be shown in the decision dialog: {}",
                problem.draft_id,
                problem.detail
            );
            MSG_READ_FAILED.to_string()
        })?;
    let dialog = ui.global::<Dialog>();
    dialog.set_decision(to_dialog(&view, id, true));
    dialog.set_title(SharedString::new());
    dialog.set_body(SharedString::new());
    dialog.set_context(SharedString::new());
    dialog.set_action(SharedString::new());
    dialog.set_verb(SharedString::new());
    dialog.set_form_id(SharedString::new());
    dialog.set_field_error(field_error.into());
    dialog.set_kind("decision".into());
    DECISION.with(|d| {
        *d.borrow_mut() = Some(OpenDecision {
            confirm_body: view.confirm_body.clone(),
            draft,
            stale_edit: None,
        })
    });
    Ok(())
}

/// Whether `id` is a pending judgment draft of the inbox's last read — i.e. whether its chip is
/// still drawn on the open study (Story 8.6: the focus returns to it, else to the study).
pub(crate) fn is_pending_judgment(id: &str) -> bool {
    INBOX.with(|cache| {
        cache
            .borrow()
            .judgments
            .iter()
            .any(|d| d.id.to_string() == id)
    })
}

/// A row's activation (Story 8.5b): the decision dialog of that draft, or « Action refusée » with
/// the reason (the draft gone or decided meanwhile, a read failure) and the inbox re-read.
pub(crate) fn open_decision(ui: &MainWindow, state: &JournalState, id: Uuid) {
    ui.global::<Drafts>().set_notice(SharedString::new());
    ui.global::<Drafts>().set_return_row(id.to_string().into());
    if let Err(message) = show_decision(ui, state, id, "") {
        dialog::refuse(ui, &message);
        push_drafts(ui, state);
    }
}

/// After a refused decision: show the draft again with the refusal (its values re-read), or — when
/// it can no longer be decided (gone, decided elsewhere) — keep the refusal and only « Annuler ».
fn after_refusal(ui: &MainWindow, state: &JournalState, id: Uuid) {
    let dialog = ui.global::<Dialog>();
    let error = dialog.get_field_error().to_string();
    // The refusal landed inline in the decision OR in its edit form (G3): either way the draft is
    // read afresh — its freshness, the fingerprint shown, whether it is still decidable — and the
    // decision view shows again with the refusal, never the stale edit form looping on it.
    let kind = dialog.get_kind();
    let in_edit = kind.as_str() == "form" && dialog.get_form_id().as_str() == "draft-edit";
    if (kind.as_str() == "decision" || in_edit) && show_decision(ui, state, id, &error).is_err() {
        dialog.set_form_id(SharedString::new());
        dialog.set_title(SharedString::new());
        dialog.set_body(SharedString::new());
        dialog.set_field_error(error.as_str().into());
        dialog.set_kind("decision".into());
        let mut shown = dialog.get_decision();
        shown.live = false;
        dialog.set_decision(shown);
        DECISION.with(|d| *d.borrow_mut() = None);
    }
    push_drafts(ui, state);
}

/// Decide the open draft (Story 8.5b): the checks that need no open study run first (G3 — a refused
/// decision never switches the open study for nothing); then its study is opened when it is not
/// the open one (arch A8), the draft decided, the outcome said, the open study re-rendered and the
/// inbox re-read. A decision refused AFTER that open (a race) reopens the previous study with its
/// undo history. `false` on a refusal (raised inline — the dialog's own gesture).
fn decide_open(ui: &MainWindow, s: &Rc<RefCell<JournalState>>, decision: Decision) -> bool {
    let Some(draft) = DECISION.with(|d| d.borrow().as_ref().map(|o| o.draft.clone())) else {
        return false;
    };
    let id = draft.draft.draft_id;
    let precheck = s.borrow().precheck_decision(&draft.draft, &decision);
    if let Err(message) = precheck {
        dialog::refuse(ui, &message);
        after_refusal(ui, &s.borrow(), id);
        return false;
    }
    let previous = s.borrow().decision_study();
    let mut switched: Option<state::UndoHistory> = None;
    if let Some(study_id) = draft.draft.study_id
        && previous != Some(study_id)
    {
        switched = Some(s.borrow_mut().take_undo());
        // The ordinary open path: undo history reset to that study, form pushed; the screen stays.
        ui.global::<Studies>()
            .invoke_open_study(study_id.to_string().into());
    }
    let rejected = decision == Decision::Reject;
    let result = s.borrow_mut().decide_draft(&draft.draft, decision);
    match result {
        Ok(()) => {
            let text = if rejected {
                MSG_DRAFT_REJECTED
            } else if s.borrow().draft_was_edited(id) {
                MSG_DRAFT_VALIDATED_EDITED
            } else {
                MSG_DRAFT_VALIDATED
            };
            ui.global::<Drafts>().set_notice(text.into());
            DECISION.with(|d| *d.borrow_mut() = None);
            if let Some(study_id) = draft.draft.study_id
                && s.borrow().decision_study() == Some(study_id)
            {
                let reread = s.borrow().get_study(study_id);
                if let Some(study) = reread {
                    push_form(ui, &s.borrow(), &study, number_format(ui));
                }
            }
            push_drafts(ui, &s.borrow());
            true
        }
        Err(message) => {
            if let Some(history) = switched {
                // Nothing was written: back to the study that was open, with its history.
                match previous {
                    Some(prev) => {
                        ui.global::<Studies>()
                            .invoke_open_study(prev.to_string().into());
                        s.borrow_mut().put_back_undo(history);
                    }
                    None => ui.global::<Studies>().invoke_close_study(),
                }
            }
            dialog::refuse(ui, &message);
            after_refusal(ui, &s.borrow(), id);
            false
        }
    }
}

/// Raise the stale confirmation over the decision (or its edit form): its « Annuler » goes back.
fn raise_stale_confirm(ui: &MainWindow, edit: Option<String>) {
    let body = DECISION.with(|d| {
        let mut d = d.borrow_mut();
        let open = d.as_mut()?;
        open.stale_edit = Some(edit);
        Some(open.confirm_body.clone())
    });
    let Some(body) = body else {
        return;
    };
    let dialog = ui.global::<Dialog>();
    dialog.set_resume_title(dialog.get_title());
    dialog.set_resume_body(dialog.get_body());
    dialog.set_title(SharedString::new());
    dialog.set_verb(SharedString::new());
    dialog.set_body(body.into());
    dialog.set_action("validate-stale".into());
    dialog.set_kind("confirm".into());
}

/// The owner's edit read under their number format and the grid's units (8.2b `owner_edit`): an
/// option LABEL picked in the drop-down is mapped back to its wire name first.
fn read_edit(
    ui: &MainWindow,
    draft: &DialogDraft,
    text: &str,
) -> Result<state::EditedValue, String> {
    let is_option = draft
        .payload
        .target
        .as_ref()
        .and_then(steadyinvest_contract::DraftField::of_target)
        .is_some_and(|(f, _)| f.unit() == steadyinvest_contract::DraftUnit::Option);
    let text = if is_option {
        option_wire_for_label(text)
    } else {
        text.to_string()
    };
    owner_edit(draft.record.kind, &draft.payload, &text, number_format(ui))
}

fn number_format(ui: &MainWindow) -> NumberFormat {
    NumberFormat::parse(&ui.global::<Prefs>().get_number_format()).unwrap_or_default()
}

/// Set « ★ {n} » on the Études list rows from the last counts (in place — the rows' other fields
/// are untouched). Called after every inbox read and after `refresh_studies` rebuilt the rows.
pub(crate) fn apply_row_counts(ui: &MainWindow) {
    let rows = ui.global::<Studies>().get_rows();
    INBOX.with(|cache| {
        let cache = cache.borrow();
        for i in 0..rows.row_count() {
            let Some(mut row) = rows.row_data(i) else {
                continue;
            };
            let n = Uuid::parse_str(row.id.as_str())
                .ok()
                .and_then(|id| cache.counts.by_study.get(&id).copied())
                .unwrap_or(0) as i32;
            if row.pending_drafts != n {
                row.pending_drafts = n;
                rows.set_row_data(i, row);
            }
        }
    });
}

/// The open study's reminder (the ★ band): its pending count from the last read. Called when a
/// study is shown (`push::push_form`) and after every read.
pub(crate) fn apply_open_study(ui: &MainWindow, study_id: Option<Uuid>) {
    let n = INBOX.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.open_study = study_id;
        study_id
            .and_then(|id| cache.counts.by_study.get(&id).copied())
            .unwrap_or(0)
    });
    ui.global::<Drafts>().set_open_study_pending(n as i32);
}

/// A pending AI line as Slint draws it (Story 8.6) — `visible: false` when there is none.
fn ai_line(v: &Option<AiLineView>) -> AiLine {
    match v {
        Some(l) => AiLine {
            visible: true,
            commands: l.geometry.commands.as_str().into(),
            marker: l.geometry.marker.as_str().into(),
            y: l.geometry.y,
            value: l.value.as_str().into(),
        },
        None => AiLine {
            y: -1.0,
            ..Default::default()
        },
    }
}

/// One action chip (Story 8.6): app text only — the field label and the formatted proposal.
fn ai_chip(o: &JudgmentOverlay) -> AiChip {
    AiChip {
        draft_id: o.draft_id.to_string().into(),
        field: o.field_label.as_str().into(),
        value: o.value_label.as_str().into(),
        stale: o.stale,
    }
}

thread_local! {
    /// The §1 / §3 chip rows' models, kept for the app's life and synced in place (G3): a redraw
    /// (a proposal arriving on another field, a re-read) then keeps the chip instances — a new
    /// model would recreate them and drop the keyboard focus a chip holds.
    static CHIP_ROWS: (Rc<VecModel<AiChip>>, Rc<VecModel<AiChip>>) =
        (Rc::new(VecModel::default()), Rc::new(VecModel::default()));
}

/// Make `model` hold `rows`, touching only the rows that differ.
fn sync_rows(model: &VecModel<AiChip>, rows: Vec<AiChip>) {
    let keep = rows.len().min(model.row_count());
    for (i, row) in rows.iter().enumerate().take(keep) {
        if model.row_data(i).as_ref() != Some(row) {
            model.set_row_data(i, row.clone());
        }
    }
    while model.row_count() > rows.len() {
        model.remove(model.row_count() - 1);
    }
    for row in rows.into_iter().skip(keep) {
        model.push(row);
    }
}

/// The open study's AI overlay as the `Studies.draft-judgments` struct (Story 8.6).
fn to_ai_slint(v: &AiJudgmentsView) -> AiJudgments {
    use steadyinvest_contract::DraftField as F;
    let (growth_chips, pe_chips) = CHIP_ROWS.with(|(growth, pe)| {
        sync_rows(growth, v.growth_chips.iter().map(ai_chip).collect());
        sync_rows(pe, v.pe_chips.iter().map(ai_chip).collect());
        (ModelRc::from(growth.clone()), ModelRc::from(pe.clone()))
    });
    let mut out = AiJudgments {
        est_high: ai_line(&v.est_high),
        est_low: ai_line(&v.est_low),
        pe_high: ai_line(&v.pe_high),
        pe_low: ai_line(&v.pe_low),
        growth_chips,
        pe_chips,
        ..Default::default()
    };
    for o in &v.field_chips {
        let chip = ai_chip(o);
        match o.field {
            F::ProjectedSalesGrowthPct => out.chip_sales_growth = chip,
            F::ProjectedEpsGrowthPct => out.chip_eps_growth = chip,
            F::EstimatedHighEps => out.chip_est_high = chip,
            F::EstimatedLowEps => out.chip_est_low = chip,
            F::JudgedAvgHighPe => out.chip_high_pe = chip,
            F::JudgedAvgLowPe => out.chip_low_pe = chip,
            F::RecentSevereLow => out.chip_severe_low = chip,
            F::PresentFullYearDividend => out.chip_dividend = chip,
            F::ForecastLowOption => out.chip_option = chip,
            _ => {}
        }
    }
    for (field, date) in &v.captions {
        let date: SharedString = date.as_str().into();
        match field {
            F::ProjectedSalesGrowthPct => out.cap_sales_growth = date,
            F::ProjectedEpsGrowthPct => out.cap_eps_growth = date,
            F::EstimatedHighEps => out.cap_est_high = date,
            F::EstimatedLowEps => out.cap_est_low = date,
            F::JudgedAvgHighPe => out.cap_high_pe = date,
            F::JudgedAvgLowPe => out.cap_low_pe = date,
            F::RecentSevereLow => out.cap_severe_low = date,
            F::PresentFullYearDividend => out.cap_dividend = date,
            F::ForecastLowOption => out.cap_option = date,
            _ => {}
        }
    }
    out
}

/// A study's pending judgment drafts as a comparable signature (ids + payloads, sorted).
fn drawn_signature<'a>(
    study_id: Uuid,
    drafts: impl Iterator<Item = &'a steadyinvest_persistence::DraftRecord>,
) -> Vec<(Uuid, String)> {
    let mut sig: Vec<(Uuid, String)> = drafts
        .filter(|d| d.study_id == Some(study_id))
        .map(|d| (d.id, d.payload.clone()))
        .collect();
    sig.sort();
    sig
}

/// Draw the open study's pending AI judgment lines, chips and captions (Story 8.6) — from the
/// inbox's last read (no query) and the study as just rendered. Called by `push::push_form` after
/// the owner charts, and when a read changes the open study's pending judgment drafts. Read-only:
/// the owner chart state is never touched (AC 4).
pub(crate) fn push_ai_judgments(
    ui: &MainWindow,
    study: &steadyinvest_contract::Study,
    frame: Option<&StudyFrame>,
    format: NumberFormat,
) {
    let mine: Vec<steadyinvest_persistence::DraftRecord> = INBOX.with(|cache| {
        let mut cache = cache.borrow_mut();
        let mine: Vec<_> = cache
            .judgments
            .iter()
            .filter(|d| d.study_id == Some(study.id))
            .cloned()
            .collect();
        cache.drawn = drawn_signature(study.id, mine.iter());
        mine
    });
    let overlays = pending_judgment_overlays(study, &mine, format);
    let view = ai_judgments(study, frame, &overlays, format);
    ui.global::<Studies>()
        .set_draft_judgments(to_ai_slint(&view));
}

/// Clear the AI overlay (a study closed, a dossier switch) — the next open draws it afresh.
pub(crate) fn clear_ai_judgments(ui: &MainWindow) {
    INBOX.with(|cache| cache.borrow_mut().drawn.clear());
    ui.global::<Studies>()
        .set_draft_judgments(AiJudgments::default());
}

/// After an inbox read: re-draw the open study's AI overlay when its pending judgment drafts
/// changed (a draft arrived through MCP, was decided, undone or redone — AC 9).
fn redraw_open_if_changed(ui: &MainWindow, state: &JournalState) {
    let target = INBOX.with(|cache| {
        let cache = cache.borrow();
        let id = cache.open_study?;
        (drawn_signature(id, cache.judgments.iter()) != cache.drawn).then_some(id)
    });
    let Some(id) = target else {
        return;
    };
    // A closed study is drawn afresh when it is opened again.
    if !ui.global::<Studies>().get_study_open() {
        return;
    }
    let Some(study) = state.get_study(id) else {
        return;
    };
    let frame = crate::viewmodel::engine::build_frame(&study).ok();
    push_ai_judgments(ui, &study, frame.as_ref(), number_format(ui));
}

/// Show « indisponible »: rows and « Étude : » choices cleared, counts zero, the cause named (""
/// when none can be). The poller then re-reads on its next tick even if nothing moved — a ⊘ is
/// never stuck (G3, arch A9).
fn show_failure(ui: &MainWindow, cause: &str) {
    INBOX.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.counts = PendingCounts::default();
        cache.failed = true;
        cache.choices.clear();
        cache.shown = None;
        // Story 8.6 (G3): the last successful read's proposals stay drawn — a failed poll makes
        // nothing vanish (UX §8; the study band says the read failed). Deciding one re-reads it.
        cache.poll.mark_read_failed();
    });
    let drafts = ui.global::<Drafts>();
    drafts.set_rows(ModelRc::new(VecModel::from(Vec::<DraftRow>::new())));
    drafts.set_study_options(ModelRc::new(VecModel::from(vec![SharedString::from(
        STUDY_FILTER_ALL,
    )])));
    drafts.set_study_value(STUDY_FILTER_ALL.into());
    drafts.set_pending_count(0);
    drafts.set_draft_study_count(0);
    drafts.set_unfiltered_count(0);
    drafts.set_open_study_pending(0);
    drafts.set_failure_cause(cause.into());
    drafts.set_read_failed(true);
    apply_row_counts(ui);
}

/// Read the inbox and push it (see the module doc). Safe to call at any time.
pub(crate) fn push_drafts(ui: &MainWindow, state: &JournalState) {
    let drafts_global = ui.global::<Drafts>();
    drafts_global.set_read_only(state.is_read_only());
    let data = match state.read_inbox() {
        Ok(data) => data,
        Err(error) => {
            show_failure(ui, error.cause.unwrap_or(""));
            redraw_open_if_changed(ui, state);
            return;
        }
    };
    let (drafts, studies, archived) = match data {
        Some(d) => (d.drafts, d.studies, d.archived),
        // No dossier open: a true absence — an empty inbox, no ⊘.
        None => Default::default(),
    };
    let counts = pending_counts(&drafts, &archived);
    let kind = KindFilter::from_wire(drafts_global.get_kind_filter().as_str());
    let inbox_view = drafts_global.get_view() != "record";
    // The « Étude : » pick survives a re-read while that study still has pending drafts — in
    // « À traiter »; the Registre keeps it (its study may have decided drafts only, Story 8.7).
    let picked = INBOX.with(|c| c.borrow().study_filter);
    let study_filter = picked.filter(|id| drafts.iter().any(|d| d.study_id == Some(*id)));
    let view: InboxView = match inbox_rows(
        &drafts,
        &studies,
        &archived,
        number_format(ui),
        kind,
        study_filter,
    ) {
        Ok(view) => view,
        Err(problem) => {
            tracing::warn!(
                "draft {} cannot be shown in the inbox: {}",
                problem.draft_id,
                problem.detail
            );
            // The read itself succeeded: the open study's proposals follow it (one unreadable
            // draft of any study must not freeze or hide the others).
            INBOX.with(|cache| {
                cache.borrow_mut().judgments = drafts
                    .iter()
                    .filter(|d| d.kind == DraftKind::Judgment)
                    .cloned()
                    .collect();
            });
            show_failure(ui, INBOX_CAUSE_UNREADABLE_DRAFT);
            redraw_open_if_changed(ui, state);
            return;
        }
    };
    let labels: Vec<String> = std::iter::once(STUDY_FILTER_ALL.to_string())
        .chain(view.study_choices.iter().map(|c| c.label.clone()))
        .collect();
    let (open_study, total, draft_studies, unchanged) = INBOX.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.failed = false;
        cache.judgments = drafts
            .iter()
            .filter(|d| d.kind == DraftKind::Judgment)
            .cloned()
            .collect();
        cache.choices = view.study_choices.clone();
        cache.study_filter = if inbox_view { study_filter } else { picked };
        cache.counts = counts;
        let unchanged = cache
            .shown
            .as_ref()
            .is_some_and(|(rows, l)| *rows == view.rows && *l == labels);
        if !unchanged {
            cache.shown = Some((view.rows.clone(), labels.clone()));
        }
        (
            cache.open_study,
            cache.counts.total,
            cache.counts.draft_studies,
            unchanged,
        )
    });
    // An unchanged re-read rebuilds no model (G3): the rows' focus and scroll stay put.
    if !unchanged {
        if inbox_view {
            let options: Vec<SharedString> = labels.iter().map(|l| l.as_str().into()).collect();
            drafts_global.set_study_options(ModelRc::new(VecModel::from(options)));
        }
        drafts_global.set_rows(ModelRc::new(VecModel::from(to_slint(&view))));
    }
    if inbox_view {
        let value = study_filter
            .and_then(|id| view.study_choices.iter().find(|c| c.study_id == id))
            .map_or(STUDY_FILTER_ALL, |c| c.label.as_str())
            .to_string();
        drafts_global.set_study_value(value.into());
    }
    drafts_global.set_pending_count(total as i32);
    drafts_global.set_draft_study_count(draft_studies as i32);
    drafts_global.set_unfiltered_count(view.unfiltered as i32);
    drafts_global.set_failure_cause(SharedString::new());
    drafts_global.set_read_failed(false);
    apply_open_study(ui, open_study);
    apply_row_counts(ui);
    redraw_open_if_changed(ui, state);
    if !inbox_view {
        push_record(ui, state);
    }
}

thread_local! {
    /// The Registre rows' model, kept and synced in place (8.6 G3 lesson): a re-read on a poll
    /// keeps the row instances — their « Détail » state and the keyboard focus.
    static RECORD_ROWS: Rc<VecModel<RecordRow>> = Rc::new(VecModel::default());
}

fn record_to_slint(v: &RecordView) -> Vec<RecordRow> {
    v.rows
        .iter()
        .map(|r| RecordRow {
            id: r.id.to_string().into(),
            kind: kind_wire(r.kind).into(),
            target: r.target.clone().into(),
            current: r.current.clone().into(),
            proposed: r.proposed.clone().into(),
            outcome: r.outcome.wire(),
            state: r.state.wire(),
            stale_at_decision: r.stale_at_decision,
            edited: r.edited,
            submitted: r.submitted.clone().into(),
            decided: r.decided.clone().into(),
            // AI-written: read in Slint ONLY inside an AiFrame (UX spec §4.1).
            ai_client: r.ai_client.clone().into(),
            ai_model: r.ai_model.clone().into(),
            ai_lead: r.ai_lead.clone().into(),
            ai_text: r.ai_text.clone().into(),
        })
        .collect()
}

/// Read and push the « Registre » (Story 8.7, FR77) — only while it is shown (the view chip, a
/// `data_version` move through `push_drafts`, a filter): every draft, filtered by the shared kind
/// and study picks and the outcome chip. A failed read is the ⊘ band, never an empty list.
pub(crate) fn push_record(ui: &MainWindow, state: &JournalState) {
    let drafts_global = ui.global::<Drafts>();
    let fail = |cause: &str| {
        drafts_global.set_record_failed(true);
        drafts_global.set_record_failure_cause(cause.into());
        RECORD_ROWS.with(|m| m.set_vec(Vec::new()));
        drafts_global.set_record_rows(RECORD_ROWS.with(|m| ModelRc::from(m.clone())));
    };
    let data = match state.read_record() {
        Ok(data) => data,
        Err(error) => return fail(error.cause.unwrap_or("")),
    };
    let (drafts, studies, archived) = match data {
        Some(d) => (d.drafts, d.studies, d.archived),
        None => Default::default(),
    };
    let kind = KindFilter::from_wire(drafts_global.get_kind_filter().as_str());
    let outcome = OutcomeFilter::from_wire(drafts_global.get_outcome_filter().as_str());
    let study_filter = INBOX.with(|c| c.borrow().study_filter);
    let view = match record_rows(
        &drafts,
        &studies,
        &archived,
        number_format(ui),
        kind,
        study_filter,
        outcome,
    ) {
        Ok(view) => view,
        Err(problem) => {
            tracing::warn!(
                "draft {} cannot be shown in the record: {}",
                problem.draft_id,
                problem.detail
            );
            return fail(INBOX_CAUSE_UNREADABLE_DRAFT);
        }
    };
    let labels: Vec<SharedString> = std::iter::once(SharedString::from(STUDY_FILTER_ALL))
        .chain(view.study_choices.iter().map(|c| c.label.as_str().into()))
        .collect();
    drafts_global.set_study_options(ModelRc::new(VecModel::from(labels)));
    let value = study_filter
        .and_then(|id| view.study_choices.iter().find(|c| c.study_id == id))
        .map_or(STUDY_FILTER_ALL, |c| c.label.as_str())
        .to_string();
    drafts_global.set_study_value(value.into());
    INBOX.with(|c| c.borrow_mut().record_choices = view.study_choices.clone());
    let rows = record_to_slint(&view);
    RECORD_ROWS.with(|model| {
        let keep = rows.len().min(model.row_count());
        for (i, row) in rows.iter().enumerate().take(keep) {
            if model.row_data(i).as_ref() != Some(row) {
                model.set_row_data(i, row.clone());
            }
        }
        while model.row_count() > rows.len() {
            model.remove(model.row_count() - 1);
        }
        for row in rows.into_iter().skip(keep) {
            model.push(row);
        }
        drafts_global.set_record_rows(ModelRc::from(model.clone()));
    });
    drafts_global.set_record_unfiltered(view.unfiltered as i32);
    drafts_global.set_record_failure_cause(SharedString::new());
    drafts_global.set_record_failed(false);
}

/// Back to « Toutes » and « Toutes les études » (G3): on a dossier change and when the reader
/// opens the inbox from the rail — the filters survive only the bands' « Voir les propositions »
/// path, which sets them.
pub(crate) fn reset_filters(ui: &MainWindow) {
    INBOX.with(|cache| cache.borrow_mut().study_filter = None);
    let drafts = ui.global::<Drafts>();
    drafts.set_kind_filter("all".into());
    // Story 8.7: back to « À traiter », every outcome.
    drafts.set_view("inbox".into());
    drafts.set_outcome_filter("all".into());
}

/// A dossier change (open, create, recent, restore, a lost journal): forget the poller's version,
/// the open study, the filters and the last rows; the next tick re-reads (the caller's
/// `refresh_studies` also pushes).
pub(crate) fn forget_dossier(ui: &MainWindow) {
    DECISION.with(|d| *d.borrow_mut() = None);
    // G3: an outcome said of the previous dossier is not this one's.
    ui.global::<Drafts>().set_notice(SharedString::new());
    INBOX.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.poll.reset();
        cache.open_study = None;
        cache.shown = None;
        cache.judgments.clear();
    });
    clear_ai_judgments(ui);
    reset_filters(ui);
}

/// Wire the inbox's callbacks: the kind chips, the « Étude : » filter, the bands' « Voir les
/// propositions ».
pub(crate) fn wire_drafts(ui: &MainWindow, s: &Session) {
    let drafts = ui.global::<Drafts>();
    // Story 8.7: the view chips and the Registre's outcome chips.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&s.journal_state);
        drafts.on_pick_view(move |view| {
            let ui = ui_weak.unwrap();
            ui.global::<Drafts>().set_view(view);
            // The study pick survives when the other view lists that study (push re-checks).
            INBOX.with(|cache| cache.borrow_mut().shown = None);
            push_drafts(&ui, &journal_state.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&s.journal_state);
        drafts.on_pick_outcome(move |outcome| {
            let ui = ui_weak.unwrap();
            ui.global::<Drafts>().set_outcome_filter(outcome);
            push_record(&ui, &journal_state.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&s.journal_state);
        drafts.on_pick_kind(move |kind| {
            let ui = ui_weak.unwrap();
            // « Études » lists draft studies, which belong to no study: the study pick would
            // hide them all (G3).
            if kind.as_str() == "studies" {
                INBOX.with(|cache| cache.borrow_mut().study_filter = None);
            }
            ui.global::<Drafts>().set_kind_filter(kind);
            push_drafts(&ui, &journal_state.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&s.journal_state);
        drafts.on_pick_study(move |index| {
            let ui = ui_weak.unwrap();
            INBOX.with(|cache| {
                let mut cache = cache.borrow_mut();
                // 0 = « Toutes les études »; i → the (i-1)-th study, by identity (never by label).
                let choices = if ui.global::<Drafts>().get_view() == "record" {
                    &cache.record_choices
                } else {
                    &cache.choices
                };
                cache.study_filter = usize::try_from(index)
                    .ok()
                    .and_then(|i| i.checked_sub(1))
                    .and_then(|i| choices.get(i))
                    .map(|c| c.study_id);
            });
            push_drafts(&ui, &journal_state.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&s.journal_state);
        drafts.on_show_for_study(move || {
            let ui = ui_weak.unwrap();
            INBOX.with(|cache| {
                let mut cache = cache.borrow_mut();
                cache.study_filter = cache.open_study;
            });
            ui.global::<Drafts>().set_kind_filter("all".into());
            // Setting the screen from Rust fires no `screen-activated`: push explicitly.
            ui.set_current_screen(PROPOSITIONS_SCREEN);
            push_drafts(&ui, &journal_state.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&s.journal_state);
        drafts.on_show_draft_studies(move || {
            let ui = ui_weak.unwrap();
            INBOX.with(|cache| cache.borrow_mut().study_filter = None);
            ui.global::<Drafts>().set_kind_filter("studies".into());
            ui.set_current_screen(PROPOSITIONS_SCREEN);
            push_drafts(&ui, &journal_state.borrow());
        });
    }
}

/// Wire the decision dialog (Story 8.5b): a row opens it; its verbs decide.
pub(crate) fn wire_decisions(ui: &MainWindow, s: &Session) {
    let drafts = ui.global::<Drafts>();
    // Story 8.7 (Q14): « Valider… » on a draft-study row — its decision is read afresh (as the
    // row's activation reads it), then the overlay switches straight to the prefilled create form.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&s.journal_state);
        drafts.on_validate_study(move |id_text| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            open_decision(&ui, &journal_state.borrow(), id);
            let dialog = ui.global::<Dialog>();
            if dialog.get_kind() == "decision" && dialog.get_decision().kind == "study" {
                dialog.invoke_open_draft_study_form();
            }
        });
    }
    // Story 8.7: « Créer » in the draft create form — the study and the draft's validation in one
    // write (arch A8); a refusal lands inline (the form's own gesture) and a draft decided
    // meanwhile re-reads the inbox.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&s.journal_state);
        drafts.on_create_from_draft(move |id_text, ticker, currency, name| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return false;
            };
            let result = journal_state
                .borrow_mut()
                .validate_draft_study(id, &ticker, &currency, &name);
            match result {
                Ok(_) => {
                    let text = if journal_state.borrow().draft_was_edited(id) {
                        MSG_DRAFT_VALIDATED_EDITED
                    } else {
                        MSG_DRAFT_VALIDATED
                    };
                    ui.global::<Drafts>().set_notice(text.into());
                    DECISION.with(|d| *d.borrow_mut() = None);
                    // The app's own write does not move `data_version` (A9): re-read here — the
                    // Études list (the new study) and, through it, the inbox.
                    crate::wiring::studies::refresh_studies(&ui, &journal_state.borrow());
                    true
                }
                Err(message) => {
                    dialog::refuse(&ui, &message);
                    push_drafts(&ui, &journal_state.borrow());
                    false
                }
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&s.journal_state);
        drafts.on_open_draft(move |id_text| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            open_decision(&ui, &journal_state.borrow(), id);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&s.journal_state);
        drafts.on_decide(move |verb| {
            let ui = ui_weak.unwrap();
            let Some(draft) = DECISION.with(|d| d.borrow().as_ref().map(|o| o.draft.clone()))
            else {
                return false;
            };
            let stale = draft.freshness == DraftFreshness::Stale;
            match verb.as_str() {
                "reject" => decide_open(&ui, &journal_state, Decision::Reject),
                "validate" if stale => {
                    raise_stale_confirm(&ui, None);
                    false
                }
                "validate" => decide_open(
                    &ui,
                    &journal_state,
                    Decision::Validate {
                        seen_fingerprint: None,
                    },
                ),
                "edit" => {
                    let text = ui.global::<Dialog>().get_draft_text().to_string();
                    // A text that reads as no value is refused inline, before any confirmation.
                    let value = match read_edit(&ui, &draft, &text) {
                        Ok(value) => value,
                        Err(message) => {
                            dialog::refuse(&ui, &message);
                            return false;
                        }
                    };
                    if stale {
                        raise_stale_confirm(&ui, Some(text));
                        return false;
                    }
                    decide_open(
                        &ui,
                        &journal_state,
                        Decision::ValidateEdited {
                            seen_fingerprint: None,
                            value,
                        },
                    )
                }
                _ => false,
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&s.journal_state);
        drafts.on_confirm_stale(move || {
            let ui = ui_weak.unwrap();
            let Some((draft, edit)) = DECISION.with(|d| {
                d.borrow()
                    .as_ref()
                    .and_then(|o| o.stale_edit.clone().map(|e| (o.draft.clone(), e)))
            }) else {
                return false;
            };
            // Back to the decision (or its edit form) first: a refusal lands inline there.
            let dialog = ui.global::<Dialog>();
            dialog.set_title(dialog.get_resume_title());
            dialog.set_body(dialog.get_resume_body());
            dialog.set_action(SharedString::new());
            dialog.set_verb(SharedString::new());
            let back = if dialog.get_form_id().as_str() == "draft-edit" {
                "form"
            } else {
                "decision"
            };
            dialog.set_kind(back.into());
            // The fingerprint the owner SAW when the dialog opened (arch A7, O4).
            let seen = draft.seen_fingerprint.clone();
            let decision = match edit {
                None => Decision::Validate {
                    seen_fingerprint: seen,
                },
                Some(text) => match read_edit(&ui, &draft, &text) {
                    Ok(value) => Decision::ValidateEdited {
                        seen_fingerprint: seen,
                        value,
                    },
                    Err(message) => {
                        dialog::refuse(&ui, &message);
                        return false;
                    }
                },
            };
            decide_open(&ui, &journal_state, decision)
        });
    }
}

/// The « Propositions » screen's index (`MainWindow.current-screen`).
pub(crate) const PROPOSITIONS_SCREEN: i32 = 4;

/// Start the `data_version` poller (arch A9). The returned timer must be kept alive for the event
/// loop and stopped after it (the `restore_timer` pattern in `main.rs`).
pub(crate) fn start_poller(ui: &MainWindow, s: &Session) -> Rc<slint::Timer> {
    let timer = Rc::new(slint::Timer::default());
    let ui_weak = ui.as_weak();
    let journal_state = Rc::clone(&s.journal_state);
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(POLL_PERIOD_MS),
        move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            // A busy journal (a write in progress elsewhere in the app) waits for the next tick.
            let Ok(state) = journal_state.try_borrow() else {
                return;
            };
            // Logged once, when a failure starts (the poller would repeat it every tick).
            let log = !INBOX.with(|c| c.borrow().failed);
            let read = state
                .try_data_version(log)
                .map_err(|e| e.cause.unwrap_or("").to_string());
            let action = INBOX.with(|cache| cache.borrow_mut().poll.on_tick(read));
            match action {
                PollAction::Nothing => {}
                // A failed re-read marks the poller (`show_failure`): the next tick retries.
                PollAction::Reread => push_drafts(&ui, &state),
                PollAction::Failed(cause) => show_failure(&ui, &cause),
            }
        },
    );
    timer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chip(id: &str, value: &str) -> AiChip {
        AiChip {
            draft_id: id.into(),
            value: value.into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_chip_row_is_synced_in_place_never_rebuilt() {
        let model = VecModel::from(vec![chip("a", "1"), chip("b", "2")]);
        // A proposal arrives on another field: the existing rows are untouched, one is added.
        sync_rows(&model, vec![chip("a", "1"), chip("b", "2"), chip("c", "3")]);
        assert_eq!(model.row_count(), 3);
        // One is decided, another changes: the row count shrinks from the end, rows are replaced.
        sync_rows(&model, vec![chip("a", "1"), chip("c", "3")]);
        let rows: Vec<AiChip> = model.iter().collect();
        assert_eq!(rows, vec![chip("a", "1"), chip("c", "3")]);
        sync_rows(&model, Vec::new());
        assert_eq!(model.row_count(), 0);
    }
}
