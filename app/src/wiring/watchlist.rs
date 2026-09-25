//! Watchlist wiring (Story 4.1, FR34 + Story 4.2 buy-zone alerts): the add / remove / move /
//! link / unlink intents, the `refresh_watchlist` re-render (rows ordered by position, each
//! resolving its optional study link + §4 buy-zone flag), and the same-ticker auto-link helper.
//! Moved verbatim from `main.rs` — no logic change.

use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use uuid::Uuid;

use crate::state::JournalState;
use crate::wiring::Session;
use crate::{MainWindow, WatchRow, Watchlist};
use crate::{state, viewmodel};

/// Rebuild the watchlist surface from persistence (Story 4.1): rows ordered by position, each
/// resolving its optional study link to that study's ticker (the buy-zone source for Story 4.2).
pub(crate) fn refresh_watchlist(ui: &MainWindow, state: &JournalState) {
    let watchlist = ui.global::<Watchlist>();
    // G1 final review (M5): a failed read is « indisponible », never the empty state.
    let (items, unavailable) = match state.try_list_watch_items() {
        Ok(items) => (items, false),
        Err(_) => (Vec::new(), true),
    };
    watchlist.set_unavailable(unavailable);
    let mut in_buy_zone_count = 0i32;
    let rows: Vec<WatchRow> = items
        .iter()
        .map(|w| {
            // G1 final review (L6): the link resolves by the study's OWN read — a failure (or a
            // link that no longer resolves) is « Étude indisponible », never a dangling « Étude : »
            // and never a silent absence of the zone fact.
            let study = w
                .study_id
                .map(|sid| state.try_get_study(sid).ok().flatten());
            let study_unavailable = matches!(study, Some(None));
            let study = study.flatten();
            // Story 4.2: a linked study whose current price is in its §4 buy zone flags a neutral
            // alert (unlinked entries are never in a zone). Issue #48: a price BELOW the recorded
            // band is its own neutral fact — mutually exclusive with the zone by construction.
            let (in_buy_zone, below_band) = study.as_ref().map_or((false, false), |study| {
                (
                    viewmodel::engine::study_in_buy_zone(study),
                    viewmodel::engine::study_below_forecast_band(study),
                )
            });
            if in_buy_zone {
                in_buy_zone_count += 1;
            }
            WatchRow {
                id: w.id.to_string().into(),
                ticker: w.security_ticker.clone().into(),
                // `linked` is authoritative (the cell carries a study_id); `study_link` is the
                // resolved study's ticker for display.
                linked: w.study_id.is_some(),
                study_link: study
                    .as_ref()
                    .map(|s| s.security_ticker.clone())
                    .unwrap_or_default()
                    .into(),
                in_buy_zone,
                below_band,
                study_unavailable,
            }
        })
        .collect();
    watchlist.set_count(items.len() as i32);
    watchlist.set_in_buy_zone_count(in_buy_zone_count);
    watchlist.set_rows(ModelRc::new(VecModel::from(rows)));
    watchlist.set_read_only(state.is_read_only());

    // Story 6.8 (FR48): an OPEN candidates panel re-syncs on watchlist mutations too — the
    // candidates ARE the watchlist (a closed panel costs nothing).
    crate::wiring::replacement::sync_candidates(ui, state);
}

/// Surface a watchlist write's outcome (neutral notice on refusal) and re-render the list.
fn apply_watch_result(ui: &MainWindow, state: &JournalState, result: Result<(), String>) {
    let watchlist = ui.global::<Watchlist>();
    match result {
        Ok(()) => watchlist.set_notice(SharedString::new()),
        // The UX pass: a refusal is acknowledged in the modal dialog, never a caption line.
        Err(message) => crate::wiring::dialog::refuse(ui, &message),
    }
    refresh_watchlist(ui, state);
}

/// Link a watchlist entry to a saved study of the SAME ticker (the most recent), or a neutral
/// "no study for this ticker" notice if none exists (Story 4.1 — an explicit picker is a later
/// refinement; auto-match by ticker covers the common case).
fn link_watch_to_same_ticker_study(state: &mut JournalState, id: Uuid) -> Result<(), String> {
    let Some(item) = state
        .try_list_watch_items()
        .map_err(|_| state::MSG_WATCH_LINK_LIST_UNREADABLE.to_string())?
        .into_iter()
        .find(|w| w.id == id)
    else {
        return Ok(()); // entry gone — nothing to link
    };
    // G1 final review (L6): a failed study read is named — never « aucune étude pour ce symbole ».
    match state
        .try_study_id_for_ticker(&item.security_ticker)
        .map_err(|_| state::MSG_WATCH_STUDY_UNAVAILABLE.to_string())?
    {
        Some(sid) => state.update_watch_item(id, &item.security_ticker, Some(sid)),
        None => Err(state::MSG_WATCH_NO_STUDY.to_string()),
    }
}

/// Wire the watchlist domain: add / remove / move / link / unlink, each persisted then
/// re-rendered with a neutral notice on refusal.
pub(crate) fn wire_watchlist(ui: &MainWindow, s: &Session) {
    let Session { journal_state, .. } = s;
    // ── Watchlist intents (Story 4.1, FR34) ── add / remove / move / link / unlink, each persisted
    // then re-rendered with a neutral notice on refusal. The link callbacks attach/clear a
    // same-ticker saved study (its buy zone — the seam Story 4.2 reads).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Watchlist>().on_add_watch(move |ticker| {
            let ui = ui_weak.unwrap();
            let result = journal_state.borrow_mut().add_watch_item(&ticker, None);
            let written = result.is_ok();
            apply_watch_result(&ui, &journal_state.borrow(), result);
            written
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Watchlist>().on_remove_watch(move |id| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id) else {
                return;
            };
            let result = journal_state.borrow_mut().delete_watch_item(id);
            apply_watch_result(&ui, &journal_state.borrow(), result);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Watchlist>().on_move_watch(move |id, up| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id) else {
                return;
            };
            let result = journal_state.borrow_mut().move_watch_item(id, up);
            apply_watch_result(&ui, &journal_state.borrow(), result);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Watchlist>().on_link_watch(move |id| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id) else {
                return;
            };
            let result = link_watch_to_same_ticker_study(&mut journal_state.borrow_mut(), id);
            apply_watch_result(&ui, &journal_state.borrow(), result);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Watchlist>().on_unlink_watch(move |id| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id) else {
                return;
            };
            // Clear the link by re-saving the entry's ticker with no study.
            let ticker = journal_state
                .borrow()
                .list_watch_items()
                .into_iter()
                .find(|w| w.id == id)
                .map(|w| w.security_ticker);
            let result = match ticker {
                Some(t) => journal_state.borrow_mut().update_watch_item(id, &t, None),
                None => Ok(()),
            };
            apply_watch_result(&ui, &journal_state.borrow(), result);
        });
    }
}
