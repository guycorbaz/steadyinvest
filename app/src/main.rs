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
// adds `core` (the faithful form header shows `core::METHOD_VERSION`, a static method-identity
// `&str` — display, NOT the forbidden engine call). So the crate-wide allow now covers a SHRUNK
// set: only `ingestion`, `report` and `tokio` remain unused (they light up in Epic 3 —
// ingestion/report data flow, tokio async provider I/O). (Scoping a crate-level lint allow to
// specific deps is not expressible; the comment is the scope of record, re-verified each story.)
#![allow(unused_crate_dependencies)]

mod clock;
mod config;
mod labels;
mod posture;
mod regime;
mod state;
mod theme;
mod viewmodel;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use uuid::Uuid;

use crate::clock::{SystemClock, UuidGen};
use crate::config::{AppConfig, StudyViewState};
use crate::labels::LabelSet;
use crate::regime::Regime;
use crate::state::JournalState;
use crate::theme::Theme;
use crate::viewmodel::format::{format_amount, NumberFormat};

slint::include_modules!();

/// Constant sample amounts for the Settings locale panel (formatted for display, computed
/// nothing — Cardinal Rule). Two stacked values with different digits double as the tabular-
/// figures check for the numeric font.
const SAMPLE_AMOUNT: &str = "-1234567.89";
const SAMPLE_AMOUNT_ALT: &str = "8888888.88";

/// Persist `config`, surfacing (not swallowing) a failure — a config that cannot be written is
/// a visible event, never a silence, but it must not take the app down.
fn persist(path: Option<&PathBuf>, config: &AppConfig) {
    let Some(path) = path else { return };
    if let Err(error) = config::save(path, config) {
        let message = format!("app-config save to {} failed: {error}", path.display());
        tracing::warn!("{message}");
        eprintln!("steadyinvest: {message}");
    }
}

fn push_samples(ui: &MainWindow, format: NumberFormat) {
    let prefs = ui.global::<Prefs>();
    prefs.set_sample_amount(format_amount(SAMPLE_AMOUNT, format).into());
    prefs.set_sample_amount_alt(format_amount(SAMPLE_AMOUNT_ALT, format).into());
}

/// Rebuild the dashboard list from the journal and mirror the read-only flag into the `Studies`
/// global. Called on startup and after every create.
fn refresh_studies(ui: &MainWindow, state: &JournalState) {
    let rows: Vec<StudyRow> = state
        .list_studies()
        .iter()
        .map(viewmodel::studies::to_row)
        .collect();
    let studies = ui.global::<Studies>();
    studies.set_rows(ModelRc::new(VecModel::from(rows)));
    studies.set_read_only(state.is_read_only());
}

/// Mirror a per-study view-state (regime + fold flags) into the `Studies` global and swap the
/// regime-driven token snapshot. The single place the UI's fold/regime state is pushed, so the
/// open-study restore and the toggle/regime callbacks stay consistent (one source of truth).
fn push_view_state(ui: &MainWindow, view_state: &StudyViewState) {
    let studies = ui.global::<Studies>();
    studies.set_regime(view_state.regime.as_str().into());
    studies.set_folds(ModelRc::new(VecModel::from(view_state.folds.to_vec())));
    regime::apply(ui, view_state.regime);
}

fn main() -> Result<(), slint::PlatformError> {
    // Minimal logging: tracing carries the events; a real subscriber (ADD15 rotating logs) is
    // deferred — see the Story 2.1 GitHub issue. Until then load warnings also go to stderr.
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

    // The id (stringified UUID) of the study whose form is currently open, so the fold/regime
    // callbacks know which `study_view_state` entry to mutate. `None` = the dashboard list view.
    let current_study: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

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
        if let Some(notice) = &startup_notice {
            ui.global::<Studies>().set_notice(notice.clone().into());
        }
    }

    // Create-study intent: validate + persist via the injected sources, then refresh the list.
    // A refused create (blank input / read-only / save failure) surfaces a neutral banner; a
    // successful one clears it.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Studies>()
            .on_create_study(move |ticker, currency| {
                let ui = ui_weak.unwrap();
                let result = journal_state.borrow_mut().create_study(&ticker, &currency);
                let studies = ui.global::<Studies>();
                let written = result.is_ok();
                match result {
                    Ok(_id) => studies.set_notice(SharedString::new()),
                    Err(message) => studies.set_notice(message.into()),
                }
                refresh_studies(&ui, &journal_state.borrow());
                // Report whether a study was written so the UI keeps the user's input on refusal.
                written
            });
    }

    // Open-study intent: reopen with full state and mount the faithful §1–§5 form (Story 2.3).
    // Money surfaces as formatted strings via the form adapter (the only float→string boundary), and
    // the persisted per-study fold/regime view-state is restored (default = Entry + all open).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>().on_open_study(move |id_text| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let Some(study) = journal_state.borrow().get_study(id) else {
                return;
            };
            let format = config.borrow().number_format;
            studies.set_form_header(viewmodel::form::header(&study));
            studies.set_pe_rows(ModelRc::new(VecModel::from(viewmodel::form::pe_rows(
                &study, format,
            ))));
            let view_state = config
                .borrow()
                .study_view_state
                .get(id_text.as_str())
                .cloned()
                .unwrap_or_default();
            push_view_state(&ui, &view_state);
            *current_study.borrow_mut() = Some(id_text.to_string());
            studies.set_study_open(true);
        });
    }

    // Fold a section: mutate this study's `study_view_state` (validate index → mutate → persist), then
    // push the new state back (one source of truth). Mirrors the 2.2 Prefs persistence shape — no
    // silent `.ok()`: a save failure surfaces via `persist`.
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>().on_toggle_fold(move |index, open| {
            let Some(id) = current_study.borrow().clone() else {
                return;
            };
            if !(0..regime::SECTION_COUNT as i32).contains(&index) {
                return;
            }
            let ui = ui_weak.unwrap();
            let new_state = {
                let mut cfg = config.borrow_mut();
                let entry = cfg.study_view_state.entry(id).or_default();
                entry.folds[index as usize] = open;
                entry.clone()
            };
            persist(path.as_ref(), &config.borrow());
            push_view_state(&ui, &new_state);
        });
    }

    // Switch regime: apply the regime's fold preset + swap the regime token snapshot, persist, push.
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>().on_set_regime(move |value| {
            let Some(new_regime) = Regime::parse(&value) else {
                return;
            };
            let Some(id) = current_study.borrow().clone() else {
                return;
            };
            // Selecting the already-active regime is a no-op (AC3: only an actual *switch* applies
            // the fold preset). Re-applying the preset here would silently clobber the user's manual
            // fold edits made within this regime.
            let current_regime = config
                .borrow()
                .study_view_state
                .get(&id)
                .map(|state| state.regime)
                .unwrap_or_default();
            if current_regime == new_regime {
                return;
            }
            let ui = ui_weak.unwrap();
            let new_state = {
                let mut cfg = config.borrow_mut();
                let entry = cfg.study_view_state.entry(id).or_default();
                entry.regime = new_regime;
                entry.folds = new_regime.fold_preset();
                entry.clone()
            };
            persist(path.as_ref(), &config.borrow());
            push_view_state(&ui, &new_state);
        });
    }

    // Settings intents: apply live (no restart), mirror into Prefs, persist on change.
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        ui.global::<Prefs>().on_theme_selected(move |value| {
            let Some(theme) = Theme::parse(&value) else {
                return;
            };
            let ui = ui_weak.unwrap();
            theme::apply(&ui, theme);
            ui.global::<Prefs>().set_dark_theme(theme == Theme::Dark);
            config.borrow_mut().theme = theme;
            persist(path.as_ref(), &config.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        ui.global::<Prefs>().on_label_set_selected(move |value| {
            let Some(set) = LabelSet::parse(&value) else {
                return;
            };
            let ui = ui_weak.unwrap();
            labels::apply(&ui, set);
            ui.global::<Prefs>().set_label_set(set.as_str().into());
            config.borrow_mut().label_set = set;
            persist(path.as_ref(), &config.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        ui.global::<Prefs>()
            .on_number_format_selected(move |value| {
                let Some(format) = NumberFormat::parse(&value) else {
                    return;
                };
                let ui = ui_weak.unwrap();
                push_samples(&ui, format);
                ui.global::<Prefs>()
                    .set_number_format(format.as_str().into());
                config.borrow_mut().number_format = format;
                persist(path.as_ref(), &config.borrow());
            });
    }

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
    persist(config_path.as_ref(), &config.borrow());
    Ok(())
}
