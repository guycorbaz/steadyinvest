//! Journal wiring (Réglages, Epic 5): whole-journal export / import (Story 5.3, FR60) with its
//! `exports/`-folder writer, backup + validate-before-restore (Story 5.4, FR61 — raw `.db`, never
//! silent), and the Story 5.5 (FR66) journal-location rails — native rfd pick-open / pick-create,
//! recent-journals reopen, stale-lock reclaim — all funnelling through `finish_journal_switch`
//! (recent-pointer recording, stale-on-reopen notice, full re-render, never journal-less). Moved
//! verbatim from `main.rs` — no logic change.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::state;
use crate::state::JournalState;
use crate::wiring::holdings::{HoldingFreshnessMap, refresh_holdings, retain_held_freshness};
use crate::wiring::studies::refresh_studies;
use crate::wiring::watchlist::refresh_watchlist;
use crate::wiring::{Session, persist};
use crate::{MainWindow, Prefs, RecentJournalRow, Studies};
use steadyinvest_persistence::lock_is_stale;

/// Write the whole-journal export envelope to a file (Story 5.3, FR60) and return its path. Like the
/// single-study export, it lands in the `exports/` folder under the OS data dir — **never** beside the
/// live journal DB (ADD7/8 sync-safety; the native picker + a user-chosen sync target is Story 5.5).
/// Named by the journal id (stable, unique). `app` owns the file I/O.
fn write_journal_export(journal_id: Uuid, json: &str) -> std::io::Result<std::path::PathBuf> {
    let dir = directories::ProjectDirs::from("", "", "steadyinvest")
        .map(|d| d.data_dir().join("exports"))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no OS data directory"))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("journal-{journal_id}.json"));
    std::fs::write(&path, json)?;
    Ok(path)
}

/// The `exports/` folder under the OS data dir — where the import pickers open by default (the user
/// is free to browse anywhere). `None` when the OS exposes no data directory.
fn default_exports_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "steadyinvest").map(|d| d.data_dir().join("exports"))
}

/// Push the journal-location panel state into `Prefs` (Story 5.5): the current journal path + the
/// recent-journals rows (the current one marked). A short `name` is the parent-dir + file name.
pub(crate) fn render_journal_panel(ui: &MainWindow, state: &JournalState, config: &AppConfig) {
    let prefs = ui.global::<Prefs>();
    let current = state.path().map(|p| p.to_path_buf());
    prefs.set_journal_current_path(
        current
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
            .into(),
    );
    let rows: Vec<RecentJournalRow> = config
        .recent_journals
        .iter()
        .map(|r| {
            let name = journal_short_name(&r.path);
            RecentJournalRow {
                path: r.path.display().to_string().into(),
                name: name.into(),
                current: current.as_ref().is_some_and(|c| c == &r.path),
            }
        })
        .collect();
    prefs.set_recent_journals(ModelRc::new(VecModel::from(rows)));
}

/// The session state that belongs to the OPEN dossier — what a dossier change must end (G1 G,
/// checklist §7 « clears on journal switch »). Cloned handles out of [`Session`], plus the
/// location status computed when the open dossier was opened (see [`status_after_refusal`]).
#[derive(Clone)]
pub(crate) struct DossierSession {
    current_study: Rc<RefCell<Option<String>>>,
    quick_screen: Rc<RefCell<Option<crate::wiring::quick_screen::QuickScreenSession>>>,
    quick_screen_request: Rc<std::cell::Cell<u64>>,
    screening: Rc<RefCell<Option<crate::wiring::screening::ScreeningSession>>>,
    /// `(path, status)` of the last SUCCESSFUL open/create/switch — the open dossier's own status
    /// line, which a later refusal restores (never a string read back from the screen).
    opened_status: Rc<RefCell<Option<(PathBuf, String)>>>,
}

impl DossierSession {
    /// Built ONCE, in [`wire_journal`], and cloned into each rail (the `opened_status` cell is
    /// created here and shared by those clones).
    fn of(s: &Session) -> Self {
        Self {
            current_study: Rc::clone(&s.current_study),
            quick_screen: Rc::clone(&s.quick_screen),
            quick_screen_request: Rc::clone(&s.quick_screen_request),
            screening: Rc::clone(&s.screening),
            opened_status: Rc::new(RefCell::new(None)),
        }
    }
}

/// End the previous dossier's session — the ONE reset every dossier-changing path calls (open,
/// create, recent, reclaim, restore), so a future path cannot forget a piece. Call it BEFORE the
/// re-render (the comparison's picker is then re-listed from the new dossier by
/// `refresh_studies`).
/// - the open study editor closes (a stale form must never save an old study id into the new
///   dossier);
/// - the Études, Liste de suivi and Portefeuille notices go (an outcome of the previous dossier —
///   « L'étude a été créée… », a startup notice — never reads as the new one's; the new dossier
///   re-derives its own states);
/// - the replacement candidates panel empties;
/// - the FX panel: the in-flight flag and the sticky notice reset (rates are dossier data);
/// - the comparison: picks (ids + labels), table and notice empty, the screen closes;
/// - the examen rapide: a fetch in flight is superseded, the session empties, the screen closes,
///   its notice goes;
/// - the criblage: the run stops (its queued rows drain, its late results are dropped), the slot
///   empties, the card hides — the batch counter keeps counting (it outlives any run);
/// - the Revue: its export notice goes (it named a file of the previous dossier's review).
fn clear_dossier_session(ui: &MainWindow, session: &DossierSession) {
    *session.current_study.borrow_mut() = None;
    ui.global::<Studies>().set_study_open(false);
    ui.global::<Studies>()
        .set_notice(slint::SharedString::new());
    ui.global::<crate::Watchlist>()
        .set_notice(slint::SharedString::new());
    ui.global::<crate::Holdings>()
        .set_notice(slint::SharedString::new());
    crate::wiring::replacement::clear_candidates(ui);
    ui.global::<crate::Fx>().set_refreshing(false);
    ui.global::<crate::Fx>()
        .set_notice(slint::SharedString::new());
    crate::wiring::comparison::clear_comparison(ui);
    crate::wiring::comparison::close_screen(ui);
    crate::wiring::quick_screen::clear_examination(
        ui,
        &session.quick_screen,
        &session.quick_screen_request,
    );
    crate::wiring::screening::close_screening(ui, &session.screening);
    ui.global::<crate::Review>()
        .set_notice(slint::SharedString::new());
}

/// Finish a journal open/create/switch (Story 5.5): on success, record the recent-journals pointer +
/// persist app-config, surface the right neutral notice (stale / sync-warning / opened-or-created),
/// close any open study editor, and re-render every surface + the location panel. On failure, surface
/// the cause and — when a **stale** lock blocked the open — offer to reclaim it. Never journal-less
/// (the rails reopen the previous journal on failure).
#[allow(clippy::too_many_arguments)]
fn finish_journal_switch(
    ui: &MainWindow,
    result: Result<state::OpenOutcome, String>,
    attempted: &std::path::Path,
    created: bool,
    journal_state: &Rc<RefCell<JournalState>>,
    config: &Rc<RefCell<AppConfig>>,
    config_path: &Option<PathBuf>,
    holding_freshness: &Rc<RefCell<HoldingFreshnessMap>>,
    holding_dismissed: &Rc<RefCell<std::collections::HashSet<String>>>,
    dossier: &DossierSession,
) {
    let prefs = ui.global::<Prefs>();
    match result {
        Ok(outcome) => {
            let opened_path = journal_state
                .borrow()
                .path()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| attempted.to_path_buf());
            // Stale-on-reopen: the **same** journal (matching journal_id) at a **lower** on-disk
            // version than last seen → a neutral flag (not a block). The journal_id guard prevents a
            // spurious "older" notice for a *different* (or newly-created) journal at a reused path.
            let stale_seen = config
                .borrow()
                .last_seen_for(&opened_path)
                .and_then(|(jid, seen)| {
                    (jid == outcome.journal_id.to_string() && outcome.logical_version < seen)
                        .then_some(seen)
                });
            {
                let mut cfg = config.borrow_mut();
                cfg.record_recent(
                    &opened_path,
                    &outcome.journal_id.to_string(),
                    outcome.logical_version,
                );
            }
            persist(config_path.as_ref(), &config.borrow());

            let status = opened_status(
                journal_state.borrow().is_read_only(),
                stale_seen.map(|seen| (seen, outcome.logical_version)),
                outcome.sync_warning,
                created,
            );
            prefs.set_journal_location_status(status.as_str().into());
            prefs.set_journal_reclaim_path("".into());
            *dossier.opened_status.borrow_mut() = Some((opened_path, status));

            // The whole journal changed — end the previous dossier's session (study editor,
            // comparison, examination, criblage, FX flag, notices) and re-render every surface.
            // Re-selecting the dossier already open changed nothing (G1 G review): its session
            // — an examination, a criblage, a comparison — stays.
            if !outcome.unchanged {
                clear_dossier_session(ui, dossier);
            }
            let st = journal_state.borrow();
            let format = config.borrow().number_format;
            retain_held_freshness(holding_freshness, &st);
            refresh_studies(ui, &st);
            refresh_watchlist(ui, &st);
            // Story 6.5 review: the FX panel follows the journal (rates are journal data).
            crate::wiring::fx::push_fx_rates(ui, &st);
            refresh_holdings(
                ui,
                &st,
                &holding_freshness.borrow(),
                &holding_dismissed.borrow(),
                format,
            );
            render_journal_panel(ui, &st, &config.borrow());
        }
        Err(notice) => {
            // Offer reclaim ONLY when the failure was the lock AND that lock is genuinely stale (a
            // crashed run) — never for an unrelated open failure (corrupt / not-a-journal) that merely
            // happens to sit beside a stale lock, and never for a live instance's lock.
            let lock_failure = notice == state::MSG_JOURNAL_LOCKED;
            if lock_failure && lock_is_stale(attempted) {
                prefs.set_journal_location_status(state::MSG_JOURNAL_LOCK_RECLAIMABLE.into());
                prefs.set_journal_reclaim_path(attempted.display().to_string().into());
            } else {
                // An open/create that did not happen is a refusal (acknowledged). The status
                // line is the OPEN dossier's own (G1 G), recomputed from the journal actually
                // open — never the string on screen, which may be a reclaim offer about an
                // earlier attempt.
                let st = journal_state.borrow();
                let open = st.path();
                let status = status_after_refusal(
                    open,
                    open.is_some_and(state::is_sync_folder),
                    st.is_read_only(),
                    dossier.opened_status.borrow().as_ref(),
                );
                prefs.set_journal_location_status(status.into());
                prefs.set_journal_reclaim_path("".into());
                crate::wiring::dialog::refuse(ui, &notice);
            }
        }
    }
}

/// PURE: the location status line of a dossier just opened/created. A read-only dossier (written by
/// a newer schema) says so FIRST, in the startup wording (G1 G, on-screen check: it read only « Le
/// dossier a été ouvert. »); then the stale-on-reopen flag (`(seen, here)`) or else the sync-folder
/// warning; the plain « ouvert » / « créé » acknowledgement only when no state applies. The
/// result is also what a later refusal restores ([`status_after_refusal`]).
fn opened_status(
    read_only: bool,
    stale: Option<(u64, u64)>,
    sync_warning: bool,
    created: bool,
) -> String {
    dossier_states(read_only, stale, sync_warning).unwrap_or_else(|| {
        if created {
            state::MSG_JOURNAL_CREATED.to_string()
        } else {
            state::MSG_JOURNAL_OPENED.to_string()
        }
    })
}

/// PURE: the STATES of the open dossier, read-only first — `None` when none applies.
fn dossier_states(
    read_only: bool,
    stale: Option<(u64, u64)>,
    sync_warning: bool,
) -> Option<String> {
    let state_line = if let Some((seen, here)) = stale {
        Some(state::journal_stale_message(seen, here))
    } else if sync_warning {
        Some(state::MSG_SYNC_FOLDER_WARNING.to_string())
    } else {
        None
    };
    match (read_only, state_line) {
        (true, Some(line)) => Some(format!("{} {line}", state::MSG_STARTUP_READ_ONLY)),
        (true, None) => Some(state::MSG_STARTUP_READ_ONLY.to_string()),
        (false, line) => line,
    }
}

/// PURE: the location status line after a refused open/create/switch (G1 G), from the journal
/// actually open (`open`, and whether it sits in a sync folder) and the `(path, status)` recorded
/// when a dossier was last opened:
/// - no journal open (the previous one could not be reacquired) → nothing to state;
/// - the open journal is the one recorded → its own status (stale, sync folder, opened/created);
/// - otherwise (the startup dossier: no switch recorded it) → its states recomputed — read-only,
///   the sync-folder warning — else nothing.
///
/// A reclaimable-lock offer is never kept: it described an earlier attempt, not the open dossier.
fn status_after_refusal(
    open: Option<&std::path::Path>,
    in_sync_folder: bool,
    read_only: bool,
    recorded: Option<&(PathBuf, String)>,
) -> String {
    let Some(open) = open else {
        return String::new();
    };
    match recorded {
        Some((path, status)) if path == open => status.clone(),
        _ => dossier_states(read_only, None, in_sync_folder).unwrap_or_default(),
    }
}

/// Record the **currently-open** journal's `(journal_id, logical_version)` into app-config before
/// switching away or exiting (Story 5.5) — so the last-seen pointer reflects edits made since it was
/// opened (the stale-on-reopen check compares against the true last-seen, not the open-time, version).
pub(crate) fn record_current_pointer(
    journal_state: &Rc<RefCell<JournalState>>,
    config: &Rc<RefCell<AppConfig>>,
    config_path: &Option<PathBuf>,
) {
    let st = journal_state.borrow();
    if let (Some(path), Some(jid)) = (st.path().map(|p| p.to_path_buf()), st.journal_id()) {
        config
            .borrow_mut()
            .record_recent(&path, &jid.to_string(), st.logical_version_or_zero());
        persist(config_path.as_ref(), &config.borrow());
    }
}

/// A short label for a journal path (Story 5.5): `<parent-dir>/<file>` — enough to tell journals apart
/// without showing a long absolute path.
fn journal_short_name(path: &std::path::Path) -> String {
    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    match path.parent().and_then(|p| p.file_name()) {
        Some(parent) => format!("{}/{file}", parent.to_string_lossy()),
        None => file,
    }
}

/// Wire the journal domain (all on the `Prefs` global): export / import the whole journal,
/// create-backup + request / confirm / cancel restore, and the journal-location rails (pick-open,
/// pick-create, open-recent, reclaim-and-open).
pub(crate) fn wire_journal(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        config_path,
        holding_freshness,
        holding_dismissed,
        ..
    } = s;
    // G1 G: every dossier-changing rail below ends the previous dossier's session through ONE
    // helper, `clear_dossier_session`.
    let dossier = DossierSession::of(s);
    // ── Story 5.3 (FR60) — export / import the WHOLE journal as a portable file. Scales the 5.2
    // envelope to every entity + the (journal_id, version, hash) identity tuple; import verifies and
    // applies atomically (never partially). Import is picker-fed (below) but stays path-based. The
    // actions live in Réglages (the Prefs global). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Prefs>().on_export_journal(move || {
            let ui = ui_weak.unwrap();
            let state = journal_state.borrow();
            let outcome = match state.export_journal() {
                Ok(json) => match state.journal_id() {
                    Some(jid) => match write_journal_export(jid, &json) {
                        Ok(path) => Ok(format!(
                            "{} {}",
                            state::MSG_JOURNAL_EXPORTED,
                            path.display()
                        )),
                        Err(e) => Err(format!("{} {e}", state::MSG_SAVE_FAILED)),
                    },
                    None => Err(state::MSG_NO_JOURNAL.to_string()),
                },
                Err(message) => Err(message),
            };
            match outcome {
                Ok(notice) => ui.global::<Prefs>().set_journal_status(notice.into()),
                Err(message) => crate::wiring::dialog::refuse(&ui, &message),
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Prefs>().on_import_journal(move |path| {
            let ui = ui_weak.unwrap();
            let prefs = ui.global::<Prefs>();
            let outcome = match std::fs::read_to_string(path.as_str()) {
                // Issue #65: the arbitration gate — an OLDER same-journal envelope parks behind a
                // modal confirm instead of silently snapping shared entities back.
                Ok(json) => match journal_state.borrow_mut().request_import_journal(&json) {
                    Ok(state::ImportRequest::Applied(summary)) => {
                        Ok(state::journal_imported_message(&summary))
                    }
                    Ok(state::ImportRequest::NeedsConfirm { source, current }) => {
                        let prompt = state::import_confirm_message(source, current);
                        prefs.set_journal_status("".into());
                        crate::wiring::dialog::confirm(&ui, "confirm-import", &prompt);
                        return; // nothing applied yet — no re-render needed
                    }
                    Err(message) => Err(message),
                },
                // An unreadable path is the malformed/unreadable case — a neutral refusal, no panic.
                Err(_) => Err(state::MSG_IMPORT_MALFORMED.to_string()),
            };
            match outcome {
                Ok(notice) => prefs.set_journal_status(notice.into()),
                Err(message) => {
                    prefs.set_journal_status("".into());
                    crate::wiring::dialog::refuse(&ui, &message);
                }
            }
            // A whole-journal import can touch every surface — re-render them all (dashboard,
            // watchlist, portfolio). Prune any stale per-holding freshness for tickers no longer held.
            let state = journal_state.borrow();
            let format = config.borrow().number_format;
            retain_held_freshness(&holding_freshness, &state);
            refresh_studies(&ui, &state);
            refresh_watchlist(&ui, &state);
            // Story 6.5 review: imported/restored fx_rates must show without a restart.
            crate::wiring::fx::push_fx_rates(&ui, &state);
            crate::wiring::replacement::clear_candidates(&ui);
            refresh_holdings(
                &ui,
                &state,
                &holding_freshness.borrow(),
                &holding_dismissed.borrow(),
                format,
            );
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Prefs>().on_confirm_import(move || {
            let ui = ui_weak.unwrap();
            let result = journal_state.borrow_mut().confirm_import_journal();
            let prefs = ui.global::<Prefs>();
            match result {
                Ok(summary) => {
                    prefs.set_journal_status(state::journal_imported_message(&summary).into())
                }
                Err(message) => {
                    prefs.set_journal_status("".into());
                    crate::wiring::dialog::refuse(&ui, &message);
                }
            }
            // The confirmed merge can touch every surface — same re-render as a direct import.
            let state = journal_state.borrow();
            let format = config.borrow().number_format;
            retain_held_freshness(&holding_freshness, &state);
            refresh_studies(&ui, &state);
            refresh_watchlist(&ui, &state);
            crate::wiring::fx::push_fx_rates(&ui, &state);
            crate::wiring::replacement::clear_candidates(&ui);
            refresh_holdings(
                &ui,
                &state,
                &holding_freshness.borrow(),
                &holding_dismissed.borrow(),
                format,
            );
        });
    }
    {
        let journal_state = Rc::clone(journal_state);
        ui.global::<Prefs>().on_cancel_import(move || {
            journal_state.borrow_mut().cancel_import_journal();
        });
    }

    {
        // Native `rfd` open picker → the SAME path-based `import-journal` callback (one verify-and-
        // apply code path, headless-tested). Cancel → no notice, nothing read. A read-only dossier
        // refuses at once, before the picker (G1 G, on-screen check).
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Prefs>().on_pick_and_import_journal(move || {
            let ui = ui_weak.unwrap();
            if let Err(reason) = journal_state.borrow().refuse_if_read_only() {
                crate::wiring::dialog::refuse(&ui, reason);
                return;
            }
            let mut dialog = rfd::FileDialog::new()
                .set_title("Importer un dossier")
                .add_filter("Dossier exporté (JSON)", &["json"]);
            if let Some(dir) = default_exports_dir().filter(|d| d.is_dir()) {
                dialog = dialog.set_directory(dir);
            }
            let Some(path) = dialog.pick_file() else {
                return; // the user cancelled the dialog
            };
            ui.global::<Prefs>()
                .invoke_import_journal(path.to_string_lossy().as_ref().into());
        });
    }

    // ── Story 5.4 (FR61) — backup / restore the raw .db. Create a self-contained .db backup; validate
    // a candidate backup (integrity + schema-version + identity) BEFORE any overwrite, surface its
    // (journal_id, version) + a stale/foreign warning, and apply only on explicit confirm (never
    // silently). Restore is picker-fed (below) but stays path-based. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Prefs>().on_create_backup(move || {
            let ui = ui_weak.unwrap();
            let notice = match journal_state.borrow().create_backup() {
                Ok(path) => format!("{} {}", state::MSG_BACKUP_CREATED, path.display()),
                Err(message) => message,
            };
            ui.global::<Prefs>().set_restore_status(notice.into());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Prefs>().on_request_restore(move |path| {
            let ui = ui_weak.unwrap();
            let prefs = ui.global::<Prefs>();
            match journal_state.borrow_mut().request_restore(path.as_str()) {
                // A confirmable restore is parked — reveal the confirm banner with the identity/warning.
                Ok(assessment) => {
                    let prompt = state::restore_confirm_message(&assessment);
                    prefs.set_restore_status("".into());
                    crate::wiring::dialog::confirm(&ui, "confirm-restore", &prompt);
                }
                // A hard refusal — acknowledged in the dialog, nothing parked.
                Err(message) => {
                    prefs.set_restore_status("".into());
                    crate::wiring::dialog::refuse(&ui, &message);
                }
            }
        });
    }
    {
        // Native `rfd` open picker → the SAME path-based `request-restore` callback (validate-before-
        // overwrite + the confirm banner stay the one code path). Opens in the `backups/` folder beside
        // the live journal when it exists. Cancel → no notice, nothing parked.
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Prefs>().on_pick_and_restore_backup(move || {
            let ui = ui_weak.unwrap();
            // A read-only dossier refuses at once — no picker, no confirm (G1 G, on-screen
            // check); `request_restore` and the confirm's blocked verb stay as further guards.
            if let Err(reason) = journal_state.borrow().refuse_if_read_only() {
                crate::wiring::dialog::refuse(&ui, reason);
                return;
            }
            let mut dialog = rfd::FileDialog::new()
                .set_title("Restaurer une sauvegarde")
                .add_filter("Sauvegarde (.db)", &["db"]);
            if let Some(dir) = journal_state.borrow().backups_dir().filter(|d| d.is_dir()) {
                dialog = dialog.set_directory(dir);
            }
            let Some(path) = dialog.pick_file() else {
                return; // the user cancelled the dialog
            };
            ui.global::<Prefs>()
                .invoke_request_restore(path.to_string_lossy().as_ref().into());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        let dossier = dossier.clone();
        ui.global::<Prefs>().on_confirm_restore(move || {
            let ui = ui_weak.unwrap();
            let result = journal_state.borrow_mut().confirm_restore();
            let prefs = ui.global::<Prefs>();
            // A successful restore replaces the whole journal — end the previous dossier's session
            // first (the open study editor above all: a stale in-memory form can't be saved back
            // into the restored journal with an old study_id; then the comparison, examination,
            // criblage and notices — G1 G).
            if result.is_ok() {
                clear_dossier_session(&ui, &dossier);
            }
            match result {
                Ok(()) => prefs.set_restore_status(state::MSG_RESTORE_DONE.into()),
                Err(message) => {
                    prefs.set_restore_status("".into());
                    crate::wiring::dialog::refuse(&ui, &message);
                }
            }
            // The whole journal changed — re-render every surface.
            let state = journal_state.borrow();
            let format = config.borrow().number_format;
            retain_held_freshness(&holding_freshness, &state);
            refresh_studies(&ui, &state);
            refresh_watchlist(&ui, &state);
            // Story 6.5 review: imported/restored fx_rates must show without a restart. (The
            // candidates panel is emptied by `clear_dossier_session` on success; a failed restore
            // left the dossier — and its panel — as they were.)
            crate::wiring::fx::push_fx_rates(&ui, &state);
            refresh_holdings(
                &ui,
                &state,
                &holding_freshness.borrow(),
                &holding_dismissed.borrow(),
                format,
            );
        });
    }
    {
        let journal_state = Rc::clone(journal_state);
        ui.global::<Prefs>().on_cancel_restore(move || {
            journal_state.borrow_mut().cancel_restore();
        });
    }

    // ── Story 5.5 (FR66) — journal location, recent journals, single-instance lock & sync-safety.
    // Native rfd dialogs pick the file/directory on the UI thread (the OS dialog is modal); the rails
    // close the current journal cleanly, open the target in the sync-appropriate mode, and never leave
    // the app journal-less. `finish_journal_switch` records the recent pointer + re-renders. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let config_path = config_path.clone();
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        let dossier = dossier.clone();
        ui.global::<Prefs>().on_pick_and_open_journal(move || {
            let ui = ui_weak.unwrap();
            let Some(path) = rfd::FileDialog::new()
                .set_title("Ouvrir un dossier")
                .add_filter("Dossier (.db)", &["db"])
                .pick_file()
            else {
                return; // the user cancelled the dialog
            };
            // Capture the current journal's final version before switching away (stale-detection input).
            record_current_pointer(&journal_state, &config, &config_path);
            let result = journal_state.borrow_mut().open_journal(&path);
            finish_journal_switch(
                &ui,
                result,
                &path,
                false,
                &journal_state,
                &config,
                &config_path,
                &holding_freshness,
                &holding_dismissed,
                &dossier,
            );
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let config_path = config_path.clone();
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        let dossier = dossier.clone();
        ui.global::<Prefs>().on_pick_and_create_journal(move || {
            let ui = ui_weak.unwrap();
            let Some(path) = rfd::FileDialog::new()
                .set_title("Créer un dossier")
                .add_filter("Dossier (.db)", &["db"])
                .set_file_name("dossier.db")
                .save_file()
            else {
                return;
            };
            // Split the chosen save path into (directory, name) for the create rail. The rail appends
            // `.db`, so the file actually created is `<dir>/<stem>.db` — pass THAT as the attempted
            // path (not the raw dialog path, which may carry a different/absent extension).
            let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let actual = dir.join(format!("{name}.db"));
            record_current_pointer(&journal_state, &config, &config_path);
            let result = journal_state.borrow_mut().create_journal(&dir, &name);
            finish_journal_switch(
                &ui,
                result,
                &actual,
                true,
                &journal_state,
                &config,
                &config_path,
                &holding_freshness,
                &holding_dismissed,
                &dossier,
            );
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let config_path = config_path.clone();
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        let dossier = dossier.clone();
        ui.global::<Prefs>().on_open_recent(move |path_str| {
            let ui = ui_weak.unwrap();
            let path = PathBuf::from(path_str.as_str());
            record_current_pointer(&journal_state, &config, &config_path);
            let result = journal_state.borrow_mut().open_journal(&path);
            finish_journal_switch(
                &ui,
                result,
                &path,
                false,
                &journal_state,
                &config,
                &config_path,
                &holding_freshness,
                &holding_dismissed,
                &dossier,
            );
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let config_path = config_path.clone();
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        let dossier = dossier.clone();
        ui.global::<Prefs>().on_reclaim_and_open(move |path_str| {
            let ui = ui_weak.unwrap();
            let path = PathBuf::from(path_str.as_str());
            record_current_pointer(&journal_state, &config, &config_path);
            let result = journal_state.borrow_mut().reclaim_and_open(&path);
            finish_journal_switch(
                &ui,
                result,
                &path,
                false,
                &journal_state,
                &config,
                &config_path,
                &holding_freshness,
                &holding_dismissed,
                &dossier,
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_switch_states_the_open_dossiers_own_status() {
        let a = PathBuf::from("/d/a.db");
        let b = PathBuf::from("/d/b.db");
        let stale = state::journal_stale_message(7, 5);
        let recorded = (a.clone(), stale.clone());
        // The open dossier is the one recorded: its own status stands (stale here) — whatever
        // the screen showed (a reclaim offer about an earlier attempt included).
        assert_eq!(
            status_after_refusal(Some(&a), false, false, Some(&recorded)),
            stale
        );
        // Another journal is open than the one recorded (the startup dossier): recomputed.
        assert_eq!(
            status_after_refusal(Some(&b), true, false, Some(&recorded)),
            state::MSG_SYNC_FOLDER_WARNING
        );
        assert_eq!(status_after_refusal(Some(&b), false, false, None), "");
        // …and a read-only startup dossier says so.
        assert_eq!(
            status_after_refusal(Some(&b), false, true, None),
            state::MSG_STARTUP_READ_ONLY
        );
        // A read-only dossier opened by a switch: its recorded status already says so.
        let ro = (a.clone(), opened_status(true, None, false, false));
        assert_eq!(
            status_after_refusal(Some(&a), false, true, Some(&ro)),
            state::MSG_STARTUP_READ_ONLY
        );
        // No journal could be reacquired: nothing to state about an open dossier.
        assert_eq!(status_after_refusal(None, true, true, Some(&recorded)), "");
        // Never the reclaim offer.
        for (open, sync) in [(Some(a.as_path()), false), (Some(b.as_path()), true)] {
            assert_ne!(
                status_after_refusal(open, sync, false, Some(&recorded)),
                state::MSG_JOURNAL_LOCK_RECLAIMABLE
            );
        }
    }

    #[test]
    fn an_opened_read_only_dossier_says_so_first() {
        // The on-screen check: a newer-schema dossier opened through « Ouvrir un dossier… »
        // read only « Le dossier a été ouvert. ».
        assert_eq!(
            opened_status(true, None, false, false),
            state::MSG_STARTUP_READ_ONLY
        );
        assert_eq!(
            opened_status(true, None, false, true),
            state::MSG_STARTUP_READ_ONLY
        );
        // With another state: read-only first, the other kept.
        let both = opened_status(true, None, true, false);
        assert!(both.starts_with(state::MSG_STARTUP_READ_ONLY));
        assert!(both.ends_with(state::MSG_SYNC_FOLDER_WARNING));
        let stale = opened_status(true, Some((9, 4)), true, false);
        assert!(stale.ends_with(&state::journal_stale_message(9, 4)));
        // A writable dossier: unchanged behaviour.
        assert_eq!(
            opened_status(false, None, false, false),
            state::MSG_JOURNAL_OPENED
        );
        assert_eq!(
            opened_status(false, None, false, true),
            state::MSG_JOURNAL_CREATED
        );
        assert_eq!(
            opened_status(false, None, true, true),
            state::MSG_SYNC_FOLDER_WARNING
        );
        assert_eq!(
            opened_status(false, Some((9, 4)), true, false),
            state::journal_stale_message(9, 4)
        );
    }
}
