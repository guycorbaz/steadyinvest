//! AI draft types (Epic 8, Phase 4 — arch §Phase 4 A4, Story 8.2a).
//!
//! A draft is a proposal an AI client submits through MCP (Story 8.3); it waits in the dossier's
//! `ai_drafts` table and changes nothing until the owner decides it (Story 8.2b). **Every variant is
//! defined up front** (arch A4): a later variant would be a contract change, and — per the enum
//! policy of this crate — an unknown value fails to parse on purpose (no `serde(other)`).
//!
//! The draftable field enumeration, units and the fingerprint are Story 8.2b's; here `field` is a
//! plain string and `base_fingerprint` an optional opaque text.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// What a draft proposes. Stored in `ai_drafts.kind` with the exact spellings of [`Self::as_str`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftKind {
    /// A new study for a security (FR70).
    Study,
    /// A note on an existing study (FR71).
    Note,
    /// A data-cell value of an existing study (FR72).
    Cell,
    /// A judgment value of an existing study (FR72).
    Judgment,
}

/// Where a draft stands. Stored in `ai_drafts.status` with the exact spellings of [`Self::as_str`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftStatus {
    /// Waiting for the owner's decision (changes nothing — FR72).
    Pending,
    /// Validated by the owner (FR74).
    Validated,
    /// Validated, then the validation was undone (FR74, FR77 — D3).
    ValidatedUndone,
    /// Rejected by the owner (FR74).
    Rejected,
}

/// An unknown `kind` / `status` spelling — the fail-loud parse of a stored column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownDraftValue(pub String);

impl fmt::Display for UnknownDraftValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown draft value {:?}", self.0)
    }
}

impl std::error::Error for UnknownDraftValue {}

impl DraftKind {
    /// Every variant, in declaration order.
    pub const ALL: [DraftKind; 4] = [
        DraftKind::Study,
        DraftKind::Note,
        DraftKind::Cell,
        DraftKind::Judgment,
    ];

    /// The stored spelling (identical to the serde spelling).
    pub fn as_str(self) -> &'static str {
        match self {
            DraftKind::Study => "study",
            DraftKind::Note => "note",
            DraftKind::Cell => "cell",
            DraftKind::Judgment => "judgment",
        }
    }
}

impl FromStr for DraftKind {
    type Err = UnknownDraftValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        DraftKind::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| UnknownDraftValue(s.to_string()))
    }
}

impl DraftStatus {
    /// Every variant, in declaration order.
    pub const ALL: [DraftStatus; 4] = [
        DraftStatus::Pending,
        DraftStatus::Validated,
        DraftStatus::ValidatedUndone,
        DraftStatus::Rejected,
    ];

    /// The stored spelling (identical to the serde spelling).
    pub fn as_str(self) -> &'static str {
        match self {
            DraftStatus::Pending => "pending",
            DraftStatus::Validated => "validated",
            DraftStatus::ValidatedUndone => "validated_undone",
            DraftStatus::Rejected => "rejected",
        }
    }
}

impl FromStr for DraftStatus {
    type Err = UnknownDraftValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        DraftStatus::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| UnknownDraftValue(s.to_string()))
    }
}

/// The target of a cell or judgment draft (arch A4). `field` is validated against the draftable
/// fields by Story 8.2b / 8.3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum DraftTarget {
    /// A data cell of one fiscal year.
    Cell { fiscal_year: i32, field: String },
    /// A judgment field.
    Judgment { field: String },
}

/// The version of [`DraftPayload`] this build writes and reads. A payload of another version is a
/// fail-loud refusal (corrupt on read, malformed on import).
pub const DRAFT_PAYLOAD_VERSION: u32 = 1;

/// The versioned JSON stored in `ai_drafts.payload` (arch A4). Which fields a kind carries is the
/// shape rule of [`DraftPayload::fits`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftPayload {
    /// Always [`DRAFT_PAYLOAD_VERSION`] when written by this build.
    pub version: u32,
    /// The target of a cell / judgment draft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<DraftTarget>,
    /// The proposed value of a cell / judgment draft, as text (a decimal, or an enum variant name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_value: Option<String>,
    /// The proposed text of a note draft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_text: Option<String>,
    /// The optional company name of a draft study (D2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company_name: Option<String>,
    /// The target's fingerprint at submission (Story 8.2b, arch A7); opaque here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_fingerprint: Option<String>,
}

impl DraftPayload {
    /// The shape rule (Story 8.2a, Dev Notes §3): which fields a draft of `kind` carries.
    ///
    /// | kind | target | proposed_value | note_text | company_name |
    /// |---|---|---|---|---|
    /// | study | — | — | — | optional |
    /// | note | — | — | required | — |
    /// | cell | `Cell` | required | — | — |
    /// | judgment | `Judgment` | required | — | — |
    ///
    /// `base_fingerprint` is tolerated on cell / judgment drafts only.
    pub fn fits(&self, kind: DraftKind) -> bool {
        if self.version != DRAFT_PAYLOAD_VERSION {
            return false;
        }
        match kind {
            DraftKind::Study => {
                self.target.is_none()
                    && self.proposed_value.is_none()
                    && self.note_text.is_none()
                    && self.base_fingerprint.is_none()
            }
            DraftKind::Note => {
                self.target.is_none()
                    && self.proposed_value.is_none()
                    && self.note_text.is_some()
                    && self.company_name.is_none()
                    && self.base_fingerprint.is_none()
            }
            DraftKind::Cell => {
                matches!(self.target, Some(DraftTarget::Cell { .. }))
                    && self.proposed_value.is_some()
                    && self.note_text.is_none()
                    && self.company_name.is_none()
            }
            DraftKind::Judgment => {
                matches!(self.target, Some(DraftTarget::Judgment { .. }))
                    && self.proposed_value.is_some()
                    && self.note_text.is_none()
                    && self.company_name.is_none()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> DraftPayload {
        DraftPayload {
            version: DRAFT_PAYLOAD_VERSION,
            target: None,
            proposed_value: None,
            note_text: None,
            company_name: None,
            base_fingerprint: None,
        }
    }

    fn study(name: Option<&str>) -> DraftPayload {
        DraftPayload {
            company_name: name.map(str::to_string),
            ..empty()
        }
    }

    fn note() -> DraftPayload {
        DraftPayload {
            note_text: Some("Marge en hausse.".to_string()),
            ..empty()
        }
    }

    fn cell() -> DraftPayload {
        DraftPayload {
            target: Some(DraftTarget::Cell {
                fiscal_year: 2024,
                field: "eps".to_string(),
            }),
            proposed_value: Some("3.15".to_string()),
            base_fingerprint: Some("abc".to_string()),
            ..empty()
        }
    }

    fn judgment() -> DraftPayload {
        DraftPayload {
            target: Some(DraftTarget::Judgment {
                field: "judged_avg_low_pe".to_string(),
            }),
            proposed_value: Some("11.5".to_string()),
            ..empty()
        }
    }

    #[test]
    fn kind_and_status_spellings_are_the_serde_spellings() {
        for k in DraftKind::ALL {
            assert_eq!(
                serde_json::to_string(&k).expect("serializes"),
                format!("\"{}\"", k.as_str())
            );
            assert_eq!(k.as_str().parse::<DraftKind>(), Ok(k));
        }
        for s in DraftStatus::ALL {
            assert_eq!(
                serde_json::to_string(&s).expect("serializes"),
                format!("\"{}\"", s.as_str())
            );
            assert_eq!(s.as_str().parse::<DraftStatus>(), Ok(s));
        }
        assert_eq!(DraftStatus::ValidatedUndone.as_str(), "validated_undone");
    }

    #[test]
    fn unknown_values_fail_loud() {
        assert!("proposal".parse::<DraftKind>().is_err());
        assert!("stale".parse::<DraftStatus>().is_err());
        assert!(serde_json::from_str::<DraftKind>("\"proposal\"").is_err());
        assert!(serde_json::from_str::<DraftStatus>("\"stale\"").is_err());
        assert!(
            serde_json::from_str::<DraftTarget>(r#"{"target":"note","field":"x"}"#).is_err(),
            "an unknown target tag does not parse"
        );
    }

    #[test]
    fn payload_json_is_pinned_per_kind() {
        assert_eq!(
            serde_json::to_string(&study(Some("Nestlé"))).expect("serializes"),
            r#"{"version":1,"company_name":"Nestlé"}"#
        );
        assert_eq!(
            serde_json::to_string(&note()).expect("serializes"),
            r#"{"version":1,"note_text":"Marge en hausse."}"#
        );
        assert_eq!(
            serde_json::to_string(&cell()).expect("serializes"),
            r#"{"version":1,"target":{"target":"cell","fiscal_year":2024,"field":"eps"},"proposed_value":"3.15","base_fingerprint":"abc"}"#
        );
        assert_eq!(
            serde_json::to_string(&judgment()).expect("serializes"),
            r#"{"version":1,"target":{"target":"judgment","field":"judged_avg_low_pe"},"proposed_value":"11.5"}"#
        );
    }

    #[test]
    fn payload_round_trips() {
        for p in [study(None), study(Some("X")), note(), cell(), judgment()] {
            let json = serde_json::to_string(&p).expect("serializes");
            let back: DraftPayload = serde_json::from_str(&json).expect("parses");
            assert_eq!(back, p);
        }
    }

    #[test]
    fn each_kind_fits_only_its_own_shape() {
        let shapes = [
            (DraftKind::Study, study(Some("X"))),
            (DraftKind::Note, note()),
            (DraftKind::Cell, cell()),
            (DraftKind::Judgment, judgment()),
        ];
        for (shape_kind, payload) in &shapes {
            for kind in DraftKind::ALL {
                assert_eq!(
                    payload.fits(kind),
                    kind == *shape_kind,
                    "a {shape_kind:?}-shaped payload fits {kind:?}"
                );
            }
        }
        assert!(study(None).fits(DraftKind::Study), "the name is optional");
    }

    #[test]
    fn a_missing_required_field_or_a_foreign_version_does_not_fit() {
        assert!(!empty().fits(DraftKind::Note), "a note needs its text");
        let no_value = DraftPayload {
            proposed_value: None,
            ..cell()
        };
        assert!(!no_value.fits(DraftKind::Cell));
        let wrong_target = DraftPayload {
            target: Some(DraftTarget::Judgment {
                field: "x".to_string(),
            }),
            ..cell()
        };
        assert!(!wrong_target.fits(DraftKind::Cell));
        let fingerprinted_note = DraftPayload {
            base_fingerprint: Some("f".to_string()),
            ..note()
        };
        assert!(!fingerprinted_note.fits(DraftKind::Note));
        let v2 = DraftPayload {
            version: DRAFT_PAYLOAD_VERSION + 1,
            ..note()
        };
        assert!(!v2.fits(DraftKind::Note), "a foreign version never fits");
    }
}
