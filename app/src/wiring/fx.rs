//! FX-rates wiring (Story 6.5, FR28): the Réglages « Taux de change » panel — the stored-rates
//! push, the user-initiated provider refresh (FR65 — never background-polled; #52 in-flight
//! guard), and the manual-entry intent. NO conversion is wired anywhere (consolidation = 6.6).

use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::state::JournalState;
use crate::wiring::fetch::resolve_provider_key;
use crate::wiring::Session;
use crate::{fetch, state};
use crate::{Fx, FxRateRow, MainWindow};

/// Push the stored rates into the `Fx` global (pair "EUR → CHF", exact rate spelling, day, source).
pub(crate) fn push_fx_rates(ui: &MainWindow, state: &JournalState) {
    let rows: Vec<FxRateRow> = state
        .list_fx_rates()
        .iter()
        .map(|r| FxRateRow {
            pair: format!("{} → {}", r.base_currency, r.quote_currency).into(),
            rate: r.rate.clone().into(),
            date: r.rate_date.clone().into(),
            source: r.source.clone().into(),
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
        ..
    } = s;
    // ── « Actualiser les taux » — one job, one pair per foreign currency in use (AC3). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let fetch_tx = fetch_tx.clone();
        ui.global::<Fx>().on_refresh_rates(move || {
            let ui = ui_weak.unwrap();
            let fx = ui.global::<Fx>();
            if fx.get_refreshing() {
                return; // the #52 double-click guard
            }
            let (provider, reference) = {
                let cfg = config.borrow();
                (cfg.preferred_provider, cfg.reference_currency_or_default())
            };
            if provider == crate::provider::ProviderChoice::None {
                fx.set_notice(state::MSG_PROVIDER_NONE.into());
                return;
            }
            let foreign = journal_state.borrow().foreign_currencies_in_use(&reference);
            if foreign.is_empty() {
                fx.set_notice(state::MSG_FX_NO_PAIRS.into());
                return;
            }
            let api_key = resolve_provider_key(provider);
            if provider.requires_key() && api_key.is_none() {
                fx.set_notice(state::MSG_PROVIDER_NO_KEY.into());
                return;
            }
            let pairs = foreign
                .into_iter()
                .map(|base| (base, reference.clone()))
                .collect();
            let request = fetch::FxRatesRequest {
                pairs,
                api_key,
                provider,
                // Captured at enqueue time (review): the outcome applies only to THIS journal
                // and is stamped with THIS provider, whatever changes mid-flight.
                journal_id: journal_state.borrow().journal_id(),
                source: provider.wire().to_string(),
            };
            // The flag latches ONLY on a successful send (review: a dead worker + a discarded
            // send error would otherwise disable the refresh for the whole session).
            if fetch_tx
                .send(fetch::WorkerJob::FetchFxRates(request))
                .is_ok()
            {
                fx.set_refreshing(true);
                fx.set_notice(state::MSG_FX_REFRESHING.into());
            } else {
                fx.set_notice(state::MSG_PROVIDER_OFFLINE.into());
            }
        });
    }
    // ── Manual entry (AC4): base, rate, date ("" = today); source = "manuel". ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
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
                }
                Err(message) => fx.set_notice(message.into()),
            }
            written
        });
    }
}
