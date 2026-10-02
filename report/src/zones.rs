//! FR63 — the three zone nouns of the active label set, as the NAIC-methodology reports print
//! them (owner decision, Guy 2026-10-01: the study PDF, the comparison PDF and the portfolio
//! review PDF follow the active label set, like their screens; only the screens that are not part
//! of the NAIC methodology — and their reports — stay neutral).
//!
//! The two sets live HERE, the one source the app's runtime label table (`app::labels::LABELS`)
//! reads its zone entries from — so a PDF never says another noun than the screen it mirrors.
//! The report crate never reads the app's configuration: the app passes the set in force.

/// The zone nouns of one label set, capitalised as a heading (« Zone basse »); a cell or a
/// sentence lowers them (« dans la zone basse »).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneNouns {
    /// The lower third (`buy` key, `[forecast_low, buy_top]`).
    pub low: &'static str,
    /// The middle third (`neutral` key, `(buy_top, neutral_top]`).
    pub middle: &'static str,
    /// The upper third (`sell` key, `(neutral_top, forecast_high]`).
    pub high: &'static str,
}

impl ZoneNouns {
    /// The NAIC set (in-app labels, personal use).
    pub const NAIC: ZoneNouns = ZoneNouns {
        low: "Zone d'achat",
        middle: "Zone de maintien",
        high: "Zone de vente",
    };
    /// The neutral set.
    pub const NEUTRAL: ZoneNouns = ZoneNouns {
        low: "Zone basse",
        middle: "Zone médiane",
        high: "Zone haute",
    };

    /// The noun of a zone key (`buy` | `neutral` | `sell`), as a heading; `None` for any other key.
    pub fn of_key(self, key: &str) -> Option<&'static str> {
        match key {
            "buy" => Some(self.low),
            "neutral" => Some(self.middle),
            "sell" => Some(self.high),
            _ => None,
        }
    }
}

/// Without the app (the examples, a headless render), the reports speak the neutral set — the
/// byte-identical output of the reports before FR63 reached them.
impl Default for ZoneNouns {
    fn default() -> Self {
        ZoneNouns::NEUTRAL
    }
}

/// A noun as it reads inside a cell or a sentence: lower case (« zone d'achat »).
pub(crate) fn lower(noun: &str) -> String {
    noun.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_sets_word_every_key_and_never_mix() {
        for set in [ZoneNouns::NAIC, ZoneNouns::NEUTRAL] {
            assert_eq!(set.of_key("buy"), Some(set.low));
            assert_eq!(set.of_key("neutral"), Some(set.middle));
            assert_eq!(set.of_key("sell"), Some(set.high));
            assert_eq!(set.of_key("below"), None);
            assert_eq!(set.of_key(""), None);
        }
        let naic = [
            ZoneNouns::NAIC.low,
            ZoneNouns::NAIC.middle,
            ZoneNouns::NAIC.high,
        ];
        for noun in [
            ZoneNouns::NEUTRAL.low,
            ZoneNouns::NEUTRAL.middle,
            ZoneNouns::NEUTRAL.high,
        ] {
            assert!(!naic.contains(&noun), "{noun} is in both sets");
        }
        assert_eq!(ZoneNouns::default(), ZoneNouns::NEUTRAL);
    }

    #[test]
    fn zone_nouns_are_neutral_no_banned_verb_no_wordmark() {
        use steadyinvest_core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR};
        for set in [ZoneNouns::NAIC, ZoneNouns::NEUTRAL] {
            for s in [set.low, set.middle, set.high] {
                let lower = s.to_lowercase();
                for token in lower.split(|c: char| !c.is_alphanumeric()) {
                    for banned in BANNED_VERBS_EN.iter().chain(BANNED_VERBS_FR.iter()) {
                        assert_ne!(token, banned.to_lowercase(), "{s:?} contains {banned:?}");
                    }
                }
                for mark in ["NAIC", "Stock Selection Guide", "Better Investing", "SSG"] {
                    assert!(!s.contains(mark), "{s:?} carries {mark:?}");
                }
                // WinAnsi-encodable (the PDFs' font): no glyph is lost to « ? ».
                assert!(
                    s.chars().all(|c| (c as u32) < 0x100),
                    "{s:?} leaves WinAnsi"
                );
            }
        }
    }
}
