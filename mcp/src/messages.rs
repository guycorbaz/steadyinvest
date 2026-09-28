//! The French messages of every refusal and failure the MCP server returns (Story 8.4 AC 10 / 11;
//! Story 8.0 §3.3, decision Q11: a stable English `snake_case` code + a French message).
//!
//! Every text is the §3.3 wording **verbatim**, its placeholders filled from the typed data of the
//! outcome. The matches are exhaustive (no wildcard): a new refusal, unavailability reason or MCP
//! error variant does not compile until it has a message here.

use std::path::Path;
use steadyinvest_contract::{DraftField, DraftUnit, option_name};
use steadyinvest_paths::ResolveError;
use steadyinvest_persistence::{Error, McpUnavailable, SubmissionRefusal, SubmitError};

/// A rendered outcome: the stable code and its French message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub code: &'static str,
    pub message: String,
}

impl Rendered {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Rendered {
            code,
            message: message.into(),
        }
    }
}

/// The French description of a draftable field's unit (the `{unité}` placeholder, and the tool
/// schema's per-field line).
pub fn unit_fr(field: DraftField) -> String {
    match field.unit() {
        DraftUnit::Amount => {
            "montant absolu dans la devise de l'étude, pas en millions".to_string()
        }
        DraftUnit::PerShare => "montant par action dans la devise de l'étude".to_string(),
        DraftUnit::Price => "cours dans la devise de l'étude".to_string(),
        DraftUnit::Percent => "pour cent, 12 pour 12 %".to_string(),
        DraftUnit::Ratio => "multiple".to_string(),
        DraftUnit::Option => format!("une option parmi {}", options_list(field)),
    }
}

/// The option names of an option field, comma-separated (the `{options}` placeholder).
pub fn options_list(field: DraftField) -> String {
    field
        .options()
        .iter()
        .map(|o| option_name(*o))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The identifier rule a draft study's ticker and currency follow (the `{règle}` placeholder —
/// Story 8.0 §4.1, enforced by `McpAccess`).
pub const IDENTIFIER_RULE_FR: &str = "symbole : 1 à 20 caractères parmi les lettres majuscules A-Z, \
     les chiffres, « . » et « - » ; devise : trois lettres majuscules";

/// How much of a client- or AI-written text a message echoes (G3): at most this many characters.
pub const ECHO_MAX_CHARS: usize = 40;

/// A client- or AI-written text as a message echoes it (G3): control characters (newlines, tabs…)
/// replaced by a space, and cut to [`ECHO_MAX_CHARS`] with « … ».
pub fn echo(text: &str) -> String {
    let clean: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let mut chars = clean.chars();
    let head: String = chars.by_ref().take(ECHO_MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// The French noun phrase of a capped text (the `{champ}` placeholder of `text_too_long`).
fn capped_text_fr(field: &str) -> &'static str {
    match field {
        "comment" => "du commentaire",
        "note_text" => "de la note",
        "company_name" => "du nom de la société",
        "origin_client" => "du client",
        "origin_model" => "du modèle",
        "proposed_value" => "de la valeur proposée",
        _ => "proposé",
    }
}

/// A refusal of the AI's proposal (Story 8.3 `SubmissionRefusal`).
pub fn refusal(r: &SubmissionRefusal) -> Rendered {
    let code = r.code();
    let message = match r {
        // Equal paths (a restored or replaced file) name the journal ids instead (G3 decision).
        SubmissionRefusal::DossierMismatch { read, current } if read.path == current.path => {
            format!(
                "Le dossier a changé depuis la lecture ({} ≠ {}) ; rien n'a été enregistré.",
                read.journal_id, current.journal_id
            )
        }
        SubmissionRefusal::DossierMismatch { read, current } => format!(
            "Le dossier a changé depuis la lecture ({} ≠ {}) ; rien n'a été enregistré.",
            echo(&read.path.display().to_string()),
            current.path.display()
        ),
        SubmissionRefusal::DossierReplaced => "Le fichier du dossier a été remplacé pendant \
             l'écriture (restauration) ; rien n'a été enregistré."
            .to_string(),
        SubmissionRefusal::StudyNotFound { study_id } => {
            format!("L'étude {study_id} n'existe pas dans ce dossier ; rien n'a été enregistré.")
        }
        SubmissionRefusal::StudyArchived { ticker } => {
            format!("L'étude {ticker} est archivée ; la proposition n'a pas été enregistrée.")
        }
        SubmissionRefusal::EmptyComment => {
            "Le commentaire est obligatoire ; rien n'a été enregistré.".to_string()
        }
        SubmissionRefusal::MissingOrigin => {
            "Le client et le modèle sont obligatoires ; rien n'a été enregistré.".to_string()
        }
        SubmissionRefusal::TextTooLong { field, max, len } => format!(
            "Le texte {} dépasse {max} caractères ({len}) ; rien n'a été enregistré.",
            capped_text_fr(field)
        ),
        SubmissionRefusal::IdentifierInvalid { .. } => format!(
            "Le symbole ou la devise proposés ne sont pas valides ({IDENTIFIER_RULE_FR}) ; rien \
             n'a été enregistré."
        ),
        SubmissionRefusal::FieldNotDraftable { field } => format!(
            "Le champ {} ne peut pas être proposé (voir la liste des champs du schéma de \
             l'outil) ; rien n'a été enregistré.",
            echo(field)
        ),
        SubmissionRefusal::YearNotInStudy { year, ticker } => format!(
            "L'année {year} n'existe pas dans l'étude {ticker} ; une proposition n'ajoute jamais \
             d'année."
        ),
        SubmissionRefusal::ValueUnparsable { text, field } => format!(
            "La valeur « {} » n'est pas un nombre dans l'unité de {} ({}) ; rien n'a été \
             enregistré.",
            echo(text),
            field.key(),
            unit_fr(*field)
        ),
        SubmissionRefusal::ValueNotAnOption { text, field } => format!(
            "La valeur « {} » n'est pas une option de {} ({}) ; rien n'a été enregistré.",
            echo(text),
            field.key(),
            options_list(*field)
        ),
        SubmissionRefusal::ValueOutOfRange { text, field } => format!(
            "La valeur « {} » de {} est hors des bornes d'une proposition (moins de 10¹⁵ en \
             valeur absolue, au plus 10 décimales) ; rien n'a été enregistré.",
            echo(text),
            field.key()
        ),
        SubmissionRefusal::EmptyNoteText => {
            "Le texte de la note est vide ; rien n'a été enregistré.".to_string()
        }
        SubmissionRefusal::TargetHasPending {
            field, fiscal_year, ..
        } => {
            let target = match fiscal_year {
                Some(year) => format!("Le champ {} de l'année {year}", field.key()),
                None => format!("Le champ {}", field.key()),
            };
            format!("{target} a déjà une proposition en attente ; rien n'a été enregistré.")
        }
        SubmissionRefusal::StudyExists {
            ticker, currency, ..
        } => format!("L'étude {ticker} en {currency} existe déjà ; rien n'a été enregistré."),
        SubmissionRefusal::DraftStudyPending {
            ticker, currency, ..
        } => format!(
            "Une proposition d'étude {ticker} en {currency} est déjà en attente ; rien n'a été \
             enregistré."
        ),
        SubmissionRefusal::DraftIdConflict { id } => format!(
            "Une autre proposition porte déjà l'identifiant {id} ; rien n'a été enregistré."
        ),
    };
    Rendered::new(code, message)
}

/// Why the dossier could not be used at all (Story 8.3 `McpUnavailable`). `path` is the resolved
/// dossier path (the `{chemin}` placeholder of `not_a_dossier`).
pub fn unavailable(reason: &McpUnavailable, path: &Path) -> Rendered {
    let code = reason.code();
    let message = match reason {
        McpUnavailable::Missing => {
            "Aucun dossier n'a pu être déterminé ; rien n'a été lu.".to_string()
        }
        McpUnavailable::NotADossier => format!(
            "Le fichier {} n'est pas un dossier SteadyInvest ; rien n'a été lu.",
            path.display()
        ),
        McpUnavailable::RestoreInProgress => {
            "Une restauration du dossier est en cours ; rien n'a été lu ni enregistré.".to_string()
        }
        McpUnavailable::RestoreInterrupted => "Une restauration du dossier a été interrompue ; \
             ouvrez le dossier dans l'application, qui la termine ; rien n'a été lu ni enregistré."
            .to_string(),
        McpUnavailable::Busy => "Le dossier est resté occupé par une autre écriture au-delà du \
             délai d'attente ; rien n'a été lu ni enregistré."
            .to_string(),
        McpUnavailable::NeedsRecovery => "Le dossier doit d'abord être ouvert dans l'application \
             (reprise après une interruption) ; rien n'a été lu."
            .to_string(),
        McpUnavailable::Protected { directory } => format!(
            "{} ; il ne peut pas être lu sans y créer de fichiers, ou rien ne peut y être \
             enregistré.",
            if *directory {
                "Le dossier qui contient le fichier est protégé en écriture"
            } else {
                "Le fichier du dossier est protégé en écriture"
            }
        ),
        McpUnavailable::IdentityUnreadable { .. } => "L'identité du fichier du dossier n'a pas \
             pu être lue ; rien n'a été enregistré."
            .to_string(),
    };
    Rendered::new(code, message)
}

/// The `invalid_call` message (a server-side defect or a malformed tool call — never an AI
/// proposal's fault).
pub fn invalid_call(detail: &str) -> Rendered {
    Rendered::new(
        "invalid_call",
        format!("L'appel au serveur MCP est mal formé ({detail}) ; rien n'a été enregistré."),
    )
}

/// A failure of the dossier layer: its MCP code when it has one, else `dossier_error` (Story 8.4
/// AC 11). `path` is the resolved dossier path.
pub fn failure(e: &Error, path: &Path) -> Rendered {
    match e {
        Error::McpSchemaMismatch {
            file_user_version,
            supported,
        } => {
            let tail = if *file_user_version > i64::from(*supported) {
                "ce serveur MCP est plus ancien que le dossier."
            } else {
                "ouvrez-le d'abord dans l'application."
            };
            Rendered::new(
                "schema_mismatch",
                format!(
                    "Le dossier est au schéma v{file_user_version}, ce serveur MCP en \
                     v{supported} ; {tail}"
                ),
            )
        }
        Error::McpDenied { .. } => Rendered::new(
            "write_denied",
            "Écriture refusée : seule la création de propositions est permise.",
        ),
        Error::McpInvalidCall { detail } => invalid_call(detail),
        Error::McpUnavailable { reason } => unavailable(reason, path),
        other => dossier_error(&other.to_string()),
    }
}

/// `dossier_error` (Story 8.4 AC 11): a failure with no named MCP cause.
pub fn dossier_error(cause: &str) -> Rendered {
    Rendered::new(
        "dossier_error",
        format!("Le dossier n'a pas pu être lu ou écrit ({cause}) ; rien n'a été enregistré."),
    )
}

/// A refused or failed submission.
pub fn submit_error(e: &SubmitError, path: &Path) -> Rendered {
    match e {
        SubmitError::Refused(r) => refusal(r),
        SubmitError::Failed(f) => failure(f, path),
        SubmitError::Cancelled => Rendered::new(
            "cancelled",
            "L'appel a été annulé par le client ; rien n'a été enregistré.",
        ),
    }
}

/// `dossier_path_unusable` (G3): the resolved dossier path is not valid UTF-8.
pub fn path_unusable() -> Rendered {
    Rendered::new(
        "dossier_path_unusable",
        "Le chemin du dossier contient des caractères non pris en charge ; rien n'a été lu.",
    )
}

/// Why no dossier could be determined for the call (Story 8.4 AC 5 / 11).
pub fn resolve_error(e: &ResolveError) -> Rendered {
    match e {
        // The config path and the parse detail go to the server's log only (G3).
        ResolveError::ConfigUnreadable(_) => Rendered::new(
            "config_unreadable",
            "La configuration de l'application n'a pas pu être lue ; aucun dossier n'a été \
             déterminé, rien n'a été lu.",
        ),
        ResolveError::NoLocation => Rendered::new(
            "no_dossier",
            "Aucun dossier n'a pu être déterminé ; rien n'a été lu.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use steadyinvest_core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR, contains_word};
    use steadyinvest_paths::PointerError;
    use steadyinvest_persistence::DossierIdentity;
    use uuid::Uuid;

    /// One instance of every outcome the server can render — the list AC 10 asks for. Adding a
    /// variant upstream breaks the exhaustive matches above; this list must grow with them.
    pub(crate) fn every_outcome() -> Vec<Rendered> {
        let path = PathBuf::from("/tmp/dossier.db");
        let id = Uuid::from_u128(7);
        let ident = |p: &str| DossierIdentity {
            journal_id: id,
            path: PathBuf::from(p),
        };
        let refusals = vec![
            SubmissionRefusal::DossierMismatch {
                read: ident("/a.db"),
                current: ident("/b.db"),
            },
            SubmissionRefusal::DossierReplaced,
            SubmissionRefusal::StudyNotFound { study_id: id },
            SubmissionRefusal::StudyArchived {
                ticker: "ABC".into(),
            },
            SubmissionRefusal::EmptyComment,
            SubmissionRefusal::MissingOrigin,
            SubmissionRefusal::TextTooLong {
                field: "comment",
                max: 10_000,
                len: 10_001,
            },
            SubmissionRefusal::IdentifierInvalid {
                ticker: "a b".into(),
                currency: "chf".into(),
            },
            SubmissionRefusal::FieldNotDraftable {
                field: "current_price".into(),
            },
            SubmissionRefusal::YearNotInStudy {
                year: 1999,
                ticker: "ABC".into(),
            },
            SubmissionRefusal::ValueUnparsable {
                text: "1e3".into(),
                field: DraftField::Sales,
            },
            SubmissionRefusal::ValueNotAnOption {
                text: "x".into(),
                field: DraftField::ForecastLowOption,
            },
            SubmissionRefusal::ValueOutOfRange {
                text: "1".into(),
                field: DraftField::Eps,
            },
            SubmissionRefusal::EmptyNoteText,
            SubmissionRefusal::TargetHasPending {
                field: DraftField::Eps,
                fiscal_year: Some(2024),
                pending_id: id,
            },
            SubmissionRefusal::StudyExists {
                ticker: "ABC".into(),
                currency: "USD".into(),
                study_id: id,
            },
            SubmissionRefusal::DraftStudyPending {
                ticker: "ABC".into(),
                currency: "USD".into(),
                draft_id: id,
            },
            SubmissionRefusal::DraftIdConflict { id },
        ];
        let reasons = vec![
            McpUnavailable::Missing,
            McpUnavailable::NotADossier,
            McpUnavailable::RestoreInProgress,
            McpUnavailable::RestoreInterrupted,
            McpUnavailable::Busy,
            McpUnavailable::NeedsRecovery,
            McpUnavailable::Protected { directory: false },
            McpUnavailable::Protected { directory: true },
            McpUnavailable::IdentityUnreadable { detail: "x".into() },
        ];
        let mut out: Vec<Rendered> = refusals.iter().map(refusal).collect();
        out.extend(reasons.iter().map(|r| unavailable(r, &path)));
        out.push(failure(
            &Error::McpSchemaMismatch {
                file_user_version: 7,
                supported: 9,
            },
            &path,
        ));
        out.push(failure(
            &Error::McpSchemaMismatch {
                file_user_version: 10,
                supported: 9,
            },
            &path,
        ));
        out.push(failure(&Error::McpDenied { denials: vec![] }, &path));
        out.push(failure(
            &Error::McpInvalidCall { detail: "x".into() },
            &path,
        ));
        out.push(failure(
            &Error::CorruptPayload { detail: "x".into() },
            &path,
        ));
        out.push(resolve_error(&ResolveError::ConfigUnreadable(
            PointerError::Invalid {
                path: PathBuf::from("/c.json"),
                detail: "x".into(),
            },
        )));
        out.push(resolve_error(&ResolveError::NoLocation));
        out.push(invalid_call("x"));
        out.push(submit_error(&SubmitError::Cancelled, &path));
        out.push(path_unusable());
        out.push(refusal(&SubmissionRefusal::TextTooLong {
            field: "proposed_value",
            max: 100,
            len: 101,
        }));
        out
    }

    #[test]
    fn every_code_of_the_spec_has_a_message() {
        let codes: std::collections::BTreeSet<&str> =
            every_outcome().iter().map(|r| r.code).collect();
        for code in [
            "dossier_mismatch",
            "dossier_replaced",
            "schema_mismatch",
            "no_dossier",
            "study_not_found",
            "empty_comment",
            "missing_origin",
            "field_not_draftable",
            "year_not_in_study",
            "value_unparsable",
            "value_not_an_option",
            "identifier_invalid",
            "target_has_pending",
            "study_exists",
            "draft_study_pending",
            "write_denied",
            "study_archived",
            "value_out_of_range",
            "empty_note_text",
            "text_too_long",
            "draft_id_conflict",
            "dossier_busy",
            "restore_interrupted",
            "dossier_locked",
            "dossier_needs_recovery",
            "dossier_protected",
            "not_a_dossier",
            "dossier_identity_unreadable",
            "invalid_call",
            "config_unreadable",
            "dossier_error",
            "cancelled",
            "dossier_path_unusable",
        ] {
            assert!(codes.contains(code), "no message for {code}");
        }
        for r in every_outcome() {
            assert!(!r.message.trim().is_empty(), "{} renders empty", r.code);
            assert!(
                !r.message.contains('{') && !r.message.contains('}'),
                "{} leaves a placeholder: {}",
                r.code,
                r.message
            );
        }
    }

    #[test]
    fn no_message_uses_a_banned_verb() {
        for r in every_outcome() {
            for verb in BANNED_VERBS_FR.iter().chain(BANNED_VERBS_EN.iter()) {
                assert!(
                    !contains_word(&r.message, verb),
                    "{}: « {verb} » in {}",
                    r.code,
                    r.message
                );
            }
        }
        for field in DraftField::ALL {
            for verb in BANNED_VERBS_FR.iter().chain(BANNED_VERBS_EN.iter()) {
                assert!(!contains_word(&unit_fr(field), verb), "{verb} in a unit");
            }
        }
    }

    #[test]
    fn placeholders_are_filled_from_the_typed_data() {
        let r = refusal(&SubmissionRefusal::YearNotInStudy {
            year: 1999,
            ticker: "NVDA".into(),
        });
        assert_eq!(
            r.message,
            "L'année 1999 n'existe pas dans l'étude NVDA ; une proposition n'ajoute jamais d'année."
        );
        let r = refusal(&SubmissionRefusal::ValueNotAnOption {
            text: "x".into(),
            field: DraftField::ForecastLowOption,
        });
        assert!(
            r.message
                .contains("avg_low_pe_times_eps, avg_low_price_last5y"),
            "{}",
            r.message
        );
        let r = unavailable(&McpUnavailable::NotADossier, Path::new("/x/y.db"));
        assert!(r.message.contains("/x/y.db"), "{}", r.message);
    }

    #[test]
    fn echoes_are_cut_and_carry_no_control_character() {
        let long = format!("1\nFORGED {}", "9".repeat(200));
        let r = refusal(&SubmissionRefusal::ValueUnparsable {
            text: long,
            field: DraftField::Eps,
        });
        assert!(!r.message.contains('\n'), "{}", r.message);
        assert!(r.message.contains("…"), "{}", r.message);
        assert!(r.message.chars().count() < 250, "{}", r.message);
        assert_eq!(echo("a\tb"), "a b");
    }

    #[test]
    fn a_mismatch_on_the_same_path_names_the_journal_ids_and_config_paths_stay_out() {
        let path = PathBuf::from("/d/dossier.db");
        let r = refusal(&SubmissionRefusal::DossierMismatch {
            read: DossierIdentity {
                journal_id: Uuid::from_u128(1),
                path: path.clone(),
            },
            current: DossierIdentity {
                journal_id: Uuid::from_u128(2),
                path,
            },
        });
        assert_eq!(
            r.message,
            format!(
                "Le dossier a changé depuis la lecture ({} ≠ {}) ; rien n'a été enregistré.",
                Uuid::from_u128(1),
                Uuid::from_u128(2)
            )
        );
        let r = resolve_error(&ResolveError::ConfigUnreadable(PointerError::Invalid {
            path: PathBuf::from("/secret/home/config.json"),
            detail: "x".into(),
        }));
        assert!(!r.message.contains("/secret"), "{}", r.message);
    }
}
