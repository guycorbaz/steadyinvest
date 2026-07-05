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
use crate::viewmodel::format::{NumberFormat, format_amount};
use crate::wiring::fetch::mirror_provider_prefs;
use crate::wiring::holdings::refresh_holdings;
use crate::wiring::{Session, persist};
use crate::{Holdings, MainWindow, Prefs};
use crate::{config, labels, theme};

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

/// Mirror the Story 6.9 per-field-type fallback providers from the VALIDATED config accessors
/// into `Prefs` (wire strings; "" = no fallback). Capability-filtered — a hand-edited
/// fundamentals fallback of `twelvedata` mirrors as "" (dropped), matching the effective chain.
pub(crate) fn mirror_fallback_prefs(ui: &MainWindow, cfg: &crate::config::AppConfig) {
    use steadyinvest_ingestion::FieldKind;
    let prefs = ui.global::<Prefs>();
    let wire = |field: FieldKind| {
        cfg.fallback_provider_or_none(field)
            .map(|c| c.wire())
            .unwrap_or_default()
    };
    prefs.set_price_fallback(wire(FieldKind::Price).into());
    prefs.set_fundamentals_fallback(wire(FieldKind::Fundamentals).into());
    prefs.set_fx_fallback(wire(FieldKind::Fx).into());
}

/// Mirror the Story 6.7 risk settings (concentration threshold + diversify-by-size table) from
/// the validated config accessors into BOTH globals: `Prefs` (the Réglages fields) and `Holdings`
/// (the canonical strings `refresh_holdings` bakes at render time — the reference-currency
/// pattern). Always effective values (defaults when unset/damaged).
pub(crate) fn mirror_risk_settings(ui: &MainWindow, cfg: &crate::config::AppConfig) {
    let threshold = cfg.concentration_threshold_pct_or_default();
    let (small_max, medium_max) = cfg.size_bounds_or_default();
    let (target_small, target_medium, target_large) = cfg.size_targets_or_default();
    let prefs = ui.global::<Prefs>();
    prefs.set_concentration_threshold_pct(threshold.clone().into());
    prefs.set_size_small_max(small_max.clone().into());
    prefs.set_size_medium_max(medium_max.clone().into());
    prefs.set_size_target_small_pct(target_small.clone().into());
    prefs.set_size_target_medium_pct(target_medium.clone().into());
    prefs.set_size_target_large_pct(target_large.clone().into());
    let holdings = ui.global::<Holdings>();
    holdings.set_concentration_threshold_pct(threshold.into());
    holdings.set_size_small_max(small_max.into());
    holdings.set_size_medium_max(medium_max.into());
    holdings.set_size_target_small_pct(target_small.into());
    holdings.set_size_target_medium_pct(target_medium.into());
    holdings.set_size_target_large_pct(target_large.into());
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
        let journal_state = Rc::clone(journal_state);
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
                // Story 6.8 (2026-07-03 review): an OPEN candidates panel re-renders in the new
                // locale (its strings are baked at push time).
                crate::wiring::replacement::sync_candidates(&ui, &journal_state.borrow());
                // Issue #107: the dashboard's potential-return column is baked in the current locale
                // at curate time — re-render it so the "%" figures re-format with the new separator.
                crate::wiring::studies::refresh_studies(&ui, &journal_state.borrow());
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
    // ── Story 6.7 (FR45) — the concentration threshold. "" resets to the default (50); else
    // validate (0,100) (the trailing-stop shape). A refusal names itself and writes nothing; a
    // success re-mirrors + re-renders the Portefeuille block (the mirrors bake at render time). ──
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let path = config_path.clone();
        let journal_state = Rc::clone(journal_state);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Prefs>()
            .on_concentration_threshold_pct_changed(move |value| {
                let ui = ui_weak.unwrap();
                let value = value.trim();
                let stored = if value.is_empty() {
                    None
                } else if config::is_valid_trailing_stop_pct(value) {
                    Some(value.to_string())
                } else {
                    ui.global::<Prefs>()
                        .set_risk_settings_status(crate::state::MSG_CONCENTRATION_INVALID.into());
                    return;
                };
                config.borrow_mut().concentration_threshold_pct = stored;
                persist(path.as_ref(), &config.borrow());
                ui.global::<Prefs>().set_risk_settings_status("".into());
                mirror_risk_settings(&ui, &config.borrow());
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
    // ── Story 6.7 (FR45) — the diversify-by-size table: two boundaries + three targets, committed
    // WHOLE or not at all ("" = that field's default; the small < medium cross-check runs on the
    // EFFECTIVE values so a half-empty commit cannot cross the defaults). ──
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let path = config_path.clone();
        let journal_state = Rc::clone(journal_state);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Prefs>().on_size_table_changed(
            move |small_max, medium_max, target_small, target_medium, target_large| {
                let ui = ui_weak.unwrap();
                let refuse = || {
                    ui_weak
                        .unwrap()
                        .global::<Prefs>()
                        .set_risk_settings_status(crate::state::MSG_SIZE_TABLE_INVALID.into());
                };
                // "" → None (that field's pinned default); a non-empty field must validate.
                let bound = |v: &str, ok: &mut bool| {
                    let v = v.trim();
                    if v.is_empty() {
                        None
                    } else if config::is_valid_size_bound(v) {
                        Some(v.to_string())
                    } else {
                        *ok = false;
                        None
                    }
                };
                let target = |v: &str, ok: &mut bool| {
                    let v = v.trim();
                    if v.is_empty() {
                        None
                    } else if config::is_valid_size_target_pct(v) {
                        Some(v.to_string())
                    } else {
                        *ok = false;
                        None
                    }
                };
                let mut ok = true;
                let small = bound(&small_max, &mut ok);
                let medium = bound(&medium_max, &mut ok);
                let t_small = target(&target_small, &mut ok);
                let t_medium = target(&target_medium, &mut ok);
                let t_large = target(&target_large, &mut ok);
                if !ok {
                    refuse();
                    return;
                }
                // Cross-check on the EFFECTIVE pair (entered or default): small < medium.
                let effective = |v: &Option<String>, default: &str| {
                    rust_decimal::Decimal::from_str_exact(v.as_deref().unwrap_or(default))
                };
                match (
                    effective(&small, config::DEFAULT_SIZE_SMALL_MAX),
                    effective(&medium, config::DEFAULT_SIZE_MEDIUM_MAX),
                ) {
                    (Ok(s), Ok(m)) if s < m => {}
                    _ => {
                        refuse();
                        return;
                    }
                }
                {
                    let mut cfg = config.borrow_mut();
                    cfg.size_small_max = small;
                    cfg.size_medium_max = medium;
                    cfg.size_target_small_pct = t_small;
                    cfg.size_target_medium_pct = t_medium;
                    cfg.size_target_large_pct = t_large;
                }
                persist(path.as_ref(), &config.borrow());
                ui.global::<Prefs>().set_risk_settings_status("".into());
                mirror_risk_settings(&ui, &config.borrow());
                let format = config.borrow().number_format;
                refresh_holdings(
                    &ui,
                    &journal_state.borrow(),
                    &holding_freshness.borrow(),
                    &holding_dismissed.borrow(),
                    format,
                );
            },
        );
    }
    // ── Story 6.9 (FR26) — the per-field-type fallback provider. "" clears; a wire name must
    // parse (and never `none` — the absent field IS "no fallback"); an incapable pick for the
    // field is ignored (the UI never offers one — defensive against a stale UI state). ──
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let path = config_path.clone();
        ui.global::<Prefs>()
            .on_fallback_provider_selected(move |field, wire| {
                use steadyinvest_ingestion::FieldKind;
                let field = match field.as_str() {
                    "price" => FieldKind::Price,
                    "fundamentals" => FieldKind::Fundamentals,
                    "fx" => FieldKind::Fx,
                    _ => return,
                };
                let stored = if wire.is_empty() {
                    None
                } else if config::is_valid_fallback_provider(&wire)
                    && steadyinvest_ingestion::supports(wire.as_str(), field)
                {
                    Some(wire.to_string())
                } else {
                    return;
                };
                let ui = ui_weak.unwrap();
                {
                    let mut cfg = config.borrow_mut();
                    match field {
                        FieldKind::Price => cfg.price_fallback_provider = stored,
                        FieldKind::Fundamentals => cfg.fundamentals_fallback_provider = stored,
                        FieldKind::Fx => cfg.fx_fallback_provider = stored,
                    }
                }
                persist(path.as_ref(), &config.borrow());
                mirror_fallback_prefs(&ui, &config.borrow());
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
            // Story 6.9 (2026-07-03 review): a primary switch changes the EFFECTIVE fallbacks
            // (a fallback equal to the new primary neutralizes) — the chips must follow.
            mirror_fallback_prefs(&ui, &config.borrow());
            // Drop any prior save/test verdict — it referred to the previous provider (F3).
            ui.global::<Prefs>().set_provider_status("".into());
        });
    }
}
