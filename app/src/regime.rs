//! Reading-regime axis (Story 2.3, FR56): a study is read either in **entry** (Saisie — dense
//! filling/reading, every section open) or **contemplation** (judgment-moment focus — §1 + §4 open,
//! the rest folded). The regime is a **fold preset + a colour/marker token snapshot on CONSTANT
//! GEOMETRY** — switching it changes fold state and a regime-driven emphasis token only; never a row
//! height, font size or column width (UX-DR6, the metric/typo Tokens family is quasi-static).
//!
//! 2.3 ships the enum, the fold presets, the token-swap *mechanism* and the active-regime indicator.
//! The later deltas hang off this hook: ✓-marker attenuation is Story 2.5, zone saturation is 2.6 —
//! 2.3 deliberately does **not** invent those visuals (Dev Notes § "Regimes — exactly what 2.3
//! ships"). So the only token that actually moves here is [`Regime::emphasis`], a single alpha the
//! form reads to calm its editing affordances in contemplation.

use serde::{Deserialize, Serialize};

/// How many collapsible sections the form has (§1–§5). The fold preset is one bool per section.
pub const SECTION_COUNT: usize = 5;

/// The two reading regimes (FR56). `Entry` is the default — a freshly-opened study starts dense.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Regime {
    #[default]
    Entry,
    Contemplation,
}

impl Regime {
    /// Stable identifier used by the UI callback and the config file (serde matches this via
    /// `rename_all = "kebab-case"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Regime::Entry => "entry",
            Regime::Contemplation => "contemplation",
        }
    }

    /// Parse a UI-callback identifier; `None` for anything unknown (caller falls back).
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "entry" => Some(Regime::Entry),
            "contemplation" => Some(Regime::Contemplation),
            _ => None,
        }
    }

    /// The fold preset this regime applies to §1–§5 (`true` = open). Entry opens every section;
    /// contemplation keeps §1 (visual analysis) and §4 (risk/reward judgment) open and folds
    /// §2/§3/§5 to their information-scent summaries (the judgment-moment focus, UX spec).
    pub fn fold_preset(self) -> [bool; SECTION_COUNT] {
        match self {
            Regime::Entry => [true, true, true, true, true],
            //                §1    §2     §3     §4    §5
            Regime::Contemplation => [true, false, false, true, false],
        }
    }

    /// The regime-driven emphasis alpha (1.0 = full). Contemplation attenuates the editing
    /// affordances so the reader's eye rests on the open judgment sections; this is the single
    /// colour/alpha token the 2.3 form reads, and the hook 2.5/2.6 extend. Never a geometry value.
    pub fn emphasis(self) -> f32 {
        match self {
            Regime::Entry => 1.0,
            Regime::Contemplation => 0.55,
        }
    }
}

/// Push the regime-driven token snapshot into the `Tokens` global (the swap mechanism). Only an
/// alpha moves in 2.3 — constant geometry holds by construction (no size is touched here).
pub fn apply(ui: &crate::MainWindow, regime: Regime) {
    use slint::ComponentHandle;
    ui.global::<crate::Tokens>()
        .set_regime_emphasis(regime.emphasis());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_is_the_default_regime() {
        assert_eq!(Regime::default(), Regime::Entry);
    }

    #[test]
    fn identifiers_round_trip_and_reject_unknown() {
        for regime in [Regime::Entry, Regime::Contemplation] {
            assert_eq!(Regime::parse(regime.as_str()), Some(regime));
        }
        assert_eq!(Regime::parse("reading"), None);
    }

    #[test]
    fn serde_uses_kebab_case_identifiers() {
        assert_eq!(serde_json::to_string(&Regime::Entry).unwrap(), "\"entry\"");
        assert_eq!(
            serde_json::from_str::<Regime>("\"contemplation\"").unwrap(),
            Regime::Contemplation
        );
    }

    #[test]
    fn entry_preset_opens_every_section() {
        assert_eq!(Regime::Entry.fold_preset(), [true; SECTION_COUNT]);
    }

    #[test]
    fn contemplation_preset_keeps_only_sections_1_and_4_open() {
        // §1 (index 0) and §4 (index 3) open; §2/§3/§5 folded — the judgment-moment focus.
        assert_eq!(
            Regime::Contemplation.fold_preset(),
            [true, false, false, true, false]
        );
    }

    #[test]
    fn emphasis_is_full_in_entry_and_attenuated_in_contemplation() {
        assert_eq!(Regime::Entry.emphasis(), 1.0);
        assert!(Regime::Contemplation.emphasis() < 1.0);
        // It stays a sane alpha — never a geometry value, never out of [0,1].
        assert!((0.0..=1.0).contains(&Regime::Contemplation.emphasis()));
    }
}
