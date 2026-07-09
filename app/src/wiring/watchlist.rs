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
    let by_id: std::collections::HashMap<Uuid, String> = state
        .list_studies()
        .into_iter()
        .map(|s| (s.id, s.security_ticker))
        .collect();
    let items = state.list_watch_items();
    let mut in_buy_zone_count = 0i32;
    let rows: Vec<WatchRow> = items
        .iter()
        .map(|w| {
            // Story 4.2: a linked study whose current price is in its §4 buy zone flags a neutral
            // alert (unlinked entries are never in a zone). Issue #48: a price BELOW the recorded
            // band is its own neutral fact — mutually exclusive with the zone by construction.
            let (in_buy_zone, below_band) =
                w.study_id
                    .and_then(|sid| state.get_study(sid))
                    .map_or((false, false), |study| {
                        (
                            viewmodel::engine::study_in_buy_zone(&study),
                            viewmodel::engine::study_below_forecast_band(&study),
                        )
                    });
            if in_buy_zone {
                in_buy_zone_count += 1;
            }
            WatchRow {
                id: w.id.to_string().into(),
                ticker: w.security_ticker.clone().into(),
                // `linked` is authoritative (the cell carries a study_id); `study_link` is the
                // resolved ticker for display (may be "" if the linked study no longer resolves).
                linked: w.study_id.is_some(),
                study_link: w
                    .study_id
                    .and_then(|sid| by_id.get(&sid))
                    .cloned()
                    .unwrap_or_default()
                    .into(),
                in_buy_zone,
                below_band,
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
        Err(message) => watchlist.set_notice(message.into()),
    }
    refresh_watchlist(ui, state);
}

/// Link a watchlist entry to a saved study of the SAME ticker (the most recent), or a neutral
/// "no study for this ticker" notice if none exists (Story 4.1 — an explicit picker is a later
/// refinement; auto-match by ticker covers the common case).
fn link_watch_to_same_ticker_study(state: &mut JournalState, id: Uuid) -> Result<(), String> {
    let Some(item) = state.list_watch_items().into_iter().find(|w| w.id == id) else {
        return Ok(()); // entry gone — nothing to link
    };
    match state.study_id_for_ticker(&item.security_ticker) {
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
            apply_watch_result(&ui, &journal_state.borrow(), result);
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
