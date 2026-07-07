//! FX-rates wiring (Story 6.5, FR28): the Réglages « Taux de change » panel — the stored-rates
//! push, the user-initiated provider refresh (FR65 — never background-polled; #52 in-flight
//! guard), and the manual-entry intent. NO conversion is wired anywhere (consolidation = 6.6).

use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::state::JournalState;
use crate::wiring::Session;
use crate::{Fx, FxRateRow, MainWindow};
use crate::{fetch, state};

/// Push the stored rates into the `Fx` global (pair "EUR → CHF", exact rate spelling, day, source).
pub(crate) fn push_fx_rates(ui: &MainWindow, state: &JournalState) {
    let rows: Vec<FxRateRow> = state
        .list_fx_rates()
        .iter()
        .map(|r| FxRateRow {
            id: r.id.to_string().into(),
            pair: format!("{} → {}", r.base_currency, r.quote_currency).into(),
            rate: r.rate.clone().into(),
            date: r.rate_date.clone().into(),
            source: r.source.clone().into(),
            quote: r.quote_currency.clone().into(),
        })
        .collect();
    ui.global::<Fx>()
        .set_rates(ModelRc::new(VecModel::from(rows)));
}

/// Wire the FX domain: the provider refresh + the manual-entry form.
pub(crate) fn wire_fx(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        fetch_tx,
        fetch_cancel,
        holding_freshness,
        holding_dismissed,
        ..
    } = s;
    // ── « Actualiser les taux » — one job, one pair per foreign currency in use (AC3). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let fetch_tx = fetch_tx.clone();
        let fetch_cancel = std::sync::Arc::clone(fetch_cancel);
        ui.global::<Fx>().on_refresh_rates(move || {
            let ui = ui_weak.unwrap();
            let fx = ui.global::<Fx>();
            if fx.get_refreshing() {
                return; // the #52 double-click guard
            }
            let reference = config.borrow().reference_currency_or_default();
            let primary = config.borrow().preferred_provider;
            if primary == crate::provider::ProviderChoice::None {
                fx.set_notice(state::MSG_PROVIDER_NONE.into());
                return;
            }
            let foreign = journal_state.borrow().foreign_currencies_in_use(&reference);
            if foreign.is_empty() {
                fx.set_notice(state::MSG_FX_NO_PAIRS.into());
                return;
            }
            // Story 6.9 (FR26): the FX fallback chain, each member with its own key. The stamped
            // source is each pair's EFFECTIVE member, carried back per result.
            let chain = crate::wiring::fetch::resolve_chain(
                &config.borrow(),
                steadyinvest_ingestion::FieldKind::Fx,
            );
            if chain.is_empty() {
                fx.set_notice(state::MSG_PROVIDER_NO_KEY.into());
                return;
            }
            let pairs: Vec<(String, String)> = foreign
                .into_iter()
                .map(|base| (base, reference.clone()))
                .collect();
            let pair_count = pairs.len();
            let request = fetch::FxRatesRequest {
                pairs,
                chain,
                primary,
                // Captured at enqueue time (review): the outcome applies only to THIS journal,
                // whatever changes mid-flight.
                journal_id: journal_state.borrow().journal_id(),
            };
            // Issue #100: fresh batch — clear any prior cancel BEFORE the worker can pick the job up.
            fetch_cancel.store(false, std::sync::atomic::Ordering::Relaxed);
            // The flag latches ONLY on a successful send (review: a dead worker + a discarded
            // send error would otherwise disable the refresh for the whole session).
            if fetch_tx
                .send(fetch::WorkerJob::FetchFxRates(request))
                .is_ok()
            {
                fx.set_refreshing(true);
                fx.set_refresh_progress(format!("0 / {pair_count}").into()); // issue #100
                fx.set_notice(state::MSG_FX_REFRESHING.into());
            } else {
                fx.set_notice(state::MSG_PROVIDER_OFFLINE.into());
            }
        });
    }
    // ── Issue #100 — cancel the in-flight FX batch: raise the shared flag; the worker breaks after
    //    the current pair and returns the pairs done so far. ──
    {
        let fetch_cancel = std::sync::Arc::clone(fetch_cancel);
        ui.global::<Fx>().on_cancel_refresh(move || {
            fetch_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        });
    }
    // ── Manual entry (AC4): base, rate, date ("" = today); source = "manuel". ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Fx>().on_add_rate(move |base, rate, date| {
            let ui = ui_weak.unwrap();
            let reference = config.borrow().reference_currency_or_default();
            let result = journal_state
                .borrow_mut()
                .upsert_manual_fx_rate(&base, &rate, &date, &reference);
            let written = result.is_ok();
            let fx = ui.global::<Fx>();
            match result {
                Ok(()) => {
                    fx.set_notice(state::MSG_FX_RECORDED.into());
                    push_fx_rates(&ui, &journal_state.borrow());
                    // Story 6.6 (review): the Portefeuille consolidation block converts with
                    // these rates — a new rate must re-render it without a restart.
                    let format = config.borrow().number_format;
                    crate::wiring::holdings::refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                }
                Err(message) => fx.set_notice(message.into()),
            }
            written
        });
    }
    // ── Delete a stored rate by id (issue #90): the panel's repair path for a mis-entered rate or a
    //    row stranded against an old reference currency. Re-pushes the list and re-renders the
    //    consolidation (a removed rate can make a bank's subtotal go honestly ABSENT). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Fx>().on_delete_rate(move |id| {
            let ui = ui_weak.unwrap();
            let result = journal_state.borrow_mut().delete_fx_rate(&id);
            let fx = ui.global::<Fx>();
            match result {
                // Only a real removal states "supprimé"; a stale UI id (Ok(false)) says nothing.
                Ok(removed) => {
                    if removed {
                        fx.set_notice(state::MSG_FX_DELETED.into());
                    }
                    push_fx_rates(&ui, &journal_state.borrow());
                    let format = config.borrow().number_format;
                    crate::wiring::holdings::refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                }
                Err(message) => fx.set_notice(message.into()),
            }
        });
    }
}
