//! Provider-fetch wiring (Epic 3 + Story 4.4): the off-thread worker's outcome handler (study
//! fetch Stories 3.1/3.5, holdings price refresh Story 4.4/FR40, key test Story 3.2), the study
//! `fetch-provider` intent, and the Réglages key save/delete/test callbacks — plus the key
//! helpers: `resolve_provider_key` (OS keychain → env fallback, AC5/AC6) and
//! `mirror_provider_prefs` (status-only, the key value never crosses — NFR-S1). Moved verbatim
//! from `main.rs` — no logic change.

use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use uuid::Uuid;

use crate::provider::ProviderChoice;
use crate::wiring::Session;
use crate::wiring::holdings::{HoldingFreshness, mark_holding_stale, refresh_holdings};
use crate::wiring::push::{display_timestamp, push_form};
use crate::wiring::studies::refresh_studies;
use crate::{Fx, Holdings, MainWindow, Prefs, Studies};
use crate::{fetch, keychain, state};

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

/// Build the SHIPPED fallback chain for `field` (Story 6.9, FR26): the config's effective chain
/// with each member's OWN key resolved (the keychain slots are per-provider); a keyed member
/// without a key is dropped here — the worker must never send a guaranteed-401 request. An empty
/// result with a configured primary means "no usable key anywhere" (the caller surfaces
/// `MSG_PROVIDER_NO_KEY`, the unchanged 3.2 semantics).
pub(crate) fn resolve_chain(
    config: &crate::config::AppConfig,
    field: steadyinvest_ingestion::FieldKind,
) -> Vec<fetch::ChainMember> {
    config
        .provider_chain(field)
        .into_iter()
        .filter_map(|provider| {
            let api_key = resolve_provider_key(provider);
            if provider.requires_key() && api_key.is_none() {
                return None;
            }
            Some(fetch::ChainMember { provider, api_key })
        })
        .collect()
}

/// Issue #101: the configured FALLBACK for `field` that is INACTIVE because its provider needs a key
/// but none is stored — so [`resolve_chain`] silently dropped it and the shipped chain is a single
/// member. A quota/outage then fails outright with the failover the user configured never running.
/// `None` when the fallback is absent, keyless-capable, or has its key. Used to name the skipped
/// fallback in a fetch-failure notice, so the outright failure is not a mystery.
pub(crate) fn configured_fallback_missing_key(
    config: &crate::config::AppConfig,
    field: steadyinvest_ingestion::FieldKind,
) -> Option<crate::provider::ProviderChoice> {
    let fallback = config.fallback_provider_or_none(field)?;
    (fallback.requires_key() && !keychain::has_key(fallback).unwrap_or(false)).then_some(fallback)
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
    // Issue #44 (F10): a locked/unavailable store must read as "can't tell", NOT a false "no key
    // configured" — the tri-state (`key-status-unknown`) drives an honest third status line.
    let (configured, unknown) = if provider.requires_key() {
        match keychain::has_key(provider) {
            Ok(present) => (present, false),
            Err(_) => (false, true),
        }
    } else {
        (false, false)
    };
    prefs.set_key_configured(configured);
    prefs.set_key_status_unknown(unknown);
    // Issue #38: surface any ORPHANED key — a key-using provider that is NOT the active one but still
    // holds a stored key — so it can be removed even while "Aucun" is selected (NFR-S1). Only the
    // presence boolean is read; the key value never crosses.
    prefs.set_orphan_keys(ModelRc::new(VecModel::from(orphan_key_rows(provider))));
}

/// The key-using providers that are NOT `active` yet still hold a stored key (issue #38) — an
/// unavailable/locked store yields `false`, so we never list an orphan we cannot confirm.
fn orphan_key_rows(active: ProviderChoice) -> Vec<crate::StoredKeyRow> {
    crate::provider::KEYED_PROVIDERS
        .into_iter()
        .filter(|p| *p != active && keychain::has_key(*p).unwrap_or(false))
        .map(|p| crate::StoredKeyRow {
            wire: p.wire().into(),
            display: p.display_name().into(),
        })
        .collect()
}

/// Wire the provider-fetch domain: install the worker's outcome handler (it must be set before
/// any job can be enqueued — the send sites are wired after it), then register the study
/// `fetch-provider` intent and the Réglages key save/delete/test callbacks.
/// Issue #100: advance the holdings-refresh batch by one resolved (or cancel-skipped) job — decrement
/// the pending counter, update the "done / total" progress caption, and clear the "refreshing" latch
/// when the batch fully drains (naming a cancellation if the flag is still raised).
fn advance_holding_batch(
    ui: &MainWindow,
    refresh_pending: &Rc<std::cell::RefCell<usize>>,
    refresh_total: &Rc<std::cell::RefCell<usize>>,
    fetch_cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let holdings = ui.global::<Holdings>();
    let mut pending = refresh_pending.borrow_mut();
    *pending = pending.saturating_sub(1);
    let total = *refresh_total.borrow();
    if *pending == 0 {
        holdings.set_refreshing(false);
        holdings.set_refresh_progress(SharedString::new());
        if fetch_cancel.load(std::sync::atomic::Ordering::Relaxed) {
            holdings.set_notice(state::MSG_REFRESH_CANCELLED.into());
        }
    } else {
        let done = total.saturating_sub(*pending);
        holdings.set_refresh_progress(format!("{done} / {total}").into());
    }
}

pub(crate) fn wire_fetch(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        current_study,
        holding_freshness,
        holding_dismissed,
        refresh_pending,
        refresh_total,
        fetch_cancel,
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
        let refresh_total = Rc::clone(refresh_total);
        let fetch_cancel = std::sync::Arc::clone(fetch_cancel);
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
                        if still_open
                            && let Some(study) = journal_state.borrow().get_study(outcome.study_id)
                        {
                            let format = config.borrow().number_format;
                            push_form(&ui, &journal_state.borrow(), &study, format);
                        }
                        refresh_studies(&ui, &journal_state.borrow());
                    };
                    match outcome.result {
                        // Story 3.5 / #46: a transport-success that returned ZERO usable years is
                        // "no data", NOT "no change" — report it honestly and flag last-known
                        // provider data stale (never apply an empty refresh as if nothing changed).
                        Ok(fetched) if fetched.canonical.years.is_empty() => {
                            tracing::warn!(study_id = %outcome.study_id, "study fetch returned no usable years (no data)");
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
                                    tracing::info!(study_id = %outcome.study_id, "study fetch applied");
                                    // Name the recompute cause (FR29) and, after an annual update
                                    // (Story 3.6), the re-validation scope ("N à revérifier").
                                    // Story 6.9 (FR26): a fallback names itself alongside.
                                    let mut notice = state::refresh_summary(report);
                                    if let Some(effective) = outcome.fell_back_to {
                                        notice = format!(
                                            "{notice} {}",
                                            state::provider_fallback_notice(effective)
                                        );
                                    }
                                    studies.set_notice(notice.into());
                                    render_open();
                                }
                                Err(message) => studies.set_notice(message.into()),
                            }
                        }
                        // Story 3.5 (FR23/FR24/NFR-R1): name the cause, RETAIN last-known values, and
                        // flag the open study's provider cells stale (degrading the verdict). Never
                        // clear data; the user keeps working offline and retries later.
                        Err(error) => {
                            // Look up the ticker for a diagnosable log line (the FetchOutcome carries
                            // only the study_id). The borrow drops at the `;`, before the borrow_mut
                            // below — the RefCell lesson.
                            let ticker = journal_state
                                .borrow()
                                .get_study(outcome.study_id)
                                .map(|s| s.security_ticker)
                                .unwrap_or_default();
                            tracing::warn!(%ticker, error = %error, "study fetch failed");
                            // Issue #101: if a configured fallback was skipped for a missing key, the
                            // failover the user set up never ran — name it so the outright failure is
                            // not a mystery ("I configured a backup, why did it just fail?").
                            let mut notice = state::provider_failure_notice(&error).to_string();
                            if let Some(fallback) = configured_fallback_missing_key(
                                &config.borrow(),
                                steadyinvest_ingestion::FieldKind::Fundamentals,
                            ) {
                                notice.push(' ');
                                notice.push_str(&state::fallback_no_key_notice(fallback));
                            }
                            studies.set_notice(notice.into());
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
                        Ok(Some(dated)) => {
                            // Issue #72: the close carries its real trading-session date — passed to
                            // `apply_holding_price` so the confront cache keys by the session, not the
                            // refresh day; the price alone drives the zone + the stop ratchet.
                            let price = dated.close;
                            let session_date = dated.session_date;
                            // Bind the result to a local so the `borrow_mut()` RefMut is DROPPED
                            // before the arms run — temporaries in a `match` scrutinee otherwise
                            // live for the whole `match`, so the `borrow_mut()` (ratchet) and
                            // `borrow()` (now / re-render) below would panic "RefCell already
                            // borrowed" on every successful price refresh.
                            let applied = journal_state.borrow_mut().apply_holding_price(
                                outcome.study_id,
                                price,
                                session_date,
                            );
                            match applied {
                                Ok(()) => {
                                    tracing::info!(ticker = %outcome.ticker, "price refresh: quote applied");
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
                                    // Story 6.9 (FR26): a fallback names itself — but NEVER over
                                    // a sibling ticker's failure notice (the F4 rule holds for
                                    // the new notice too, 2026-07-03 review): it only replaces
                                    // the in-progress banner or an empty slot.
                                    let current = holdings.get_notice();
                                    let replaceable = current.is_empty()
                                        || current.as_str() == state::MSG_HOLDINGS_REFRESHING;
                                    if let Some(effective) = outcome.fell_back_to {
                                        if replaceable {
                                            holdings.set_notice(
                                                state::provider_fallback_notice(effective).into(),
                                            );
                                        }
                                    } else if current.as_str() == state::MSG_HOLDINGS_REFRESHING {
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
                            tracing::warn!(ticker = %outcome.ticker, "price refresh: provider returned no quote");
                            holdings.set_notice(state::MSG_PROVIDER_NO_DATA.into());
                            mark_holding_stale(&holding_freshness, &key);
                        }
                        Err(error) => {
                            tracing::warn!(ticker = %outcome.ticker, error = %error, "price refresh failed");
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
                        && let Some(study) = journal_state.borrow().get_study(outcome.study_id)
                    {
                        push_form(&ui, &journal_state.borrow(), &study, format);
                    }
                    refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                    // One job resolved — advance the batch counter, clear the latch when fully drained.
                    advance_holding_batch(&ui, &refresh_pending, &refresh_total, &fetch_cancel);
                }
                fetch::WorkerOutcome::HoldingSkipped => {
                    // Issue #100: a cancelled per-ticker job the worker drained without fetching — it
                    // only advances the batch counter (no price applied, no re-render needed).
                    advance_holding_batch(&ui, &refresh_pending, &refresh_total, &fetch_cancel);
                }
                fetch::WorkerOutcome::FxProgress { done, total } => {
                    // Issue #100: mid-batch FX progress — count up instead of a frozen banner.
                    ui.global::<Fx>()
                        .set_refresh_progress(format!("{done} / {total}").into());
                }
                fetch::WorkerOutcome::FxRates {
                    journal_id,
                    results,
                    fell_back_to,
                } => {
                    // Story 6.5 (FR28) + review: the outcome only applies to the journal that
                    // ASKED (an in-flight journal switch must not write phantom rates into the
                    // new one). Story 6.9 (FR26): each pair's stamped source is its EFFECTIVE
                    // chain member — the provider that actually fetched, never the primary's
                    // name on a fallback's data.
                    let fx = ui.global::<Fx>();
                    fx.set_refreshing(false);
                    fx.set_refresh_progress(SharedString::new()); // issue #100: clear the counter
                    if journal_state.borrow().journal_id() != journal_id {
                        fx.set_notice(state::MSG_FX_JOURNAL_CHANGED.into());
                        return;
                    }
                    let total = results.len();
                    let mut landed = 0usize;
                    let mut failure: Option<String> = None;
                    for outcome in results {
                        match outcome.result {
                            Ok(Some(rate)) => {
                                match journal_state.borrow_mut().apply_fx_fetch(
                                    &outcome.base,
                                    &outcome.quote,
                                    rate,
                                    &outcome.effective,
                                ) {
                                    Ok(()) => landed += 1,
                                    // An app-side refusal (read-only, invalid) is a cause too —
                                    // never a bare "0/N" (review).
                                    Err(message) => {
                                        failure.get_or_insert(message);
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(error) => {
                                tracing::warn!(base = %outcome.base, quote = %outcome.quote, error = %error, "fx rate fetch failed");
                                failure.get_or_insert_with(|| {
                                    state::provider_failure_notice(&error).to_string()
                                });
                            }
                        }
                    }
                    // The count ALWAYS shows; the first failure cause rides along whenever one
                    // exists (a partial failure must name itself — review); a fallback names
                    // itself too (Story 6.9).
                    let mut notice = state::fx_refreshed_message(landed, total);
                    if let Some(cause) = failure {
                        notice = format!("{notice} {cause}");
                    }
                    if let Some(effective) = fell_back_to {
                        notice = format!("{notice} {}", state::provider_fallback_notice(effective));
                    }
                    // Issue #100: a cancelled batch names itself (the pairs done so far still applied).
                    if fetch_cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        notice = format!("{} {notice}", state::MSG_REFRESH_CANCELLED);
                    }
                    fx.set_notice(notice.into());
                    crate::wiring::fx::push_fx_rates(&ui, &journal_state.borrow());
                    // Story 6.6 (review): freshly fetched rates feed the consolidation block —
                    // re-render the register so it converts without a restart.
                    let format = config.borrow().number_format;
                    refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                }
                fetch::WorkerOutcome::TestKey(result) => {
                    // The key test (Story 3.2): a verdict, not study data. Surface it as the
                    // provider/key status in Réglages (cause-named on failure).
                    match &result {
                        Ok(()) => tracing::info!("provider key test succeeded"),
                        Err(error) => tracing::warn!(error = %error, "provider key test failed"),
                    }
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
                        // Issue #42: a quota reply PROVES the provider accepted the key (it ran the
                        // request and hit the rate limit) — acceptance, never a rejected key.
                        Err(steadyinvest_ingestion::IngestionError::Provider(
                            steadyinvest_ingestion::ProviderError::Quota { .. },
                        )) => state::MSG_KEY_OK_QUOTA.to_string(),
                        // Issue #42: a network failure never reached the provider — the key is neither
                        // confirmed nor refused (inconclusive), so it must not read as "clé invalide".
                        Err(steadyinvest_ingestion::IngestionError::Provider(
                            steadyinvest_ingestion::ProviderError::Network { .. },
                        )) => state::MSG_KEY_TEST_INCONCLUSIVE.to_string(),
                        Err(error) => {
                            state::MSG_PROVIDER_FAILED.replace("{cause}", &error.to_string())
                        }
                    };
                    prefs.set_provider_status(status.into());
                    // Issue #40: the test resolved — clear the in-flight flag so the panel's buttons
                    // re-enable (whatever the verdict).
                    prefs.set_key_testing(false);
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
            // Story 6.9 (FR26): the FUNDAMENTALS fallback chain, each member with its own key
            // (Story 3.2 keychain + env fallback). No primary → the provider guard; a configured
            // primary whose whole chain lacks keys → the key guard (unchanged semantics).
            if config.borrow().preferred_provider == ProviderChoice::None {
                studies.set_notice(state::MSG_PROVIDER_NONE.into());
                return;
            }
            let chain = resolve_chain(
                &config.borrow(),
                steadyinvest_ingestion::FieldKind::Fundamentals,
            );
            if chain.is_empty() {
                studies.set_notice(state::MSG_PROVIDER_NO_KEY.into());
                return;
            }
            studies.set_fetching(true);
            studies.set_notice(state::MSG_PROVIDER_FETCHING.into());
            let primary = config.borrow().preferred_provider;
            tracing::info!(ticker = %ticker, provider = primary.wire(), "study fetch requested");
            if fetch_tx
                .send(fetch::WorkerJob::Fetch(fetch::FetchRequest {
                    study_id,
                    ticker,
                    chain,
                    primary,
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
            // Issue #41: a blank/whitespace field is a no-op, but say so — a silent return read as a
            // successful save (the field cleared either way). Return `false` so the UI KEEPS the
            // field (nothing to clear) and shows the neutral notice.
            if key.trim().is_empty() {
                prefs.set_provider_status(state::MSG_KEY_BLANK.into());
                return false;
            }
            match keychain::set_key(provider, key.trim()) {
                Ok(()) => {
                    prefs.set_key_configured(true);
                    prefs.set_provider_status(state::MSG_KEY_SAVED.into());
                    true
                }
                // A keychain failure keeps the field (return `false`) so a transient error does not
                // silently discard the key the user just typed. Issue #44: name the actual cause
                // (duplicate slot / over-long key / unavailable), not a flat "unavailable".
                Err(error) => {
                    prefs.set_provider_status(state::keychain_error_notice(error).into());
                    false
                }
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
                Err(error) => prefs.set_provider_status(state::keychain_error_notice(error).into()),
            }
        });
    }
    {
        // Issue #38: remove the ORPHANED key of a NON-active provider (the active provider uses the
        // Delete button above). Re-mirrors so the just-removed row leaves the orphan list at once.
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        ui.global::<Prefs>().on_delete_key_for(move |wire| {
            let ui = ui_weak.unwrap();
            let prefs = ui.global::<Prefs>();
            let Some(provider) = ProviderChoice::parse(&wire) else {
                return;
            };
            match keychain::delete_key(provider) {
                Ok(()) => prefs.set_provider_status(state::MSG_KEY_DELETED.into()),
                Err(error) => prefs.set_provider_status(state::keychain_error_notice(error).into()),
            }
            mirror_provider_prefs(&ui, config.borrow().preferred_provider);
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
            } else {
                // Issue #40: latch the in-flight flag ONLY on a successful send (a dead worker must
                // not disable the button for the session) — the Test button gates on this and the
                // TestKey outcome handler clears it, so a double-click can't stack duplicate jobs.
                prefs.set_key_testing(true);
            }
        });
    }
}
