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
//! This is the ONLY place the `DraftRow.ai_*` fields are set: AI-written text enters no other
//! model (the guard test in `posture.rs`).

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use steadyinvest_contract::DraftKind;
use uuid::Uuid;

use crate::state::JournalState;
use crate::viewmodel::drafts::{
    InboxView, KindFilter, PendingCounts, PollAction, PollState, STUDY_FILTER_ALL, StudyChoice,
    inbox_rows, pending_counts,
};
use crate::viewmodel::format::NumberFormat;
use crate::wiring::Session;
use crate::{DraftRow, Drafts, MainWindow, Prefs, Studies};

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
            ai_text: r.ai_text.clone().into(),
        })
        .collect()
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

/// Show « indisponible »: rows cleared, counts zero, the cause named ("" when none can be).
fn show_failure(ui: &MainWindow, cause: &str) {
    INBOX.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.counts = PendingCounts::default();
        cache.failed = true;
    });
    let drafts = ui.global::<Drafts>();
    drafts.set_rows(ModelRc::new(VecModel::from(Vec::<DraftRow>::new())));
    drafts.set_pending_count(0);
    drafts.set_draft_study_count(0);
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
            return;
        }
    };
    let (drafts, studies) = match data {
        Some(d) => (d.drafts, d.studies),
        // No dossier open: a true absence — an empty inbox, no ⊘.
        None => (Vec::new(), std::collections::HashMap::new()),
    };
    let counts = pending_counts(&drafts);
    let kind = KindFilter::from_wire(drafts_global.get_kind_filter().as_str());
    // The « Étude : » pick survives a re-read while that study still has pending drafts.
    let study_filter = INBOX.with(|c| c.borrow().study_filter);
    let study_filter = study_filter.filter(|id| counts.by_study.contains_key(id));
    let view = match inbox_rows(&drafts, &studies, number_format(ui), kind, study_filter) {
        Ok(view) => view,
        Err(problem) => {
            tracing::warn!(
                "draft {} cannot be shown in the inbox: {}",
                problem.draft_id,
                problem.detail
            );
            show_failure(ui, "");
            return;
        }
    };
    let (open_study, total, draft_studies) = INBOX.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.failed = false;
        cache.choices = view.study_choices.clone();
        cache.study_filter = study_filter;
        cache.counts = counts;
        (
            cache.open_study,
            cache.counts.total,
            cache.counts.draft_studies,
        )
    });
    let mut options: Vec<SharedString> = vec![STUDY_FILTER_ALL.into()];
    options.extend(
        view.study_choices
            .iter()
            .map(|c| SharedString::from(c.label.as_str())),
    );
    let value = study_filter
        .and_then(|id| view.study_choices.iter().find(|c| c.study_id == id))
        .map_or(STUDY_FILTER_ALL, |c| c.label.as_str())
        .to_string();
    drafts_global.set_study_options(ModelRc::new(VecModel::from(options)));
    drafts_global.set_study_value(value.into());
    drafts_global.set_rows(ModelRc::new(VecModel::from(to_slint(&view))));
    drafts_global.set_pending_count(total as i32);
    drafts_global.set_draft_study_count(draft_studies as i32);
    drafts_global.set_failure_cause(SharedString::new());
    drafts_global.set_read_failed(false);
    apply_open_study(ui, open_study);
    apply_row_counts(ui);
}

/// A dossier change (open, create, recent, restore, a lost journal): forget the poller's version
/// and the open study; the next tick re-reads (the caller's `refresh_studies` also pushes).
pub(crate) fn forget_dossier() {
    INBOX.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.poll.reset();
        cache.open_study = None;
        cache.study_filter = None;
    });
}

/// Wire the inbox's callbacks: the kind chips, the « Étude : » filter, the bands' « Voir les
/// propositions ».
pub(crate) fn wire_drafts(ui: &MainWindow, s: &Session) {
    let drafts = ui.global::<Drafts>();
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&s.journal_state);
        drafts.on_pick_kind(move |kind| {
            let ui = ui_weak.unwrap();
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
                cache.study_filter = usize::try_from(index)
                    .ok()
                    .and_then(|i| i.checked_sub(1))
                    .and_then(|i| cache.choices.get(i))
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
                PollAction::Reread => push_drafts(&ui, &state),
                PollAction::Failed(cause) => show_failure(&ui, &cause),
            }
        },
    );
    timer
}
