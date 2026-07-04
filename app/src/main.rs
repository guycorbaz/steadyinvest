//! steadyinvest-app — thin Slint desktop UI (binary).
//!
//! Story 2.1 shell: persistent nav rail + top bar + pinned footer disclaimer (FR64), token
//! design system with runtime dark/light switch (`theme.rs` → `Tokens` global), NAIC↔neutral
//! label set (`labels.rs`) and locale number format (`viewmodel/format.rs`), app-config
//! persistence in the OS config dir (`config.rs`, ADD7 — never inside the journal). Charts are
//! drawn natively in Slint (`Path` + `TouchArea`); there is no web view and no egui.
//!
//! # Internationalisation (UX-DR29)
//!
//! Every user-visible string in `ui/**/*.slint` is wrapped in `@tr()` with **French source
//! text**; no translation catalogs ship yet. When a second UI language lands, the drop-in
//! pipeline is:
//!
//! 1. extract: `cargo install slint-tr-extractor`, then
//!    `find ui -name '*.slint' | xargs slint-tr-extractor -o steadyinvest-app.pot`
//! 2. translate the `.pot` into per-language `.po` files (e.g. `lang/en/steadyinvest-app.po`);
//! 3. bundle: enable `slint-build`'s bundled-translations support in `build.rs`
//!    (`CompilerConfiguration::with_bundled_translations("lang")`) so catalogs compile into the
//!    binary — single-binary posture, no gettext system dependency;
//! 4. switch at runtime with `slint::select_bundled_translation("en")`.
//!
//! This axis is strictly distinct from the NAIC↔neutral label set (`labels.rs`), which is a
//! runtime data table of method vocabulary, not a translation.

// Story 2.2 put `contract`, `persistence`, `uuid` and `chrono` onto the runtime path; Story 2.3
// added `core` (the faithful form header shows `core::METHOD_VERSION`, a static method-identity
// `&str` — display, NOT the forbidden engine call); Story 2.4 promotes `arboard` (production
// paste-a-column clipboard read) and `rust_decimal` (locale entry parsing → `contract::Money`) from
// dev-only to genuinely-used runtime deps. So the crate-wide allow now covers a SHRUNK set: only
// `ingestion`, `report` and `tokio` remain unused (they light up in Epic 3 — ingestion/report data
// flow, tokio async provider I/O). (Scoping a crate-level lint allow to specific deps is not
// expressible; the comment is the scope of record, re-verified each story.)
#![allow(unused_crate_dependencies)]

mod clock;
mod config;
mod fetch;
mod keychain;
mod labels;
mod logging;
mod posture;
mod provider;
mod regime;
mod seam_check;
mod state;
mod theme;
mod viewmodel;
mod wiring;

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use uuid::Uuid;

use crate::clock::{SystemClock, UuidGen};
use crate::config::AppConfig;
use crate::state::{JournalState, UnlockScope};
use crate::theme::Theme;
use crate::wiring::fetch::mirror_provider_prefs;
use crate::wiring::holdings::{refresh_holdings, HoldingFreshnessMap};
use crate::wiring::journal::{record_current_pointer, render_journal_panel};
use crate::wiring::persist;
use crate::wiring::prefs::push_samples;
use crate::wiring::studies::refresh_studies;
use crate::wiring::watchlist::refresh_watchlist;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    // File logging + panic hook FIRST — before any work that might log or panic — so a crash (or any
    // `tracing` warn/info the app already emits) lands in a daily-rotating file under the OS data dir
    // (the ADD15 rotating logs, previously deferred). Non-fatal: no data dir → runs without a file.
    let log_dir = logging::init();
    if let Some(dir) = &log_dir {
        eprintln!("steadyinvest: logging to {}", dir.display());
    }
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "steadyinvest starting");
    // Story 3.1 — install the pure-Rust `ring` crypto provider for rustls before any HTTPS fetch
    // (reqwest uses `rustls-no-provider`). Idempotent; must run before the fetch worker is used.
    steadyinvest_ingestion::install_crypto_provider();

    let config_path = config::default_path();
    if config_path.is_none() {
        eprintln!("steadyinvest: no OS config directory found; preferences are not persisted");
    }
    let loaded = match &config_path {
        Some(path) => config::load(path),
        None => config::Loaded {
            config: AppConfig::default(),
            warning: None,
        },
    };
    if let Some(warning) = &loaded.warning {
        eprintln!("steadyinvest: {warning}");
    }
    let config = Rc::new(RefCell::new(loaded.config));

    // Open the last-used journal (or create the default one), with identity + time from the
    // injected sources (ADD15). This is the first time the app opens the journal — Story 2.1
    // deliberately did not. Failure degrades to a usable journal-less state, never a crash.
    let configured = config.borrow().journal_path.clone();
    let (journal_state, startup_notice) = JournalState::open_or_create(
        configured.as_deref(),
        Box::new(SystemClock),
        Box::new(UuidGen),
    );
    // Persist the resolved path so the same journal reopens next launch (only when it changed).
    {
        let resolved = journal_state.path().map(Path::to_path_buf);
        let mut cfg = config.borrow_mut();
        if cfg.journal_path != resolved {
            cfg.journal_path = resolved;
            drop(cfg);
            persist(config_path.as_ref(), &config.borrow());
        }
    }
    let journal_state = Rc::new(RefCell::new(journal_state));

    // Story 6.1 (FR37): restore the last-selected active portfolio from app-config. A stale/garbage
    // id is ignored by `set_active_portfolio` (it validates against the live list → falls back to the
    // first), so this is safe across a journal switch or a deleted portfolio.
    if let Some(id) = config
        .borrow()
        .active_portfolio_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        journal_state.borrow_mut().set_active_portfolio(id);
    }

    // The id (stringified UUID) of the study whose form is currently open, so the fold/regime
    // callbacks know which `study_view_state` entry to mutate. `None` = the dashboard list view.
    let current_study: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    // The scope of a pending "unlock all" (Story 2.5), held between the confirmation overlay being
    // raised (`request-unlock`) and the user pressing Confirmer (`confirm-unlock`). `None` = no
    // pending action; Annuler clears it without mutating anything.
    let pending_unlock: Rc<RefCell<Option<UnlockScope>>> = Rc::new(RefCell::new(None));

    // Story 2.12 — the pending dashboard lifecycle action `(action, study_id)`, held between
    // `request-study-action` raising the confirm overlay and `confirm-study-action`. `None` = no
    // pending action; Annuler clears it without mutating anything.
    let pending_study_action: Rc<RefCell<Option<(String, Uuid)>>> = Rc::new(RefCell::new(None));

    // Story 2.8 — the open study cached for the duration of a §1 judgment-line drag, so each `moved`
    // event recomputes from memory (no journal read/write) under the <100 ms budget. `None` = no drag
    // in flight; set on pointer-down, cleared on release/commit.
    let drag_study: Rc<RefCell<Option<steadyinvest_contract::Study>>> = Rc::new(RefCell::new(None));
    // Whether the in-flight drag actually moved (a `moved` event fired). A pointer-up with no
    // movement is a click, not a drag — it must NOT rewrite the forecast (review P2).
    let drag_moved: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

    // Story 2.9 — the saved study captured while the scenario-compare overlay is open, so the typed
    // alternate is recomputed against it WITHOUT persisting. `None` = the overlay is closed.
    let compare_study: Rc<RefCell<Option<steadyinvest_contract::Study>>> =
        Rc::new(RefCell::new(None));

    // Story 4.4 — transient per-ticker holdings price-refresh freshness (NOT persisted; display-time
    // only). Populated by the off-thread refresh outcomes; read when rebuilding the register.
    let holding_freshness: Rc<RefCell<HoldingFreshnessMap>> =
        Rc::new(RefCell::new(HoldingFreshnessMap::new()));

    // Story 4.7 — holding ids whose neutral-trigger action panel the user has dismissed this session
    // (transient; never persisted — a dismissed trigger re-appears next launch if still firing).
    let holding_dismissed: Rc<RefCell<std::collections::HashSet<String>>> =
        Rc::new(RefCell::new(std::collections::HashSet::new()));
    // Issue #52 — outstanding holdings-refresh jobs in flight. Set to the enqueued count when a
    // refresh starts; each outcome decrements it; the button is disabled (`Holdings.refreshing`)
    // until it returns to zero, so a double-click can't enqueue duplicate jobs.
    let refresh_pending: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));

    let ui = MainWindow::new()?;

    // Initial state, pushed before the window shows: no flash, no restart needed later.
    {
        let cfg = config.borrow();
        theme::apply(&ui, cfg.theme);
        labels::apply(&ui, cfg.label_set);
        let prefs = ui.global::<Prefs>();
        prefs.set_dark_theme(cfg.theme == Theme::Dark);
        prefs.set_label_set(cfg.label_set.as_str().into());
        prefs.set_number_format(cfg.number_format.as_str().into());
        push_samples(&ui, cfg.number_format);
        mirror_provider_prefs(&ui, cfg.preferred_provider);
        // Story 4.3 (FR63): mirror the reference currency (validated) into Prefs + the holdings
        // register, so the picker reflects it and amounts are labelled before the window shows.
        let currency = cfg.reference_currency_or_default();
        prefs.set_reference_currency(currency.clone().into());
        ui.global::<Holdings>()
            .set_reference_currency(currency.into());
        // Story 6.2 (FR38): push the fixed holding-currency allow-list so the picker and the Rust
        // validator share one source of truth (config::SUPPORTED_CURRENCIES).
        let supported: Vec<SharedString> = config::SUPPORTED_CURRENCIES
            .iter()
            .map(|c| SharedString::from(*c))
            .collect();
        ui.global::<Holdings>()
            .set_supported_currencies(ModelRc::new(VecModel::from(supported)));
        // Story 4.5 (FR42): mirror the default trailing-stop % (validated; "" when none) so the
        // set-stop control pre-fills it.
        prefs.set_default_trailing_stop_pct(
            cfg.default_trailing_stop_pct_or_none()
                .unwrap_or_default()
                .into(),
        );
        // Story 6.4 (FR41): mirror the default dividend withholding rate (always a value; 35 = CH).
        prefs.set_withholding_rate_pct(cfg.withholding_rate_pct_or_default().into());
        // Story 6.7 (FR45): mirror the concentration threshold + the diversify-by-size table
        // (validated effective values) into Prefs + Holdings before the first render.
        wiring::prefs::mirror_risk_settings(&ui, &cfg);
        // Story 6.9 (FR26): mirror the per-field-type fallback providers.
        wiring::prefs::mirror_fallback_prefs(&ui, &cfg);

        // Best-effort restore BEFORE show to minimise the visible jump; the authoritative
        // restore happens again right after show() below — before the window is mapped, winit
        // (X11) has no scale factor yet and silently misapplies a physical size.
        let (width, height) = cfg.sane_window_size();
        ui.window()
            .set_size(slint::PhysicalSize::new(width, height));
        if cfg.maximized {
            ui.window().set_maximized(true);
        }
    }

    // Initial Studies state: the list, the read-only flag, and any startup notice (read-only
    // journal / unreadable configured file). Pushed before the window shows.
    {
        refresh_studies(&ui, &journal_state.borrow());
        refresh_watchlist(&ui, &journal_state.borrow());
        // Story 6.5 (FR28): the stored FX rates for the Réglages panel.
        wiring::fx::push_fx_rates(&ui, &journal_state.borrow());
        refresh_holdings(
            &ui,
            &journal_state.borrow(),
            &holding_freshness.borrow(),
            &holding_dismissed.borrow(),
            config.borrow().number_format,
        );
        if let Some(notice) = &startup_notice {
            ui.global::<Studies>().set_notice(notice.clone().into());
        }
        // Story 5.5: record the startup journal in the recent list (so it appears + its last-seen
        // pointer is set) and render the location panel.
        {
            let st = journal_state.borrow();
            if let Some(path) = st.path().map(|p| p.to_path_buf()) {
                if let Some(jid) = st.journal_id() {
                    let version = st.logical_version_or_zero();
                    config
                        .borrow_mut()
                        .record_recent(&path, &jid.to_string(), version);
                    persist(config_path.as_ref(), &config.borrow());
                }
            }
            render_journal_panel(&ui, &st, &config.borrow());
        }
    }

    // ── Story 3.1 — provider auto-fetch (EODHD), off the UI thread ──
    // The worker thread owns the tokio runtime + network I/O; results return via the thread_local
    // handler set below (which holds the `Rc` state and runs on the UI thread).
    let fetch_tx = fetch::spawn_fetch_worker();

    // Bundle the session-scoped handles the wiring closures capture (`wiring::Session`), then
    // register every domain's callbacks. `wire_fetch` installs the worker's outcome handler before
    // any job can be enqueued (the send sites are themselves wired after it).
    let session = wiring::Session {
        journal_state: Rc::clone(&journal_state),
        config: Rc::clone(&config),
        config_path: config_path.clone(),
        fetch_tx,
        current_study: Rc::clone(&current_study),
        pending_unlock: Rc::clone(&pending_unlock),
        pending_study_action: Rc::clone(&pending_study_action),
        drag_study: Rc::clone(&drag_study),
        drag_moved: Rc::clone(&drag_moved),
        compare_study: Rc::clone(&compare_study),
        holding_freshness: Rc::clone(&holding_freshness),
        holding_dismissed: Rc::clone(&holding_dismissed),
        refresh_pending: Rc::clone(&refresh_pending),
    };
    wiring::fetch::wire_fetch(&ui, &session);
    wiring::studies::wire_studies(&ui, &session);
    wiring::overlays::wire_overlays(&ui, &session);
    wiring::journal::wire_journal(&ui, &session);
    wiring::watchlist::wire_watchlist(&ui, &session);
    wiring::holdings::wire_holdings(&ui, &session);
    wiring::replacement::wire_replacement(&ui, &session);
    wiring::wire_navigation(&ui, &session);
    wiring::cells::wire_cells(&ui, &session);
    wiring::judgment::wire_judgment(&ui, &session);
    wiring::prefs::wire_prefs(&ui, &session);
    wiring::fx::wire_fx(&ui, &session);

    // show() → re-apply the size from inside the running event loop → run: before the window
    // is mapped winit has no scale factor yet and misapplies a physical size (observed on
    // X11/XWayland: width falls back to min-width, height is scaled as if logical), so the
    // restore set before show() is only best-effort. A short timer re-requests the persisted
    // size for ~300 ms — past the map + scale-factor races — then stops for good, so it never
    // fights the user or a tiling window manager. `window().size()` reads back the requested
    // size, not the mapped one, so there is nothing reliable to compare against; the fixed
    // tick count is deliberate.
    ui.show()?;
    let restore_timer = Rc::new(slint::Timer::default());
    {
        let timer = Rc::clone(&restore_timer);
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let attempts = std::cell::Cell::new(0u32);
        restore_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(50),
            move || {
                let ui = ui_weak.unwrap();
                let cfg = config.borrow();
                if cfg.maximized || attempts.get() >= 6 {
                    timer.stop();
                    return;
                }
                attempts.set(attempts.get() + 1);
                let (width, height) = cfg.sane_window_size();
                ui.window()
                    .set_size(slint::PhysicalSize::new(width, height));
            },
        );
    }
    slint::run_event_loop()?;
    restore_timer.stop();
    ui.hide()?;

    // Window geometry is captured at exit (size only while not maximized, so un-maximizing
    // after a relaunch restores the last floating size).
    {
        let mut cfg = config.borrow_mut();
        cfg.maximized = ui.window().is_maximized();
        if !cfg.maximized {
            let size = ui.window().size();
            cfg.window_width = size.width;
            cfg.window_height = size.height;
        }
    }
    // Record the open journal's final version at exit too (Story 5.5) — the last-seen pointer then
    // reflects the whole session's edits (this also persists the geometry captured just above).
    record_current_pointer(&journal_state, &config, &config_path);
    persist(config_path.as_ref(), &config.borrow());
    Ok(())
}
