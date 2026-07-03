//! The user's configured data provider (Story 3.2, FR63).
//!
//! A small string-enum mirroring [`crate::theme::Theme`] / [`crate::regime::Regime`]: persisted in
//! app-config as a stable kebab-case wire string, chosen in Réglages. Distinct from
//! `ingestion::Provider` (the concrete adapter dispatch) — this is the *choice*; the app maps it to
//! an adapter and to a per-provider keychain slot. The API **key** is never stored here (it lives
//! only in the OS secret store — NFR-S1); this records only *which* provider is preferred.

use serde::{Deserialize, Serialize};

/// Which data provider the app fetches from. EODHD is the only adapter today (Story 3.1); `None`
/// models "no provider / keyless" so a fetch runs with `api_key = None`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderChoice {
    /// EODHD (Story 3.1). Requires an API key.
    #[default]
    Eodhd,
    /// Twelve Data (Story 7.4) — the price-led second source. Requires an API key.
    TwelveData,
    /// No provider configured, or a keyless provider — the fetch path passes no key.
    None,
}

impl ProviderChoice {
    /// Parse a UI-callback / wire identifier; `None` for anything unknown (caller falls back to
    /// the default). The kebab-case strings match the serde representation so config and the UI
    /// agree.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "eodhd" => Some(ProviderChoice::Eodhd),
            "twelvedata" => Some(ProviderChoice::TwelveData),
            "none" => Some(ProviderChoice::None),
            _ => None,
        }
    }

    /// The stable wire string (UI mirror + keychain slot suffix). Twin of [`parse`].
    pub fn wire(self) -> &'static str {
        match self {
            ProviderChoice::Eodhd => "eodhd",
            ProviderChoice::TwelveData => "twelvedata",
            ProviderChoice::None => "none",
        }
    }

    /// The human-readable display name (Story 6.9 — the fallback notice names the effective
    /// provider). Proper nouns, matching the Réglages chips.
    pub fn display_name(self) -> &'static str {
        match self {
            ProviderChoice::Eodhd => "EODHD",
            ProviderChoice::TwelveData => "Twelve Data",
            ProviderChoice::None => "—",
        }
    }

    /// Whether a fetch with this provider needs an API key. A keyless/none provider does not, so
    /// the absence of a stored key is not an error for it (AC3).
    pub fn requires_key(self) -> bool {
        matches!(self, ProviderChoice::Eodhd | ProviderChoice::TwelveData)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_eodhd() {
        assert_eq!(ProviderChoice::default(), ProviderChoice::Eodhd);
    }

    #[test]
    fn parse_round_trips_with_wire() {
        for choice in [
            ProviderChoice::Eodhd,
            ProviderChoice::TwelveData,
            ProviderChoice::None,
        ] {
            assert_eq!(ProviderChoice::parse(choice.wire()), Some(choice));
        }
    }

    #[test]
    fn parse_unknown_is_none() {
        assert_eq!(ProviderChoice::parse("yahoo"), None);
        assert_eq!(ProviderChoice::parse(""), None);
    }

    #[test]
    fn serde_uses_kebab_case_identifiers() {
        assert_eq!(
            serde_json::to_string(&ProviderChoice::Eodhd).unwrap(),
            "\"eodhd\""
        );
        assert_eq!(
            serde_json::from_str::<ProviderChoice>("\"none\"").unwrap(),
            ProviderChoice::None
        );
    }

    #[test]
    fn keyed_providers_require_a_key_none_does_not() {
        assert!(ProviderChoice::Eodhd.requires_key());
        assert!(ProviderChoice::TwelveData.requires_key());
        assert!(!ProviderChoice::None.requires_key());
    }

    #[test]
    fn twelvedata_wire_drives_its_keychain_slot() {
        // The keychain slot is `provider:{wire}`, so the new variant gets its own slot for free.
        assert_eq!(ProviderChoice::TwelveData.wire(), "twelvedata");
    }
}
