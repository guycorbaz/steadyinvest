//! NAIC↔neutral label set (FR63) — a runtime-swappable DATA TABLE of method vocabulary, keyed
//! by stable identifiers. Strictly independent from the UI-language axis: `@tr()` translates UI
//! chrome at compile time; this table swaps live, no restart.
//!
//! Seed (Story 2.1, dev discretion — recorded in the Dev Agent Record): the method term itself
//! plus the three judgment-zone nouns, the only method vocabulary that exists in the UI today.
//! Later stories extend the table; every key MUST be defined in both sets (tested below).
//!
//! Where the set applies (owner decision, Guy 2026-10-01 — it replaces the 2026-09-26 one that
//! kept the comparison and the PDFs neutral): every screen of the NAIC methodology and its PDF —
//! the study (SSG) and its PDF, the comparison (Stock Comparison Guide) and its PDF, the portfolio
//! review and its PDF. The screens outside the methodology stay neutral: the quick screen, the
//! criblage, the holdings (« Portefeuilles »), the watchlist, the AI proposals — and the MCP
//! server's codes (low / middle / high). The zone nouns live in the report crate
//! ([`ZoneNouns`]), the one source the PDFs and this table share.

use serde::{Deserialize, Serialize};
use steadyinvest_report::ZoneNouns;

/// Which label set is active. NAIC terms are kept as in-app labels (personal use); the neutral
/// set is the swappable replacement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LabelSet {
    #[default]
    Naic,
    Neutral,
}

impl LabelSet {
    /// Stable identifier used by the UI callbacks and the config file.
    pub fn as_str(self) -> &'static str {
        match self {
            LabelSet::Naic => "naic",
            LabelSet::Neutral => "neutral",
        }
    }

    /// Parse a UI-callback identifier; `None` for anything unknown (caller falls back).
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "naic" => Some(LabelSet::Naic),
            "neutral" => Some(LabelSet::Neutral),
            _ => None,
        }
    }
}

/// One vocabulary entry: a stable key and its display string in each set.
pub struct LabelEntry {
    pub key: &'static str,
    pub naic: &'static str,
    pub neutral: &'static str,
}

/// The whole table. Keys are stable identifiers (never displayed); zone labels name the defined
/// price bands — nouns, not imperatives (core::method exemption note).
pub const LABELS: [LabelEntry; 4] = [
    LabelEntry {
        key: "study-method",
        naic: "SSG (Stock Selection Guide)",
        neutral: "Étude d'action",
    },
    LabelEntry {
        key: "zone-buy",
        naic: ZoneNouns::NAIC.low,
        neutral: ZoneNouns::NEUTRAL.low,
    },
    LabelEntry {
        key: "zone-hold",
        naic: ZoneNouns::NAIC.middle,
        neutral: ZoneNouns::NEUTRAL.middle,
    },
    LabelEntry {
        key: "zone-sell",
        naic: ZoneNouns::NAIC.high,
        neutral: ZoneNouns::NEUTRAL.high,
    },
];

/// The zone nouns of `set`, as the NAIC-methodology PDFs print them (FR63, 2026-10-01): the
/// study, comparison and review exports pass the set in force at export time.
pub fn zone_nouns(set: LabelSet) -> ZoneNouns {
    match set {
        LabelSet::Naic => ZoneNouns::NAIC,
        LabelSet::Neutral => ZoneNouns::NEUTRAL,
    }
}

/// Resolve one key in the given set. `None` for an unknown key — callers in `app` use the
/// statically-known keys below, so a `None` is a programming error surfaced by the tests.
pub fn label(set: LabelSet, key: &str) -> Option<&'static str> {
    LABELS
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| match set {
            LabelSet::Naic => entry.naic,
            LabelSet::Neutral => entry.neutral,
        })
}

/// Push the resolved strings of `set` into the `Labels` Slint global (live swap, no restart).
pub fn apply(ui: &crate::MainWindow, set: LabelSet) {
    use slint::ComponentHandle;
    let labels = ui.global::<crate::Labels>();
    let resolve = |key| slint::SharedString::from(label(set, key).expect("seed key defined"));
    labels.set_study_method(resolve("study-method"));
    labels.set_zone_buy(resolve("zone-buy"));
    labels.set_zone_hold(resolve("zone-hold"));
    labels.set_zone_sell(resolve("zone-sell"));
}

/// Push the NEUTRAL zone nouns into the `NeutralZones` Slint global, once, before the window shows.
/// FR63 (owner decision, Guy 2026-10-01): the screens outside the NAIC methodology — the holdings
/// and the watchlist — word the zones in the neutral set whatever the active one. The nouns are the
/// neutral set's ([`ZoneNouns::NEUTRAL`]), the one source this table also reads — never a second
/// copy. A label-set swap never touches them.
pub fn apply_neutral_zones(ui: &crate::MainWindow) {
    use slint::ComponentHandle;
    let zones = ui.global::<crate::NeutralZones>();
    let nouns = neutral_zone_nouns();
    zones.set_zone_buy(nouns.low.into());
    zones.set_zone_hold(nouns.middle.into());
    zones.set_zone_sell(nouns.high.into());
}

/// The zone nouns of the screens outside the NAIC methodology: the neutral set, whatever the
/// active one (FR63, 2026-10-01).
pub fn neutral_zone_nouns() -> ZoneNouns {
    zone_nouns(LabelSet::Neutral)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_defined_in_both_sets_no_silent_fallback() {
        for entry in &LABELS {
            assert!(
                !entry.naic.trim().is_empty(),
                "key {:?} has an empty NAIC label",
                entry.key
            );
            assert!(
                !entry.neutral.trim().is_empty(),
                "key {:?} has an empty neutral label",
                entry.key
            );
        }
    }

    #[test]
    fn keys_are_unique() {
        for (i, a) in LABELS.iter().enumerate() {
            for b in &LABELS[i + 1..] {
                assert_ne!(a.key, b.key, "duplicate label key");
            }
        }
    }

    #[test]
    fn lookup_resolves_per_set_and_rejects_unknown_keys() {
        assert_eq!(
            label(LabelSet::Naic, "study-method"),
            Some("SSG (Stock Selection Guide)")
        );
        assert_eq!(
            label(LabelSet::Neutral, "study-method"),
            Some("Étude d'action")
        );
        assert_eq!(label(LabelSet::Naic, "no-such-key"), None);
    }

    #[test]
    fn the_pdfs_zone_nouns_are_the_screens() {
        // One vocabulary: the PDF of a NAIC-methodology screen says the screen's nouns.
        for set in [LabelSet::Naic, LabelSet::Neutral] {
            let nouns = zone_nouns(set);
            assert_eq!(label(set, "zone-buy"), Some(nouns.low));
            assert_eq!(label(set, "zone-hold"), Some(nouns.middle));
            assert_eq!(label(set, "zone-sell"), Some(nouns.high));
        }
        assert_eq!(label(LabelSet::Naic, "zone-buy"), Some("Zone d'achat"));
        assert_eq!(label(LabelSet::Neutral, "zone-buy"), Some("Zone basse"));
    }

    #[test]
    fn the_neutral_screens_nouns_are_the_neutral_sets_whatever_the_active_one() {
        // FR63 (2026-10-01): holdings and watchlist never follow the active set.
        assert_eq!(neutral_zone_nouns(), ZoneNouns::NEUTRAL);
        assert_eq!(
            neutral_zone_nouns().low,
            label(LabelSet::Neutral, "zone-buy").unwrap()
        );
        assert_eq!(
            neutral_zone_nouns().middle,
            label(LabelSet::Neutral, "zone-hold").unwrap()
        );
        assert_eq!(
            neutral_zone_nouns().high,
            label(LabelSet::Neutral, "zone-sell").unwrap()
        );
        assert_ne!(
            neutral_zone_nouns().low,
            label(LabelSet::Naic, "zone-buy").unwrap()
        );
    }

    #[test]
    fn identifiers_round_trip() {
        for set in [LabelSet::Naic, LabelSet::Neutral] {
            assert_eq!(LabelSet::parse(set.as_str()), Some(set));
        }
        assert_eq!(LabelSet::parse("klingon"), None);
    }
}
