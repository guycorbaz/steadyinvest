//! Provider-fetch wiring (Epic 3 + Story 4.4): the off-thread worker's outcome handler (study
//! fetch Stories 3.1/3.5, holdings price refresh Story 4.4/FR40, key test Story 3.2), the study
//! `fetch-provider` intent, and the Réglages key save/delete/test callbacks — plus the key
//! helpers: `resolve_provider_key` (OS keychain → env fallback, AC5/AC6) and
//! `mirror_provider_prefs` (status-only, the key value never crosses — NFR-S1). Moved verbatim
//! from `main.rs` — no logic change.

use std::rc::Rc;

use slint::{ComponentHandle, SharedString};
use uuid::Uuid;

use crate::provider::ProviderChoice;
use crate::wiring::holdings::{mark_holding_stale, refresh_holdings, HoldingFreshness};
use crate::wiring::push::{display_timestamp, push_form};
use crate::wiring::studies::refresh_studies;
use crate::wiring::Session;
use crate::{fetch, keychain, state};
use crate::{Holdings, MainWindow, Prefs, Studies};

/// The legacy interim key source (Story 3.1), kept ONLY as a fallback for environments with no
/// running OS secret agent (headless/NAS — AC5/AC6).
const ENV_KEY_FALLBACK: &str = "STEADYINVEST_EODHD_API_KEY";

/// Resolve the API key for a fetch/test (Story 3.2): the OS keychain first, then the env-var
/// fallback. `None` for a keyless provider or when no key is found anywhere. The key value is never
/// logged — only the fact that the fallback was used.
pub(crate) fn resolve_provider_key(provider: ProviderChoice) -> Option<String> {
    if !provider.requires_key() {
        return None;
    }
    match keychain::get_key(provider) {
        Ok(Some(key)) => return Some(key),
        Ok(None) => {}
        // Store unavailable (no agent) — fall through to the env fallback rather than fail (AC6).
        Err(_) => {}
    }
    // Trim consistently with the keychain path (which stores `key.trim()`), so a padded env value
    // doesn't fetch with stray whitespace the provider would reject (F9).
    let env_key = std::env::var(ENV_KEY_FALLBACK)
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty());
    if env_key.is_some() {
        tracing::info!(
            "provider key read from the {ENV_KEY_FALLBACK} fallback environment variable"
        );
    }
    env_key
}

/// Mirror the provider choice + keychain status into `Prefs` (Story 3.2). The key VALUE never
/// crosses — only the boolean "configured" status (NFR-S1). A store failure shows as "not
/// configured" plus a neutral notice (AC6).
pub(crate) fn mirror_provider_prefs(ui: &MainWindow, provider: ProviderChoice) {
    let prefs = ui.global::<Prefs>();
    prefs.set_provider(provider.wire().into());
    // A read failure (no secret agent) is reported as "not configured" — SILENTLY here: this runs at
    // startup and on provider switch, before the user has asked for anything. The explicit
    // save/delete/test actions surface `MSG_KEYCHAIN_UNAVAILABLE` when the user actually acts (AC6),
    // and a fetch still falls back to the env-var key.
    let configured = provider.requires_key() && keychain::has_key(provider).unwrap_or(false);
    prefs.set_key_configured(configured);
}

/// Wire the provider-fetch domain: install the worker's outcome handler (it must be set before
/// any job can be enqueued — the send sites are wired after it), then register the study
/// `fetch-provider` intent and the Réglages key save/delete/test callbacks.
pub(crate) fn wire_fetch(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        current_study,
        holding_freshness,
        holding_dismissed,
        refresh_pending,
        fetch_tx,
        ..
    } = s;
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let current_study = Rc::clone(current_study);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        let refresh_pending = Rc::clone(refresh_pending);
        fetch::set_outcome_handler(move |outcome| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let studies = ui.global::<Studies>();
            match outcome {
                fetch::WorkerOutcome::Fetch(outcome) => {
                    // Clear the in-progress flag ONLY for a study fetch — a key-test (which never
                    // sets `fetching`) must not re-enable the study Fetch button mid-fetch (F5).
                    studies.set_fetching(false);
                    // Re-render the open study (the stale murmur + degraded verdict show here) and
                    // refresh the dashboard — shared by the success, empty-payload, and failure arms.
                    let render_open = || {
                        let still_open = current_study
                            .borrow()
                            .as_deref()
                            .and_then(|s| Uuid::parse_str(s).ok())
                            == Some(outcome.study_id);
                        if still_open {
                            if let Some(study) = journal_state.borrow().get_study(outcome.study_id)
                            {
                                let format = config.borrow().number_format;
                                push_form(&ui, &journal_state.borrow(), &study, format);
                            }
                        }
                        refresh_studies(&ui, &journal_state.borrow());
                    };
                    match outcome.result {
                        // Story 3.5 / #46: a transport-success that returned ZERO usable years is
                        // "no data", NOT "no change" — report it honestly and flag last-known
                        // provider data stale (never apply an empty refresh as if nothing changed).
                        Ok(fetched) if fetched.canonical.years.is_empty() => {
                            studies.set_notice(state::MSG_PROVIDER_NO_DATA.into());
                            let _ = journal_state
                                .borrow_mut()
                                .mark_provider_stale(outcome.study_id);
                            render_open();
                        }
                        Ok(fetched) => {
                            let applied = journal_state
                                .borrow_mut()
                                .apply_provider_refresh(outcome.study_id, &fetched);
                            match applied {
                                Ok(report) => {
                                    // Name the recompute cause (FR29) and, after an annual update
                                    // (Story 3.6), the re-validation scope ("N à revérifier").
                                    studies.set_notice(state::refresh_summary(report).into());
                                    render_open();
                                }
                                Err(message) => studies.set_notice(message.into()),
                            }
                        }
                        // Story 3.5 (FR23/FR24/NFR-R1): name the cause, RETAIN last-known values, and
                        // flag the open study's provider cells stale (degrading the verdict). Never
                        // clear data; the user keeps working offline and retries later.
                        Err(error) => {
                            studies.set_notice(state::provider_failure_notice(&error).into());
                            let _ = journal_state
                                .borrow_mut()
                                .mark_provider_stale(outcome.study_id);
                            render_open();
                        }
                    }
                }
                fetch::WorkerOutcome::HoldingFetch(outcome) => {
                    // Story 4.4 (FR40): a holdings price-refresh result → the holdings surface (NOT
                    // the study screen). Success fills `current_price` (the §4 zone recomputes) +
                    // stamps a fresh `as_of`; a failure / no-data flags the ticker `périmé`, keeping
                    // its last-known zone visibly stale (AC4) — never a fresh-looking wrong zone.
                    let holdings = ui.global::<Holdings>();
                    let format = config.borrow().number_format;
                    let key = outcome.ticker.to_uppercase();
                    match outcome.result {
                        // The price arrived: fill `current_price` (price-only — never the yearly
                        // cells, issue #50) so the §4 zone recomputes, and stamp a fresh `as_of`.
                        Ok(Some(price)) => {
                            match journal_state
                                .borrow_mut()
                                .apply_holding_price(outcome.study_id, price)
                            {
                                Ok(()) => {
                                    // Story 4.5 (FR42): ratchet every same-ticker holding's stop level
                                    // up against the fresh price (a falling price writes nothing).
                                    let _ = journal_state
                                        .borrow_mut()
                                        .ratchet_trailing_stops_for_study(outcome.study_id, price);
                                    let now = display_timestamp(&journal_state.borrow().now());
                                    holding_freshness.borrow_mut().insert(
                                        key,
                                        HoldingFreshness {
                                            stale: false,
                                            as_of: Some(now),
                                        },
                                    );
                                    // Clear ONLY the in-progress banner — don't wipe a sibling
                                    // ticker's failure notice that resolved earlier. (Review F4.)
                                    if holdings.get_notice().as_str()
                                        == state::MSG_HOLDINGS_REFRESHING
                                    {
                                        holdings.set_notice(SharedString::new());
                                    }
                                }
                                // The study vanished / went read-only between enqueue and outcome:
                                // surface the cause AND flag the ticker stale, like the other failure
                                // arms (don't leave it falsely fresh). (Review F3.)
                                Err(message) => {
                                    holdings.set_notice(message.into());
                                    mark_holding_stale(&holding_freshness, &key);
                                }
                            }
                        }
                        // Transport-success but the provider exposed no current close → "no data":
                        // flag `périmé`, keep the last-known zone (AC4); never stamp "à jour" when the
                        // price the user asked to refresh did not come back.
                        Ok(None) => {
                            holdings.set_notice(state::MSG_PROVIDER_NO_DATA.into());
                            mark_holding_stale(&holding_freshness, &key);
                        }
                        Err(error) => {
                            holdings.set_notice(state::provider_failure_notice(&error).into());
                            mark_holding_stale(&holding_freshness, &key);
                        }
                    }
                    // Re-render the open study too (a holding may BE the open study — its §4 zone bar
                    // must reflect the just-filled current_price), then rebuild the register.
                    if current_study
                        .borrow()
                        .as_deref()
                        .and_then(|s| Uuid::parse_str(s).ok())
                        == Some(outcome.study_id)
                    {
                        if let Some(study) = journal_state.borrow().get_study(outcome.study_id) {
                            push_form(&ui, &journal_state.borrow(), &study, format);
                        }
                    }
                    refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                    // One job resolved — clear the in-flight latch when the batch is fully drained,
                    // re-enabling the refresh button (issue #52).
                    {
                        let mut pending = refresh_pending.borrow_mut();
                        *pending = pending.saturating_sub(1);
                        if *pending == 0 {
                            ui.global::<Holdings>().set_refreshing(false);
                        }
                    }
                }
                fetch::WorkerOutcome::TestKey(result) => {
                    // The key test (Story 3.2): a verdict, not study data. Surface it as the
                    // provider/key status in Réglages (cause-named on failure).
                    let prefs = ui.global::<Prefs>();
                    let status = match result {
                        Ok(()) => state::MSG_KEY_OK.to_string(),
                        Err(steadyinvest_ingestion::IngestionError::Provider(
                            steadyinvest_ingestion::ProviderError::InvalidOrAbsentKey,
                        )) => state::MSG_KEY_INVALID.to_string(),
                        // 403: the key is valid but the plan/account is not authorized (e.g. EODHD
                        // free tier excludes fundamentals) — say so honestly, not "key invalid".
                        Err(steadyinvest_ingestion::IngestionError::Provider(
                            steadyinvest_ingestion::ProviderError::Forbidden { .. },
                        )) => state::MSG_KEY_FORBIDDEN.to_string(),
                        Err(error) => {
                            state::MSG_PROVIDER_FAILED.replace("{cause}", &error.to_string())
                        }
                    };
                    prefs.set_provider_status(status.into());
                }
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let current_study = Rc::clone(current_study);
        let config = Rc::clone(config);
        let fetch_tx = fetch_tx.clone();
        ui.global::<Studies>().on_fetch_provider(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            // The open study (the demo keeps `current_study` None → no fetch on the demo).
            let Some(study_id) = current_study
                .borrow()
                .as_deref()
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                return;
            };
            let Some(ticker) = journal_state
                .borrow()
                .get_study(study_id)
                .map(|s| s.security_ticker)
            else {
                return;
            };
            // Key source (Story 3.2): the OS keychain for the preferred provider, with an env-var
            // fallback for environments that have no running secret agent (AC5/AC6). A keyless
            // provider fetches with no key; a key-requiring provider with no key found is refused.
            let provider_choice = config.borrow().preferred_provider;
            if provider_choice == ProviderChoice::None {
                studies.set_notice(state::MSG_PROVIDER_NONE.into());
                return;
            }
            let api_key = resolve_provider_key(provider_choice);
            if provider_choice.requires_key() && api_key.is_none() {
                studies.set_notice(state::MSG_PROVIDER_NO_KEY.into());
                return;
            }
            studies.set_fetching(true);
            studies.set_notice(state::MSG_PROVIDER_FETCHING.into());
            if fetch_tx
                .send(fetch::WorkerJob::Fetch(fetch::FetchRequest {
                    study_id,
                    ticker,
                    api_key,
                    provider: provider_choice,
                }))
                .is_err()
            {
                // The worker thread is gone (should never happen) — don't latch the in-progress
                // state, which would disable the button for the rest of the session (review P1).
                studies.set_fetching(false);
                studies.set_notice(
                    state::MSG_PROVIDER_FAILED
                        .replace("{cause}", "le service de récupération est indisponible")
                        .into(),
                );
            }
        });
    }

    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        ui.global::<Prefs>().on_key_saved(move |key| {
            let ui = ui_weak.unwrap();
            let prefs = ui.global::<Prefs>();
            let provider = config.borrow().preferred_provider;
            // A blank field is a no-op (mirror the `set_rationale` empty-is-nothing discipline).
            if key.trim().is_empty() {
                return;
            }
            match keychain::set_key(provider, key.trim()) {
                Ok(()) => {
                    prefs.set_key_configured(true);
                    prefs.set_provider_status(state::MSG_KEY_SAVED.into());
                }
                Err(_) => prefs.set_provider_status(state::MSG_KEYCHAIN_UNAVAILABLE.into()),
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        ui.global::<Prefs>().on_key_deleted(move || {
            let ui = ui_weak.unwrap();
            let prefs = ui.global::<Prefs>();
            let provider = config.borrow().preferred_provider;
            match keychain::delete_key(provider) {
                Ok(()) => {
                    prefs.set_key_configured(false);
                    prefs.set_provider_status(state::MSG_KEY_DELETED.into());
                }
                Err(_) => prefs.set_provider_status(state::MSG_KEYCHAIN_UNAVAILABLE.into()),
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let fetch_tx = fetch_tx.clone();
        ui.global::<Prefs>().on_key_tested(move || {
            let ui = ui_weak.unwrap();
            let prefs = ui.global::<Prefs>();
            let provider = config.borrow().preferred_provider;
            // A keyless provider has nothing to test.
            if !provider.requires_key() {
                prefs.set_provider_status(state::MSG_KEY_OK.into());
                return;
            }
            let api_key = resolve_provider_key(provider);
            if api_key.is_none() {
                prefs.set_provider_status(state::MSG_PROVIDER_NO_KEY.into());
                return;
            }
            // Off the UI thread: a minimal live fetch whose verdict returns via the outcome handler.
            prefs.set_provider_status(state::MSG_KEY_TESTING.into());
            if fetch_tx
                .send(fetch::WorkerJob::TestKey(fetch::TestKeyRequest {
                    api_key,
                    provider,
                }))
                .is_err()
            {
                prefs.set_provider_status(
                    state::MSG_PROVIDER_FAILED
                        .replace("{cause}", "le service de récupération est indisponible")
                        .into(),
                );
            }
        });
    }
}
