//! NAIC↔neutral label set (FR63) — a runtime-swappable DATA TABLE of method vocabulary, keyed
//! by stable identifiers. Strictly independent from the UI-language axis: `@tr()` translates UI
//! chrome at compile time; this table swaps live, no restart.
//!
//! Seed (Story 2.1, dev discretion — recorded in the Dev Agent Record): the method term itself
//! plus the three judgment-zone nouns, the only method vocabulary that exists in the UI today.
//! Later stories extend the table; every key MUST be defined in both sets (tested below).

use serde::{Deserialize, Serialize};

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
        naic: "Zone d'achat",
        neutral: "Zone basse",
    },
    LabelEntry {
        key: "zone-hold",
        naic: "Zone de maintien",
        neutral: "Zone médiane",
    },
    LabelEntry {
        key: "zone-sell",
        naic: "Zone de vente",
        neutral: "Zone haute",
    },
];

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
    fn identifiers_round_trip() {
        for set in [LabelSet::Naic, LabelSet::Neutral] {
            assert_eq!(LabelSet::parse(set.as_str()), Some(set));
        }
        assert_eq!(LabelSet::parse("klingon"), None);
    }
}
