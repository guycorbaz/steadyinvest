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
use crate::wiring::holdings::{refresh_holdings, retain_held_freshness, HoldingFreshnessMap};
use crate::wiring::studies::refresh_studies;
use crate::wiring::watchlist::refresh_watchlist;
use crate::wiring::{persist, Session};
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
    current_study: &Rc<RefCell<Option<String>>>,
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

            let status = if let Some(seen) = stale_seen {
                state::journal_stale_message(seen, outcome.logical_version)
            } else if outcome.sync_warning {
                state::MSG_SYNC_FOLDER_WARNING.to_string()
            } else if created {
                state::MSG_JOURNAL_CREATED.to_string()
            } else {
                state::MSG_JOURNAL_OPENED.to_string()
            };
            prefs.set_journal_location_status(status.into());
            prefs.set_journal_reclaim_path("".into());

            // The whole journal changed — close any open study editor and re-render every surface.
            *current_study.borrow_mut() = None;
            ui.global::<Studies>().set_study_open(false);
            let st = journal_state.borrow();
            let format = config.borrow().number_format;
            retain_held_freshness(holding_freshness, &st);
            refresh_studies(ui, &st);
            refresh_watchlist(ui, &st);
            // Story 6.5 review: the FX panel follows the journal (rates are journal data); the
            // in-flight flag and sticky notice reset with it.
            crate::wiring::fx::push_fx_rates(ui, &st);
            ui.global::<crate::Fx>().set_refreshing(false);
            ui.global::<crate::Fx>()
                .set_notice(slint::SharedString::new());
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
                prefs.set_journal_location_status(notice.into());
                prefs.set_journal_reclaim_path("".into());
            }
        }
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
        current_study,
        ..
    } = s;
    // ── Story 5.3 (FR60) — export / import the WHOLE journal as a portable file. Scales the 5.2
    // envelope to every entity + the (journal_id, version, hash) identity tuple; import verifies and
    // applies atomically (never partially). Path-based for now — the native picker is Story 5.5. The
    // actions live in Réglages (the Prefs global). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Prefs>().on_export_journal(move || {
            let ui = ui_weak.unwrap();
            let state = journal_state.borrow();
            let notice = match state.export_journal() {
                Ok(json) => match state.journal_id() {
                    Some(jid) => match write_journal_export(jid, &json) {
                        Ok(path) => format!("{} {}", state::MSG_JOURNAL_EXPORTED, path.display()),
                        Err(e) => format!("{} {e}", state::MSG_SAVE_FAILED),
                    },
                    None => state::MSG_NO_JOURNAL.to_string(),
                },
                Err(message) => message,
            };
            ui.global::<Prefs>().set_journal_status(notice.into());
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
            let notice = match std::fs::read_to_string(path.as_str()) {
                Ok(json) => match journal_state.borrow_mut().import_journal(&json) {
                    Ok(summary) => state::journal_imported_message(&summary),
                    Err(message) => message,
                },
                // An unreadable path is the malformed/unreadable case — a neutral refusal, no panic.
                Err(_) => state::MSG_IMPORT_MALFORMED.to_string(),
            };
            ui.global::<Prefs>().set_journal_status(notice.into());
            // A whole-journal import can touch every surface — re-render them all (dashboard,
            // watchlist, portfolio). Prune any stale per-holding freshness for tickers no longer held.
            let state = journal_state.borrow();
            let format = config.borrow().number_format;
            retain_held_freshness(&holding_freshness, &state);
            refresh_studies(&ui, &state);
            refresh_watchlist(&ui, &state);
            // Story 6.5 review: imported/restored fx_rates must show without a restart.
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

    // ── Story 5.4 (FR61) — backup / restore the raw .db. Create a self-contained .db backup; validate
    // a candidate backup (integrity + schema-version + identity) BEFORE any overwrite, surface its
    // (journal_id, version) + a stale/foreign warning, and apply only on explicit confirm (never
    // silently). Path-based for now — the native picker is Story 5.5. ──
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
                    prefs.set_restore_confirm(state::restore_confirm_message(&assessment).into());
                    prefs.set_restore_status("".into());
                }
                // A hard refusal — show the cause, no banner.
                Err(message) => {
                    prefs.set_restore_confirm("".into());
                    prefs.set_restore_status(message.into());
                }
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        let current_study = Rc::clone(current_study);
        ui.global::<Prefs>().on_confirm_restore(move || {
            let ui = ui_weak.unwrap();
            let result = journal_state.borrow_mut().confirm_restore();
            let prefs = ui.global::<Prefs>();
            prefs.set_restore_confirm("".into());
            // A successful restore replaces the whole journal — close any open study editor first so a
            // stale in-memory form can't be saved back into the restored journal (an old study_id would
            // otherwise be written into the new journal).
            if result.is_ok() {
                *current_study.borrow_mut() = None;
                ui.global::<Studies>().set_study_open(false);
            }
            let notice = match result {
                Ok(()) => state::MSG_RESTORE_DONE.to_string(),
                Err(message) => message,
            };
            prefs.set_restore_status(notice.into());
            // The whole journal changed — re-render every surface.
            let state = journal_state.borrow();
            let format = config.borrow().number_format;
            retain_held_freshness(&holding_freshness, &state);
            refresh_studies(&ui, &state);
            refresh_watchlist(&ui, &state);
            // Story 6.5 review: imported/restored fx_rates must show without a restart.
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
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Prefs>().on_cancel_restore(move || {
            let ui = ui_weak.unwrap();
            journal_state.borrow_mut().cancel_restore();
            ui.global::<Prefs>().set_restore_confirm("".into());
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
        let current_study = Rc::clone(current_study);
        ui.global::<Prefs>().on_pick_and_open_journal(move || {
            let ui = ui_weak.unwrap();
            let Some(path) = rfd::FileDialog::new()
                .set_title("Ouvrir un journal")
                .add_filter("journal", &["db"])
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
                &current_study,
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
        let current_study = Rc::clone(current_study);
        ui.global::<Prefs>().on_pick_and_create_journal(move || {
            let ui = ui_weak.unwrap();
            let Some(path) = rfd::FileDialog::new()
                .set_title("Créer un journal")
                .add_filter("journal", &["db"])
                .set_file_name("journal.db")
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
                &current_study,
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
        let current_study = Rc::clone(current_study);
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
                &current_study,
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
        let current_study = Rc::clone(current_study);
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
                &current_study,
            );
        });
    }
}
