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

// contract/ingestion/persistence/report/tokio are declared since story 1.1 so the dependency
// graph and crate boundaries are fixed; they stay unused until 2.2/3.x. steadyinvest-core is
// used by the posture gate (tests only), which this lint does not count.
#![allow(unused_crate_dependencies)]

mod config;
mod labels;
mod posture;
mod theme;
mod viewmodel;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use slint::ComponentHandle;

use crate::config::AppConfig;
use crate::labels::LabelSet;
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
