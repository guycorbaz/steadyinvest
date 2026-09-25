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
use crate::viewmodel::format::{NumberFormat, NumberReading, format_amount, read_number};
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

/// Mirror the Réglages number settings — the Story 4.5 default trailing stop, the Story 6.4
/// withholding rate, the Story 6.7 concentration threshold + diversify-by-size table — from the
/// validated config accessors. `Prefs` (the Réglages fields, the dialogs' prefills and labels)
/// carries them in the user's number format (G1 I: « 12,5 » under the comma format — read back by
/// the same [`crate::viewmodel::format::parse_decimal`] rule); `Holdings` keeps the canonical
/// strings `refresh_holdings` bakes at render time (the reference-currency pattern). Always
/// effective values (defaults when unset/damaged).
pub(crate) fn mirror_risk_settings(ui: &MainWindow, cfg: &crate::config::AppConfig) {
    let shown = risk_settings_shown(cfg);
    let prefs = ui.global::<Prefs>();
    prefs.set_default_trailing_stop_pct(shown.default_stop.into());
    prefs.set_withholding_rate_pct(shown.withholding.into());
    prefs.set_concentration_threshold_pct(shown.threshold.into());
    prefs.set_size_small_max(shown.small_max.into());
    prefs.set_size_medium_max(shown.medium_max.into());
    prefs.set_size_target_small_pct(shown.target_small.into());
    prefs.set_size_target_medium_pct(shown.target_medium.into());
    prefs.set_size_target_large_pct(shown.target_large.into());
    let threshold = cfg.concentration_threshold_pct_or_default();
    let (small_max, medium_max) = cfg.size_bounds_or_default();
    let (target_small, target_medium, target_large) = cfg.size_targets_or_default();
    let holdings = ui.global::<Holdings>();
    holdings.set_concentration_threshold_pct(threshold.into());
    holdings.set_size_small_max(small_max.into());
    holdings.set_size_medium_max(medium_max.into());
    holdings.set_size_target_small_pct(target_small.into());
    holdings.set_size_target_medium_pct(target_medium.into());
    holdings.set_size_target_large_pct(target_large.into());
}

/// The Réglages number settings as the user reads them (G1 I): the effective values spelled in
/// the user's number format ("" for an unset default stop). Pure — the display half of
/// [`mirror_risk_settings`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RiskSettingsShown {
    pub default_stop: String,
    pub withholding: String,
    pub threshold: String,
    pub small_max: String,
    pub medium_max: String,
    pub target_small: String,
    pub target_medium: String,
    pub target_large: String,
}

pub(crate) fn risk_settings_shown(cfg: &crate::config::AppConfig) -> RiskSettingsShown {
    let shown = |canonical: &str| format_amount(canonical, cfg.number_format);
    let (small_max, medium_max) = cfg.size_bounds_or_default();
    let (target_small, target_medium, target_large) = cfg.size_targets_or_default();
    RiskSettingsShown {
        default_stop: shown(&cfg.default_trailing_stop_pct_or_none().unwrap_or_default()),
        withholding: shown(&cfg.withholding_rate_pct_or_default()),
        threshold: shown(&cfg.concentration_threshold_pct_or_default()),
        small_max: shown(&small_max),
        medium_max: shown(&medium_max),
        target_small: shown(&target_small),
        target_medium: shown(&target_medium),
        target_large: shown(&target_large),
    }
}

/// A Réglages number field as typed (G1 I review): `Ok(None)` when blank (the field's default /
/// clear), `Ok(Some(canonical))` when it reads under the user's number format AND passes the
/// setting's own validation (the canonical spelling is what app-config stores), else the named
/// refusal — `invalid` for a non-number or an out-of-range value, the format's ambiguous-number
/// message for a number in another spelling.
pub(crate) fn setting_input(
    value: &str,
    format: NumberFormat,
    valid: fn(&str) -> bool,
    invalid: &str,
) -> Result<Option<String>, String> {
    match read_number(value, format) {
        NumberReading::Blank => Ok(None),
        NumberReading::Value(d) => {
            let canonical = d.normalize().to_string();
            if valid(&canonical) {
                Ok(Some(canonical))
            } else {
                Err(invalid.to_string())
            }
        }
        NumberReading::Ambiguous => Err(crate::state::ambiguous_number_message(format).to_string()),
        NumberReading::NotANumber => Err(invalid.to_string()),
    }
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
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
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
                // G1 I: the rails read typed amounts under the new format, and every figure baked
                // in the old one — the Réglages fields, the register, the open ledger — re-renders.
                journal_state.borrow_mut().set_number_format(format);
                mirror_risk_settings(&ui, &config.borrow());
                // G1 I review: the Réglages panels re-assign their drafts on this bump (their
                // two-way bindings are severed), so the fields re-spell in place. Bumped on a FORMAT
                // change only: a save re-syncs its own card, and never wipes another card's unsaved
                // text (G1 review, AC1).
                let prefs = ui.global::<Prefs>();
                prefs.set_number_settings_epoch(prefs.get_number_settings_epoch().wrapping_add(1));
                refresh_holdings(
                    &ui,
                    &journal_state.borrow(),
                    &holding_freshness.borrow(),
                    &holding_dismissed.borrow(),
                    format,
                );
                if let Ok(open) =
                    uuid::Uuid::parse_str(&ui.global::<Holdings>().get_ledger_holding_id())
                {
                    crate::wiring::holdings::sync_ledger_panel(&ui, &journal_state.borrow(), open);
                }
                crate::wiring::fx::push_fx_rates(&ui, &journal_state.borrow());
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
                let ui = ui_weak.unwrap();
                let value = value.trim();
                // Empty clears the default; a malformed percent is REFUSED with a named notice (issue
                // #88). G1 review (spec AC1): the refusal returns `false` so the panel KEEPS the
                // typed text, and it touches no other card's status slot.
                // G1 I: read under the user's number format, stored canonical.
                let format = config.borrow().number_format;
                let stored = match setting_input(
                    value,
                    format,
                    config::is_valid_trailing_stop_pct,
                    crate::state::MSG_TRAILING_STOP_INVALID,
                ) {
                    Ok(stored) => stored,
                    Err(message) => {
                        // The refusal keeps the typed text (G1 review, AC1).
                        crate::wiring::dialog::refuse(&ui, &message);
                        return false;
                    }
                };
                config.borrow_mut().default_trailing_stop_pct = stored;
                persist(path.as_ref(), &config.borrow());
                mirror_risk_settings(&ui, &config.borrow());
                true
            });
    }
    // ── Story 6.4 (FR41) — the default dividend withholding rate. "" resets to the pinned 35. ──
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let path = config_path.clone();
        ui.global::<Prefs>()
            .on_withholding_rate_pct_changed(move |value| {
                let ui = ui_weak.unwrap();
                let value = value.trim();
                // Empty resets to the default; a malformed/out-of-range percent is REFUSED with a
                // named notice (issue #88); the refusal keeps the typed text (G1 review, AC1).
                let format = config.borrow().number_format;
                let stored = match setting_input(
                    value,
                    format,
                    config::is_valid_withholding_rate_pct,
                    crate::state::MSG_WITHHOLDING_INVALID,
                ) {
                    Ok(stored) => stored,
                    Err(message) => {
                        // The refusal keeps the typed text (G1 review, AC1).
                        crate::wiring::dialog::refuse(&ui, &message);
                        return false;
                    }
                };
                config.borrow_mut().withholding_rate_pct = stored;
                persist(path.as_ref(), &config.borrow());
                mirror_risk_settings(&ui, &config.borrow());
                true
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
                let format = config.borrow().number_format;
                let stored = match setting_input(
                    value,
                    format,
                    config::is_valid_trailing_stop_pct,
                    crate::state::MSG_CONCENTRATION_INVALID,
                ) {
                    Ok(stored) => stored,
                    Err(message) => {
                        // The refusal keeps the typed text (G1 review, AC1).
                        crate::wiring::dialog::refuse(&ui, &message);
                        return false;
                    }
                };
                config.borrow_mut().concentration_threshold_pct = stored;
                persist(path.as_ref(), &config.borrow());
                mirror_risk_settings(&ui, &config.borrow());
                let format = config.borrow().number_format;
                refresh_holdings(
                    &ui,
                    &journal_state.borrow(),
                    &holding_freshness.borrow(),
                    &holding_dismissed.borrow(),
                    format,
                );
                true
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
                // A refusal names itself in the dialog; it returns `false` so the table KEEPS the
                // typed text (G1 review, spec AC1) and touches no other card's status slot.
                let set_status = |msg: String| {
                    crate::wiring::dialog::refuse(&ui, &msg);
                };
                // "" → None (that field's pinned default); a non-empty field must validate. Issue #96:
                // the FIRST offending field is named. G1 I: each field reads under the user's number
                // format (an ambiguous number is refused as such), stored canonical.
                let format = config.borrow().number_format;
                let mut refusal: Option<String> = None;
                let mut field = |v: &str, valid: fn(&str) -> bool, label: &'static str| {
                    let invalid = crate::state::size_field_invalid_message(label);
                    match setting_input(v, format, valid, &invalid) {
                        Ok(stored) => stored,
                        Err(message) => {
                            refusal.get_or_insert(message);
                            None
                        }
                    }
                };
                let bound = |v: &str| config::is_valid_size_bound(v);
                let target = |v: &str| config::is_valid_size_target_pct(v);
                let small = field(&small_max, bound, "Borne petite");
                let medium = field(&medium_max, bound, "Borne moyenne");
                let t_small = field(&target_small, target, "Cible petite");
                let t_medium = field(&target_medium, target, "Cible moyenne");
                let t_large = field(&target_large, target, "Cible grande");
                if let Some(message) = refusal {
                    set_status(message);
                    return false;
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
                        set_status(crate::state::MSG_SIZE_PAIR_CROSSED.to_string());
                        return false;
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
                mirror_risk_settings(&ui, &config.borrow());
                let format = config.borrow().number_format;
                refresh_holdings(
                    &ui,
                    &journal_state.borrow(),
                    &holding_freshness.borrow(),
                    &holding_dismissed.borrow(),
                    format,
                );
                true
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_number_settings_are_shown_in_the_users_format_and_read_back() {
        let mut cfg = crate::config::AppConfig {
            default_trailing_stop_pct: Some("12.5".into()),
            withholding_rate_pct: Some("35".into()),
            ..Default::default()
        };
        cfg.number_format = NumberFormat::Comma;
        let comma = risk_settings_shown(&cfg);
        assert_eq!(comma.default_stop, "12,5");
        assert_eq!(comma.small_max, "1\u{00A0}000\u{00A0}000\u{00A0}000");
        cfg.number_format = NumberFormat::Point;
        let point = risk_settings_shown(&cfg);
        assert_eq!(point.default_stop, "12.5");
        assert_eq!(point.small_max, "1,000,000,000");
        // What is shown reads back to the stored value, under its own format.
        let valid = |v: &str| crate::config::is_valid_trailing_stop_pct(v);
        for (shown, format) in [(&comma, NumberFormat::Comma), (&point, NumberFormat::Point)] {
            assert_eq!(
                setting_input(&shown.default_stop, format, valid, "invalide"),
                Ok(Some("12.5".to_string()))
            );
            assert_eq!(
                setting_input(
                    &shown.small_max,
                    format,
                    crate::config::is_valid_size_bound,
                    "x"
                ),
                Ok(Some("1000000000".to_string()))
            );
        }
        // An unset default stop shows empty, and empty reads as « clear ».
        cfg.default_trailing_stop_pct = None;
        assert_eq!(risk_settings_shown(&cfg).default_stop, "");
        assert_eq!(setting_input("", NumberFormat::Point, valid, "x"), Ok(None));
    }

    #[test]
    fn a_setting_refuses_a_non_number_and_an_ambiguous_number_by_name() {
        let valid = |v: &str| crate::config::is_valid_trailing_stop_pct(v);
        assert_eq!(
            setting_input("abc", NumberFormat::Comma, valid, "invalide"),
            Err("invalide".to_string())
        );
        assert_eq!(
            setting_input("150", NumberFormat::Comma, valid, "invalide"),
            Err("invalide".to_string())
        );
        assert_eq!(
            setting_input("12,5", NumberFormat::Point, valid, "invalide"),
            Err(crate::state::MSG_NUMBER_AMBIGUOUS_POINT.to_string())
        );
        assert_eq!(
            setting_input("12.500", NumberFormat::Comma, valid, "invalide"),
            Err(crate::state::MSG_NUMBER_AMBIGUOUS_COMMA.to_string())
        );
    }
}
