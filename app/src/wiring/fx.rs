//! FX-rates wiring (Story 6.5, FR28): the Réglages « Taux de change » panel — the stored-rates
//! push, the user-initiated provider refresh (FR65 — never background-polled; #52 in-flight
//! guard), and the manual-entry intent. NO conversion is wired anywhere (consolidation = 6.6).

use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::state::JournalState;
use crate::wiring::Session;
use crate::{Fx, FxRateRow, MainWindow};
use crate::{fetch, state};

/// Push the stored rates into the `Fx` global (pair "EUR → CHF", the exact rate in the user's
/// number format — G1 I —, day, source).
pub(crate) fn push_fx_rates(ui: &MainWindow, state: &JournalState) {
    let format = state.number_format();
    // G1 final review (M5): a failed read is « indisponible », never an empty list.
    let (rates, unavailable) = match state.try_list_fx_rates() {
        Ok(rates) => (rates, false),
        Err(_) => (Vec::new(), true),
    };
    ui.global::<Fx>().set_rates_unavailable(unavailable);
    let rows: Vec<FxRateRow> = rates
        .iter()
        .map(|r| FxRateRow {
            id: r.id.to_string().into(),
            pair: format!("{} → {}", r.base_currency, r.quote_currency).into(),
            rate: crate::viewmodel::format::format_amount(&r.rate, format).into(),
            date: r.rate_date.clone().into(),
            source: r.source.clone().into(),
            quote: r.quote_currency.clone().into(),
        })
        .collect();
    ui.global::<Fx>()
        .set_rates(ModelRc::new(VecModel::from(rows)));
}

/// The FR28 footnote of a converted surface: every rate used, as pure data entries — pair, the
/// exact rate in the user's number format (G1 final review: never a raw « 0.8 » beside the
/// surface's « 0,8 »), then "(date, source)" — joined by « · ». No prose baked into Rust (posture:
/// the @tr scan cannot see it); the surrounding sentence lives in Slint.
pub(crate) fn rate_notes(
    rates: &[steadyinvest_persistence::FxRateItem],
    format: crate::viewmodel::format::NumberFormat,
) -> String {
    rates
        .iter()
        .map(|r| {
            format!(
                "{} → {} {} ({}, {})",
                r.base_currency,
                r.quote_currency,
                crate::viewmodel::format::format_amount(&r.rate, format),
                r.rate_date,
                r.source
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
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
                crate::wiring::dialog::refuse(&ui, state::MSG_PROVIDER_NONE);
                return;
            }
            let foreign = journal_state.borrow().foreign_currencies_in_use(&reference);
            if foreign.is_empty() {
                crate::wiring::dialog::refuse(&ui, state::MSG_FX_NO_PAIRS);
                return;
            }
            // Story 6.9 (FR26): the FX fallback chain, each member with its own key. The stamped
            // source is each pair's EFFECTIVE member, carried back per result.
            let chain = crate::wiring::fetch::resolve_chain(
                &config.borrow(),
                steadyinvest_ingestion::FieldKind::Fx,
            );
            if chain.is_empty() {
                crate::wiring::dialog::refuse(&ui, state::MSG_PROVIDER_NO_KEY);
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
                crate::wiring::dialog::refuse(&ui, state::MSG_PROVIDER_OFFLINE);
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
                Err(message) => crate::wiring::dialog::refuse(&ui, &message),
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
                Err(message) => crate::wiring::dialog::refuse(&ui, &message),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::rate_notes;
    use crate::viewmodel::format::NumberFormat;

    #[test]
    fn rate_notes_spell_the_rate_in_the_users_number_format() {
        let eur = steadyinvest_persistence::FxRateItem {
            id: uuid::Uuid::nil(),
            base_currency: "EUR".to_string(),
            quote_currency: "CHF".to_string(),
            rate: "0.8".to_string(),
            rate_date: "2026-09-01".to_string(),
            source: "manuel".to_string(),
            created_at: steadyinvest_contract::Timestamp("2026-09-01T00:00:00Z".to_string()),
        };
        let usd = steadyinvest_persistence::FxRateItem {
            base_currency: "USD".to_string(),
            rate: "0.91".to_string(),
            ..eur.clone()
        };
        assert_eq!(
            rate_notes(&[eur.clone(), usd], NumberFormat::Comma),
            "EUR → CHF 0,8 (2026-09-01, manuel) · USD → CHF 0,91 (2026-09-01, manuel)"
        );
        assert_eq!(
            rate_notes(std::slice::from_ref(&eur), NumberFormat::Point),
            "EUR → CHF 0.8 (2026-09-01, manuel)"
        );
        assert_eq!(rate_notes(&[], NumberFormat::Comma), "");
    }
}
