//! Preferences wiring (Réglages, Story 2.2 + FR63): theme, NAIC↔neutral label set, locale number
//! format (with the tabular-figures sample amounts), the Story 4.3 reference currency (re-labels +
//! re-renders the register), the Story 4.5 default trailing-stop %, and the Story 3.2 provider
//! choice — each applied live, mirrored into `Prefs`, and persisted on change. Moved verbatim from
//! `main.rs` — no logic change.

use std::rc::Rc;

use slint::ComponentHandle;

use crate::labels::LabelSet;
use crate::provider::ProviderChoice;
use crate::theme::Theme;
use crate::viewmodel::format::{format_amount, NumberFormat};
use crate::wiring::fetch::mirror_provider_prefs;
use crate::wiring::holdings::refresh_holdings;
use crate::wiring::{persist, Session};
use crate::{config, labels, theme};
use crate::{Holdings, MainWindow, Prefs};

/// Constant sample amounts for the Settings locale panel (formatted for display, computed
/// nothing — Cardinal Rule). Two stacked values with different digits double as the tabular-
/// figures check for the numeric font.
const SAMPLE_AMOUNT: &str = "-1234567.89";
const SAMPLE_AMOUNT_ALT: &str = "8888888.88";

pub(crate) fn push_samples(ui: &MainWindow, format: NumberFormat) {
    let prefs = ui.global::<Prefs>();
    prefs.set_sample_amount(format_amount(SAMPLE_AMOUNT, format).into());
    prefs.set_sample_amount_alt(format_amount(SAMPLE_AMOUNT_ALT, format).into());
}

/// Wire the preferences domain: theme / label-set / number-format / reference-currency /
/// default-trailing-stop / provider-choice, applied live and persisted on change.
pub(crate) fn wire_prefs(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        config_path,
        holding_freshness,
        holding_dismissed,
        ..
    } = s;
    // Settings intents: apply live (no restart), mirror into Prefs, persist on change.
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
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
        let config = Rc::clone(config);
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
        let config = Rc::clone(config);
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
    // ── Story 4.3 — reference currency (FR63) ── persist the chosen code and re-label the register.
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let path = config_path.clone();
        let journal_state = Rc::clone(journal_state);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Prefs>()
            .on_reference_currency_selected(move |value| {
                // Validate the shape (3 uppercase letters); ignore a malformed pick rather than
                // persist garbage (mirrors the parse-guard of the other Prefs callbacks).
                if !config::is_valid_currency_code(&value) {
                    return;
                }
                let ui = ui_weak.unwrap();
                config.borrow_mut().reference_currency = value.to_string();
                persist(path.as_ref(), &config.borrow());
                ui.global::<Prefs>().set_reference_currency(value.clone());
                ui.global::<Holdings>().set_reference_currency(value);
                // Re-render the register: since Story 6.2 the per-row currency label and the
                // per-currency capital-at-risk buckets are BAKED at render time (a pre-6.2 NULL-currency
                // row coalesces to the reference currency), so setting the global above is not enough.
                let format = config.borrow().number_format;
                refresh_holdings(
                    &ui,
                    &journal_state.borrow(),
                    &holding_freshness.borrow(),
                    &holding_dismissed.borrow(),
                    format,
                );
            });
    }
    // ── Story 4.5 (FR42/FR63) — the default trailing-stop %. "" clears; else validate (0,100). ──
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let path = config_path.clone();
        ui.global::<Prefs>()
            .on_default_trailing_stop_pct_changed(move |value| {
                let value = value.trim();
                // Empty clears the default; otherwise ignore a malformed percent (parse-guard).
                let stored = if value.is_empty() {
                    None
                } else if config::is_valid_trailing_stop_pct(value) {
                    Some(value.to_string())
                } else {
                    return;
                };
                let ui = ui_weak.unwrap();
                config.borrow_mut().default_trailing_stop_pct = stored;
                persist(path.as_ref(), &config.borrow());
                ui.global::<Prefs>().set_default_trailing_stop_pct(
                    config
                        .borrow()
                        .default_trailing_stop_pct_or_none()
                        .unwrap_or_default()
                        .into(),
                );
            });
    }
    // ── Story 6.4 (FR41) — the default dividend withholding rate. "" resets to the pinned 35. ──
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let path = config_path.clone();
        ui.global::<Prefs>()
            .on_withholding_rate_pct_changed(move |value| {
                let value = value.trim();
                // Empty resets to the default; otherwise ignore a malformed/out-of-range percent
                // (parse-guard — the accessor validates [0, 100]).
                let stored = if value.is_empty() {
                    None
                } else if config::is_valid_withholding_rate_pct(value) {
                    Some(value.to_string())
                } else {
                    return;
                };
                let ui = ui_weak.unwrap();
                config.borrow_mut().withholding_rate_pct = stored;
                persist(path.as_ref(), &config.borrow());
                ui.global::<Prefs>().set_withholding_rate_pct(
                    config.borrow().withholding_rate_pct_or_default().into(),
                );
            });
    }
    // ── Story 3.2 — provider selection + key management (FR25/FR63) ──
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let path = config_path.clone();
        ui.global::<Prefs>().on_provider_selected(move |value| {
            let Some(choice) = ProviderChoice::parse(&value) else {
                return;
            };
            let ui = ui_weak.unwrap();
            config.borrow_mut().preferred_provider = choice;
            persist(path.as_ref(), &config.borrow());
            mirror_provider_prefs(&ui, choice);
            // Drop any prior save/test verdict — it referred to the previous provider (F3).
            ui.global::<Prefs>().set_provider_status("".into());
        });
    }
}
