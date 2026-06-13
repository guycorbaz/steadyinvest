//! Canonical theme state (UX-DR7): the Rust side owns which theme is active and pushes the
//! matching colour/alpha scale into the `Tokens` Slint global — the single neutral source of
//! truth every component reads. Only the colour/alpha family swaps at runtime; the metric/typo
//! family in `tokens.slint` is quasi-static, so a switch causes no geometry change (UX-DR6).
//!
//! No `arc_swap` here on purpose: Slint properties are main-thread, a plain owner pushing into
//! the global is the v1 mechanism (architecture note in Story 2.1).

use serde::{Deserialize, Serialize};

/// The two shipped themes. Dark is the default (UX-DR1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    /// Parse a UI-callback identifier; `None` for anything unknown (caller falls back).
    /// (No `as_str` twin here: the UI mirrors the theme as the `dark-theme` bool, and the
    /// config file goes through serde.)
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "dark" => Some(Theme::Dark),
            "light" => Some(Theme::Light),
            _ => None,
        }
    }

    pub fn palette(self) -> &'static Palette {
        match self {
            Theme::Dark => &DARK,
            Theme::Light => &LIGHT,
        }
    }
}

/// One full colour/alpha scale (`0xRRGGBB`). Pure data so it stays unit-testable headless;
/// the push into Slint happens in [`apply`].
pub struct Palette {
    pub bg: u32,
    pub surface: u32,
    pub surface_alt: u32,
    pub separator: u32,
    pub text_high: u32,
    pub text_mid: u32,
    pub text_low: u32,
    pub zone_buy: u32,
    pub zone_hold: u32,
    pub zone_sell: u32,
    /// Judgment-zone fill alpha — dark 32–40 %, light 15–18 % (UX spec).
    pub zone_alpha: f32,
    /// Keyboard focus-ring ink (NFR-U2).
    pub focus: u32,
    /// Cell grid-line ink for the faithful SSG tables (Story 2.3) — a hair away from `separator`
    /// so the 1 px borders read as a grid against the surface in both themes.
    pub grid_line: u32,
    /// Active-cell cursor surface (Story 2.4): a brighter neutral fill behind the focused entry
    /// cell, paired with the `focus` ink ring — the visible cell cursor, no colour spent.
    pub cell_active: u32,
    /// Shouting to-fill gap ink (Story 2.4): a strong neutral so a hole in the grid is loud (UX
    /// "missing shouts"). Still ink — never a judgment hue.
    pub gap_ink: u32,
}

/// Dark ink scale (the default).
pub const DARK: Palette = Palette {
    bg: 0x0E0F12,
    surface: 0x16181D,
    surface_alt: 0x1C1F26,
    separator: 0x2A2E37,
    text_high: 0xECEEF2,
    text_mid: 0xB8BDC7,
    text_low: 0x8A8F98,
    zone_buy: 0x009E73,
    zone_hold: 0xE69F00,
    zone_sell: 0xD55E00,
    zone_alpha: 0.36,
    focus: 0xB8BDC7,
    grid_line: 0x343842,
    cell_active: 0x232A38,
    gap_ink: 0xECEEF2,
};

/// Light ink scale.
pub const LIGHT: Palette = Palette {
    bg: 0xFBFBFC,
    surface: 0xFFFFFF,
    surface_alt: 0xF4F5F7,
    separator: 0xE2E4E9,
    text_high: 0x14161A,
    text_mid: 0x3F454F,
    text_low: 0x6B7280,
    zone_buy: 0x009E73,
    zone_hold: 0xE69F00,
    zone_sell: 0xD55E00,
    zone_alpha: 0.165,
    focus: 0x3F454F,
    grid_line: 0xD2D5DC,
    cell_active: 0xE7ECF4,
    gap_ink: 0x14161A,
};

fn color(rgb: u32) -> slint::Color {
    slint::Color::from_argb_encoded(0xFF00_0000 | rgb)
}

/// Push the theme's whole colour/alpha family into the `Tokens` global. Property updates
/// trigger the redraw; no restart, no geometry change.
pub fn apply(ui: &crate::MainWindow, theme: Theme) {
    use slint::ComponentHandle;
    let palette = theme.palette();
    let tokens = ui.global::<crate::Tokens>();
    tokens.set_dark(theme == Theme::Dark);
    tokens.set_bg(color(palette.bg));
    tokens.set_surface(color(palette.surface));
    tokens.set_surface_alt(color(palette.surface_alt));
    tokens.set_separator(color(palette.separator));
    tokens.set_text_high(color(palette.text_high));
    tokens.set_text_mid(color(palette.text_mid));
    tokens.set_text_low(color(palette.text_low));
    tokens.set_zone_buy(color(palette.zone_buy));
    tokens.set_zone_hold(color(palette.zone_hold));
    tokens.set_zone_sell(color(palette.zone_sell));
    tokens.set_zone_alpha(palette.zone_alpha);
    tokens.set_focus(color(palette.focus));
    tokens.set_grid_line(color(palette.grid_line));
    tokens.set_cell_active(color(palette.cell_active));
    tokens.set_gap_ink(color(palette.gap_ink));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_is_the_default_theme() {
        assert_eq!(Theme::default(), Theme::Dark);
    }

    #[test]
    fn ui_identifiers_parse() {
        assert_eq!(Theme::parse("dark"), Some(Theme::Dark));
        assert_eq!(Theme::parse("light"), Some(Theme::Light));
        assert_eq!(Theme::parse("solarized"), None);
    }

    #[test]
    fn serde_uses_kebab_case_identifiers() {
        assert_eq!(serde_json::to_string(&Theme::Dark).unwrap(), "\"dark\"");
        assert_eq!(
            serde_json::from_str::<Theme>("\"light\"").unwrap(),
            Theme::Light
        );
    }

    #[test]
    fn zone_hues_are_identical_across_themes() {
        // Okabe-Ito judgment hues never change with the theme; only the alpha does.
        assert_eq!(DARK.zone_buy, LIGHT.zone_buy);
        assert_eq!(DARK.zone_hold, LIGHT.zone_hold);
        assert_eq!(DARK.zone_sell, LIGHT.zone_sell);
    }

    #[test]
    fn zone_alphas_stay_within_the_spec_ranges() {
        assert!((0.32..=0.40).contains(&DARK.zone_alpha));
        assert!((0.15..=0.18).contains(&LIGHT.zone_alpha));
    }

    #[test]
    fn ink_scales_match_the_ux_spec() {
        assert_eq!(DARK.bg, 0x0E0F12);
        assert_eq!(DARK.text_high, 0xECEEF2);
        assert_eq!(LIGHT.bg, 0xFBFBFC);
        assert_eq!(LIGHT.text_high, 0x14161A);
    }
}
