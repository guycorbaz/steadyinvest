//! Study-state slice (Story 2.2): open/create the journal, list studies, create a new study, and
//! reopen one with full state. This is the *open/load/save* slice only — the architecture tree's
//! full `state.rs` (immutable StudyState snapshot, undo stack, content-addressed verdict) is Story
//! 2.9 (undo) / 2.6 (verdict); a documented partial is fine here.
//!
//! **No calculation lives here** (Cardinal Rule) and **no network** — only `steadyinvest-persistence`
//! and `steadyinvest-contract` are touched. Time and identity come *only* through the injected
//! [`Clock`] / [`IdGen`] (ADD15): this module never calls `Uuid::new_v4` or a wall clock itself.
//!
//! Failure modes degrade, never crash: a newer-schema file opens read-only (writes are refused with
//! a neutral notice), a corrupt/foreign configured file is set aside in favour of the default
//! journal (also with a notice), and the app stays usable throughout.

use std::path::{Path, PathBuf};

use rust_decimal::Decimal;
use steadyinvest_contract::{
    Cell, Coverage, ForecastLowOption, Freshness, ImportError, Judgment, Money, PendingProvider,
    Provenance, Review, Source, Study, Timestamp, YearData,
};
use steadyinvest_ingestion::{CanonicalYear, FetchedFinancials};
use steadyinvest_persistence::{
    clear_lock, inspect_backup, lock_is_stale, restore_journal_file, Error as PersistError,
    HoldingItem, ImportSummary, Journal, JournalMode, PortfolioItem, StudySummary, WatchItem,
};
use uuid::Uuid;

use steadyinvest_core::verdict::StudySnapshot;

use crate::clock::{Clock, IdGen};
use crate::viewmodel::refresh::RefreshCause;
use crate::viewmodel::{engine, entry};

// ── User-facing neutral notices (FR13). French (the UI source language); the crate-local posture
//    gate scans `USER_FACING_MESSAGES` for banned verbs, exactly like the `@tr()` literals. The
//    dynamic cause spliced into some of them comes from `persistence::Error`, already posture-gated
//    in that crate. ──

/// The ticker field was blank.
pub const MSG_BLANK_TICKER: &str = "Le symbole est vide ; aucune étude n'a été enregistrée.";
/// The native-currency field was blank.
pub const MSG_BLANK_CURRENCY: &str = "La devise est vide ; aucune étude n'a été enregistrée.";
/// A write was attempted on a read-only (newer-schema) journal.
pub const MSG_READ_ONLY_WRITE: &str =
    "Journal en lecture seule (schéma plus récent) ; l'écriture n'a pas eu lieu.";
/// No journal is open at all (even the default could not be prepared).
pub const MSG_NO_JOURNAL: &str = "Aucun journal n'est ouvert ; l'écriture n'a pas eu lieu.";
/// Startup: the open journal is read-only because its file was written by a newer schema.
pub const MSG_STARTUP_READ_ONLY: &str =
    "Le journal a été écrit par un schéma plus récent ; il est ouvert en lecture seule.";
/// Startup: the configured journal file was unreadable, so the default journal is in use instead.
pub const MSG_CONFIGURED_UNREADABLE: &str =
    "Le journal configuré est illisible ; le journal par défaut est utilisé.";
/// Startup: no journal directory is available from the OS (the default path cannot be computed).
pub const MSG_NO_DATA_DIR: &str =
    "Aucun emplacement de journal n'est disponible ; les études ne sont pas enregistrées.";
/// A save failed for a reason other than the read-only / identity guards (cause appended).
pub const MSG_SAVE_FAILED: &str = "L'enregistrement a échoué.";
/// The system clipboard could not be read for a paste-a-column (Story 2.4).
pub const MSG_CLIPBOARD_UNAVAILABLE: &str =
    "Le presse-papiers est indisponible ; aucune colonne n'a été collée.";
/// A pasted column had more lines than the grid has years; the surplus lines were dropped.
pub const MSG_PASTE_CLIPPED: &str =
    "Certaines lignes dépassaient la grille et n'ont pas été collées.";
/// A direct value edit was attempted on a validated (soft-locked) cell (Story 2.5). The sign-off
/// must be cleared first — the cell is never silently blanked by a stray keystroke.
pub const MSG_SOFT_LOCKED: &str = "Cellule validée ; retirez la validation avant de la modifier.";
/// The "unlock all" confirmation copy (Story 2.5) — fact-stating, posture-gated. `{n}` is replaced
/// with the count of validated cells the chosen scope would flip back to to-review.
pub const MSG_UNLOCK_CONFIRM: &str = "Cette action retire la validation de {n} cellule(s).";
/// The neutral notice after an "unlock all" completes — `{n}` is the count actually flipped.
pub const MSG_UNLOCK_DONE: &str = "Validation retirée de {n} cellule(s).";
/// The engine could not normalize the study's data (a structural input error — duplicate year /
/// invalid split): the verdict is suspended, never computed from broken inputs (Story 2.6).
pub const MSG_NORMALIZE_FAILED: &str =
    "Les données ne peuvent pas être préparées ; le calcul est suspendu.";
/// Dashboard archive/delete confirmation + completion copy (Story 2.12) — fact-stating, posture-gated.
/// `{t}` is replaced with the study's ticker (user data, not scanned).
pub const MSG_ARCHIVE_CONFIRM: &str = "Archiver l'étude {t} ? Elle sera masquée de la vue active.";
pub const MSG_UNARCHIVE_CONFIRM: &str = "Réactiver l'étude {t} ?";
pub const MSG_DELETE_CONFIRM: &str =
    "Supprimer l'étude {t} et sa série temporelle ? Cette suppression est définitive.";
pub const MSG_ARCHIVE_DONE: &str = "Étude {t} archivée.";
pub const MSG_UNARCHIVE_DONE: &str = "Étude {t} réactivée.";
pub const MSG_DELETE_DONE: &str = "Étude {t} supprimée.";
/// Verify-engine summary copy (Story 2.13, FR9) — fact-stating, posture-gated. `{n}`/`{t}` are the
/// passed/total counts.
pub const MSG_VERIFY_PASSED: &str = "{n}/{t} études de référence réussies.";
pub const MSG_VERIFY_DEVIATIONS: &str = "{n} écart(s) sur {t} études de référence.";
/// The neutral notice when the demonstration study cannot be loaded (a packaging error, not a panic).
pub const MSG_DEMO_UNAVAILABLE: &str = "L'étude de démonstration est indisponible.";
/// Provider auto-fetch copy (Story 3.1) — fact-stating, posture-gated. `{cause}` is the provider's
/// own neutral message (already banned-verb-gated in `ingestion::error`).
pub const MSG_PROVIDER_NO_KEY: &str =
    "Aucune clé fournisseur n'est configurée ; la récupération n'a pas eu lieu.";
pub const MSG_PROVIDER_FETCHING: &str = "Récupération des données du fournisseur en cours.";
pub const MSG_PROVIDER_FAILED: &str = "La récupération n'a pas abouti : {cause}";

/// Graceful-failure cause-named copy (Story 3.5, FR23/FR24) — fact-stating, posture-gated. Each
/// names the cause; last-known values stay in place and affected provider data is flagged stale. The
/// API key never appears (NFR-S1 — these are static, no raw-error interpolation). Selected by
/// [`provider_failure_notice`].
pub const MSG_PROVIDER_OFFLINE: &str =
    "La connexion au fournisseur a échoué ; les dernières données connues restent affichées (à actualiser).";
pub const MSG_PROVIDER_QUOTA: &str =
    "Le fournisseur a signalé une limite d'usage ; les dernières données connues restent affichées, réessayez plus tard.";
pub const MSG_PROVIDER_NO_DATA: &str =
    "Le fournisseur n'a renvoyé aucune donnée pour ce symbole ; les dernières données connues restent affichées.";

/// Manual-refresh recompute-cause copy (Story 3.3, FR29) — fact-stating, posture-gated. The cause is
/// a classification of what the refresh changed; the message names it (price / fundamentals / both),
/// or states that nothing moved. Selected by [`refresh_notice`].
pub const MSG_REFRESH_NOCHANGE: &str = "Aucun changement ; les données sont déjà à jour.";
pub const MSG_REFRESH_PRICE: &str = "Recalculé : prix actualisés.";
pub const MSG_REFRESH_INPUT: &str = "Recalculé : données fondamentales actualisées.";
pub const MSG_REFRESH_BOTH: &str = "Recalculé : prix et données fondamentales actualisés.";
/// Annual-update change-visibility clause (Story 3.6, FR3/Journey-2b) — appended to the refresh
/// notice when a re-fetch reset `✓` cells to `?`, naming the re-validation scope. `{n}` is the count.
pub const MSG_REFRESH_REVALIDATE: &str = "{n} cellule(s) à revérifier.";

/// Watchlist copy (Story 4.1, FR34) — fact-stating, posture-gated. Raised when a link is requested
/// but no saved study matches the watched ticker.
pub const MSG_WATCH_NO_STUDY: &str =
    "Aucune étude enregistrée pour ce symbole ; créez-la d'abord depuis Études.";

/// Holdings register copy (Story 4.3, FR36) — fact-stating, posture-gated. Raised when a holding's
/// quantity or price is not a valid number, or its symbol is empty; nothing is written.
pub const MSG_HOLDING_INVALID_NUMBER: &str =
    "La quantité et le prix d'achat doivent être des nombres ; aucune position n'a été enregistrée.";
pub const MSG_HOLDING_INVALID_TICKER: &str =
    "Le symbole est vide ; aucune position n'a été enregistrée.";

/// Trailing-stop copy (Story 4.5, FR42) — fact-stating, posture-gated. Raised when a trailing-stop
/// percentage is not a number strictly between 0 and 100; nothing is written.
pub const MSG_HOLDING_INVALID_STOP: &str =
    "Le seuil suiveur doit être un pourcentage entre 0 et 100 ; rien n'a été enregistré.";

/// Holdings price-refresh copy (Story 4.4, FR40) — fact-stating, posture-gated. Set when a manual
/// "refresh prices" begins, or when no holding is linked to a saved study (nothing to refresh).
pub const MSG_HOLDINGS_REFRESHING: &str = "Rafraîchissement des prix en cours.";
pub const MSG_HOLDINGS_REFRESH_NONE: &str =
    "Aucune position liée à une étude ; il n'y a aucun prix à rafraîchir.";

/// Recorded-sell copy (Story 4.7, FR46/FR47) — fact-stating, posture-gated. Set when the user
/// records a sell from a neutral trigger: the sell is journalled and the holding leaves the register.
pub const MSG_HOLDING_SOLD: &str =
    "La vente a été enregistrée ; la position a été retirée du portefeuille.";

/// Study export/import copy (Story 5.2, FR59) — fact-stating, posture-gated. The export envelope is
/// the portable data contract + schema_version + integrity hash; import verifies both before saving.
pub const MSG_STUDY_EXPORTED: &str = "L'étude a été exportée.";
pub const MSG_STUDY_IMPORTED: &str = "L'étude a été importée.";
pub const MSG_STUDY_UPDATED: &str =
    "L'étude existait déjà ; elle a été mise à jour depuis le fichier.";
pub const MSG_EXPORT_MISSING: &str = "L'étude est introuvable ; rien n'a été exporté.";
pub const MSG_IMPORT_INTEGRITY: &str =
    "Le fichier ne correspond pas à son empreinte d'intégrité (fichier corrompu ou incomplet) ; rien n'a été importé.";
pub const MSG_IMPORT_VERSION: &str =
    "Le fichier provient d'une version incompatible du format ; rien n'a été importé.";
pub const MSG_IMPORT_MALFORMED: &str =
    "Le fichier n'est pas un export d'étude valide ; rien n'a été importé.";

/// Whole-journal export/import copy (Story 5.3, FR60) — fact-stating, posture-gated. The export is the
/// portable data contract for the entire journal + schema_version + (journal_id, version) + integrity
/// hash; import verifies both before applying, atomically (never partially). The integrity/version/
/// malformed rejections reuse the single-study [`MSG_IMPORT_INTEGRITY`]/[`MSG_IMPORT_VERSION`]/
/// [`MSG_IMPORT_MALFORMED`] notices (same taxonomy).
pub const MSG_JOURNAL_EXPORTED: &str = "Le journal a été exporté.";
/// Substitution template (the const is posture-scanned; [`journal_imported_message`] fills it). The
/// trailing `(source : journal {jid}, version {ver})` clause surfaces the imported file's identity so
/// the user sees whether it is the **same** journal (an update) or a **foreign** seed (AC3).
pub const MSG_JOURNAL_IMPORTED: &str =
    "Le journal a été importé : {studies} étude(s), {watch} valeur(s) suivie(s), {holdings} ligne(s) de portefeuille, {txns} mouvement(s). (source : journal {jid}, version {ver})";

/// Backup / restore copy (Story 5.4, FR61) — fact-stating, posture-gated. The backup/restore unit is
/// the raw `.db`; a restore validates integrity + schema-version + identity BEFORE any overwrite and
/// is never applied silently (a stale/foreign restore is gated behind a confirm).
pub const MSG_BACKUP_CREATED: &str = "La sauvegarde du journal a été créée.";
pub const MSG_RESTORE_DONE: &str = "Le journal a été restauré depuis la sauvegarde.";
pub const MSG_RESTORE_FAILED: &str = "La restauration a échoué ; le journal n'a pas été remplacé.";
pub const MSG_RESTORE_INTEGRITY: &str =
    "Le fichier de sauvegarde est corrompu (échec du contrôle d'intégrité) ; rien n'a été restauré.";
pub const MSG_RESTORE_NEWER_SCHEMA: &str =
    "La sauvegarde provient d'une version plus récente de l'application ; cette version ne sait pas la lire ; rien n'a été restauré.";
pub const MSG_RESTORE_NOT_A_JOURNAL: &str =
    "Le fichier n'est pas un journal valide ; rien n'a été restauré.";
pub const MSG_RESTORE_UNREADABLE: &str =
    "Le fichier de sauvegarde est illisible ; rien n'a été restauré.";
/// Substitution templates (the consts are posture-scanned; [`restore_confirm_message`] fills them).
pub const MSG_RESTORE_CONFIRM: &str =
    "Restaurer depuis cette sauvegarde (journal {jid}, version {ver}) ? {reason}Le journal actuel sera remplacé.";
pub const MSG_RESTORE_REASON_STALE: &str =
    "Cette sauvegarde (version {b}) est plus ancienne que le journal actuel (version {c}). ";
pub const MSG_RESTORE_REASON_FOREIGN: &str = "Cette sauvegarde appartient à un autre journal. ";

/// Journal-location copy (Story 5.5, FR66) — fact-stating, posture-gated. The location picker, recent
/// journals, single-instance lock and sync-folder safety.
pub const MSG_JOURNAL_OPENED: &str = "Le journal a été ouvert.";
pub const MSG_JOURNAL_CREATED: &str = "Le nouveau journal a été créé et ouvert.";
pub const MSG_JOURNAL_OPEN_FAILED: &str = "Le journal n'a pas pu être ouvert.";
pub const MSG_JOURNAL_LOCKED: &str =
    "Ce journal est déjà ouvert dans une autre fenêtre ou un autre processus ; il n'a pas été ouvert.";
pub const MSG_JOURNAL_LOCK_RECLAIMABLE: &str =
    "Ce journal porte un verrou laissé par une session interrompue ; le verrou peut être levé.";
pub const MSG_SYNC_FOLDER_WARNING: &str =
    "Ce dossier est synchronisé : le journal est ouvert en mode sûr (sans fichier annexe). Un journal en local avec des sauvegardes versionnées dans ce dossier reste l'approche recommandée.";
/// Substitution template (the const is posture-scanned; [`journal_stale_message`] fills it).
pub const MSG_JOURNAL_STALE: &str =
    "Ce journal semble plus ancien que ce que vous aviez vu (vu version {seen}, ici version {here}).";

/// The neutral stale-on-reopen notice (Story 5.5) — a `{n}`-substitution of [`MSG_JOURNAL_STALE`]
/// surfacing the last-seen vs on-disk versions, so a regressed journal is flagged (not blocked).
pub fn journal_stale_message(seen: u64, here: u64) -> String {
    MSG_JOURNAL_STALE
        .replace("{seen}", &seen.to_string())
        .replace("{here}", &here.to_string())
}

/// The neutral confirm prompt for a restore (a `{n}`-substitution of [`MSG_RESTORE_CONFIRM`] +
/// per-verdict reason clause), surfacing the backup's `(journal_id, version)` and the stale/foreign
/// warning so a restore is never applied silently (FR61).
pub fn restore_confirm_message(assessment: &RestoreAssessment) -> String {
    let reason = match assessment.verdict {
        RestoreVerdict::StaleOlder { backup, current } => MSG_RESTORE_REASON_STALE
            .replace("{b}", &backup.to_string())
            .replace("{c}", &current.to_string()),
        RestoreVerdict::ForeignJournal => MSG_RESTORE_REASON_FOREIGN.to_string(),
        _ => String::new(),
    };
    MSG_RESTORE_CONFIRM
        .replace("{jid}", &assessment.journal_id.to_string())
        .replace("{ver}", &assessment.logical_version.to_string())
        .replace("{reason}", &reason)
}

/// The neutral outcome of a whole-journal import, with per-entity counts **and the source journal
/// identity** (a `{n}`-substitution of [`MSG_JOURNAL_IMPORTED`] so the scanned const and the runtime
/// string stay one source — the `unlock_confirm_message` pattern).
pub fn journal_imported_message(summary: &ImportSummary) -> String {
    MSG_JOURNAL_IMPORTED
        .replace("{studies}", &summary.studies.to_string())
        .replace("{watch}", &summary.watch_items.to_string())
        .replace("{holdings}", &summary.holdings.to_string())
        .replace("{txns}", &summary.transactions.to_string())
        .replace("{jid}", &summary.source_journal_id.to_string())
        .replace("{ver}", &summary.source_logical_version.to_string())
}

/// Provider configuration & keychain copy (Story 3.2, FR25/FR63) — fact-stating, posture-gated. The
/// API key itself is NEVER part of any message (NFR-S1); these state the outcome only.
pub const MSG_KEY_SAVED: &str =
    "La clé du fournisseur est enregistrée dans le trousseau du système.";
pub const MSG_KEY_DELETED: &str = "La clé du fournisseur est retirée du trousseau du système.";
pub const MSG_KEY_TESTING: &str = "Test de la clé du fournisseur en cours.";
pub const MSG_KEY_OK: &str = "La clé est valide ; le fournisseur a répondu.";
pub const MSG_KEY_INVALID: &str = "La clé est invalide ou absente ; le fournisseur l'a refusée.";
pub const MSG_KEY_FORBIDDEN: &str =
    "La clé est valide, mais l'abonnement ne couvre pas ces données ; le fournisseur a refusé l'accès.";
pub const MSG_KEYCHAIN_UNAVAILABLE: &str =
    "Le trousseau du système est indisponible ; la clé n'a pas été enregistrée.";

/// The confirmation prompt for an "unlock all" of `count` cells (a `{n}`-substitution of
/// [`MSG_UNLOCK_CONFIRM`] so the scanned const and the runtime string stay one source).
pub fn unlock_confirm_message(count: usize) -> String {
    MSG_UNLOCK_CONFIRM.replace("{n}", &count.to_string())
}

/// The completion notice for an "unlock all" that flipped `count` cells.
pub fn unlock_done_message(count: usize) -> String {
    MSG_UNLOCK_DONE.replace("{n}", &count.to_string())
}

/// The confirm prompt for a dashboard lifecycle action on the study `ticker` (Story 2.12) — a `{t}`
/// substitution of the matching `MSG_*_CONFIRM` const so the scanned template and the runtime string
/// stay one source. `action` is the Slint wire string (`"archive"`/`"unarchive"`/`"delete"`); an
/// unknown action falls back to the (most cautious) delete prompt.
pub fn study_action_confirm_message(action: &str, ticker: &str) -> String {
    let template = match action {
        "archive" => MSG_ARCHIVE_CONFIRM,
        "unarchive" => MSG_UNARCHIVE_CONFIRM,
        _ => MSG_DELETE_CONFIRM,
    };
    template.replace("{t}", ticker)
}

/// The verify-engine summary line (Story 2.13): all-passed → "{n}/{t} réussies", else "{n} écart(s)".
pub fn verify_summary(passed: usize, total: usize) -> String {
    if passed == total {
        MSG_VERIFY_PASSED
            .replace("{n}", &passed.to_string())
            .replace("{t}", &total.to_string())
    } else {
        MSG_VERIFY_DEVIATIONS
            .replace("{n}", &total.saturating_sub(passed).to_string())
            .replace("{t}", &total.to_string())
    }
}

/// The neutral notice after a manual refresh (Story 3.3, FR29): name the recompute cause from the
/// [`RefreshReport`] — price, fundamentals, both, or "no change" when nothing moved. The single
/// source mapping a refresh outcome to its posture-gated message.
pub fn refresh_notice(report: RefreshReport) -> &'static str {
    if !report.changed() {
        return MSG_REFRESH_NOCHANGE;
    }
    match (report.cause.price, report.cause.input) {
        (true, true) => MSG_REFRESH_BOTH,
        (true, false) => MSG_REFRESH_PRICE,
        (false, true) => MSG_REFRESH_INPUT,
        // Changed cells with no price/input cause (e.g. FX once FR28 lands) — fall back to the
        // fundamentals wording rather than claim "no change". Unreachable in this story.
        (false, false) => MSG_REFRESH_INPUT,
    }
}

/// The full post-refresh notice (Story 3.6): the cause line ([`refresh_notice`]) plus, when this
/// refresh reset `✓` cells to `?`, the re-validation-scope clause ("N cellule(s) à revérifier") — so
/// an annual update tells the user *what to re-check*, not just *what moved*. With `revalidate == 0`
/// it is exactly [`refresh_notice`] (no regression on the common path).
pub fn refresh_summary(report: RefreshReport) -> String {
    let cause = refresh_notice(report);
    if report.revalidate == 0 {
        return cause.to_string();
    }
    let revalidate = MSG_REFRESH_REVALIDATE.replace("{n}", &report.revalidate.to_string());
    format!("{cause} · {revalidate}")
}

/// Classify a provider/ingestion failure into its neutral, cause-named notice (Story 3.5, FR24).
/// The single mapping from the `ingestion` taxonomy to the global banner — never the raw error
/// string (NFR-S1: the api_token lives in the request URL; static notices cannot leak it).
pub fn provider_failure_notice(error: &steadyinvest_ingestion::IngestionError) -> &'static str {
    use steadyinvest_ingestion::{IngestionError, ProviderError};
    match error {
        IngestionError::Provider(p) => match p {
            ProviderError::Network { .. } => MSG_PROVIDER_OFFLINE,
            ProviderError::Quota { .. } => MSG_PROVIDER_QUOTA,
            ProviderError::InvalidOrAbsentKey => MSG_KEY_INVALID,
            ProviderError::Forbidden { .. } => MSG_KEY_FORBIDDEN,
            ProviderError::TickerNotFound { .. } => MSG_PROVIDER_NO_DATA,
            // A malformed / unsupported / unparseable payload is not an outage but the data cannot be
            // prepared — the neutral "data can't be prepared" notice (a static string, no token, and
            // never `MSG_PROVIDER_FAILED`'s `{cause}` placeholder which only the worker-gone path fills).
            ProviderError::Parse { .. } | ProviderError::Unsupported { .. } => MSG_NORMALIZE_FAILED,
        },
        // The fetched data reached us but did not normalize (a structural payload error).
        IngestionError::Normalize(_) => MSG_NORMALIZE_FAILED,
    }
}

/// The completion notice after a dashboard lifecycle action on `ticker` completes (Story 2.12).
pub fn study_action_done_message(action: &str, ticker: &str) -> String {
    let template = match action {
        "archive" => MSG_ARCHIVE_DONE,
        "unarchive" => MSG_UNARCHIVE_DONE,
        _ => MSG_DELETE_DONE,
    };
    template.replace("{t}", ticker)
}

/// Every static user-facing message above — exposed so the crate-local posture gate (FR13) scans
/// them for banned verbs alongside the `@tr()` literals. Test-only (the gate's sole consumer);
/// the individual `MSG_*` consts are the runtime surfaces. Keep in sync with the consts.
#[cfg(test)]
pub const USER_FACING_MESSAGES: &[&str] = &[
    MSG_BLANK_TICKER,
    MSG_BLANK_CURRENCY,
    MSG_READ_ONLY_WRITE,
    MSG_NO_JOURNAL,
    MSG_STARTUP_READ_ONLY,
    MSG_CONFIGURED_UNREADABLE,
    MSG_NO_DATA_DIR,
    MSG_SAVE_FAILED,
    MSG_CLIPBOARD_UNAVAILABLE,
    MSG_PASTE_CLIPPED,
    MSG_SOFT_LOCKED,
    MSG_UNLOCK_CONFIRM,
    MSG_UNLOCK_DONE,
    MSG_NORMALIZE_FAILED,
    MSG_ARCHIVE_CONFIRM,
    MSG_UNARCHIVE_CONFIRM,
    MSG_DELETE_CONFIRM,
    MSG_ARCHIVE_DONE,
    MSG_UNARCHIVE_DONE,
    MSG_DELETE_DONE,
    MSG_VERIFY_PASSED,
    MSG_VERIFY_DEVIATIONS,
    MSG_DEMO_UNAVAILABLE,
    MSG_PROVIDER_NO_KEY,
    MSG_PROVIDER_FETCHING,
    MSG_PROVIDER_FAILED,
    MSG_PROVIDER_OFFLINE,
    MSG_PROVIDER_QUOTA,
    MSG_PROVIDER_NO_DATA,
    MSG_REFRESH_NOCHANGE,
    MSG_REFRESH_PRICE,
    MSG_REFRESH_INPUT,
    MSG_REFRESH_BOTH,
    MSG_REFRESH_REVALIDATE,
    MSG_WATCH_NO_STUDY,
    MSG_HOLDING_INVALID_NUMBER,
    MSG_HOLDING_INVALID_TICKER,
    MSG_HOLDING_INVALID_STOP,
    MSG_HOLDINGS_REFRESHING,
    MSG_HOLDINGS_REFRESH_NONE,
    MSG_HOLDING_SOLD,
    MSG_STUDY_EXPORTED,
    MSG_STUDY_IMPORTED,
    MSG_STUDY_UPDATED,
    MSG_EXPORT_MISSING,
    MSG_IMPORT_INTEGRITY,
    MSG_IMPORT_VERSION,
    MSG_IMPORT_MALFORMED,
    MSG_JOURNAL_EXPORTED,
    MSG_JOURNAL_IMPORTED,
    MSG_BACKUP_CREATED,
    MSG_RESTORE_DONE,
    MSG_RESTORE_FAILED,
    MSG_RESTORE_INTEGRITY,
    MSG_RESTORE_NEWER_SCHEMA,
    MSG_RESTORE_NOT_A_JOURNAL,
    MSG_RESTORE_UNREADABLE,
    MSG_RESTORE_CONFIRM,
    MSG_RESTORE_REASON_STALE,
    MSG_RESTORE_REASON_FOREIGN,
    MSG_JOURNAL_OPENED,
    MSG_JOURNAL_CREATED,
    MSG_JOURNAL_OPEN_FAILED,
    MSG_JOURNAL_LOCKED,
    MSG_JOURNAL_LOCK_RECLAIMABLE,
    MSG_SYNC_FOLDER_WARNING,
    MSG_JOURNAL_STALE,
    MSG_KEY_SAVED,
    MSG_KEY_DELETED,
    MSG_KEY_TESTING,
    MSG_KEY_OK,
    MSG_KEY_INVALID,
    MSG_KEY_FORBIDDEN,
    MSG_KEYCHAIN_UNAVAILABLE,
];

/// The scope of a bulk "unlock all" (Story 2.5): the whole study, a single year column, or a single
/// metric (one §3 column / §2 row) across all years. Each flips every `✓` it covers back to `?`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnlockScope {
    /// Every validated cell in the study.
    Study,
    /// Every validated cell in one materialized year (by index into the year window).
    Year(usize),
    /// Every validated cell of one field (a §3 column letter / §2 row key) across all years.
    Metric(String),
}

impl UnlockScope {
    /// Whether this scope covers the `(year_index, field)` cell address.
    fn covers(&self, year_index: usize, field: &str) -> bool {
        match self {
            UnlockScope::Study => true,
            UnlockScope::Year(y) => *y == year_index,
            UnlockScope::Metric(f) => f == field,
        }
    }
}

/// Where a default journal lives when the user has none yet: the OS **data** dir (NOT the config
/// dir, NOT beside `config.json`, NOT inside the journal) — outside any sync-watched tree (the
/// Synology-Drive SQLite-corruption risk, project memory). The location picker / sync-safety switch
/// is Story 5-5; this is the safe default only.
pub fn default_journal_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "steadyinvest")
        .map(|dirs| dirs.data_dir().join("journal.db"))
}

/// The all-`None` judgment a freshly-created study starts with (every optional `None`, plus the
/// default forecast-low option). 2.2 creates a study with no judgment inputs yet — those are 2.6.
fn empty_judgment() -> Judgment {
    Judgment {
        estimated_high_eps: None,
        estimated_low_eps: None,
        projected_sales_growth_pct: None,
        projected_eps_growth_pct: None,
        judged_avg_high_pe: None,
        judged_avg_low_pe: None,
        forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
        recent_severe_low: None,
        current_price: None,
        present_full_year_dividend: None,
    }
}

/// The maximum number of undo steps kept in memory (oldest dropped past this). `Study` clones are
/// small but not free; a long session does not grow the history unboundedly (Story 2.9).
const UNDO_CAP: usize = 100;

/// Which way [`JournalState::step`] moves through the history.
#[derive(Clone, Copy)]
enum Direction {
    Undo,
    Redo,
}

/// In-memory undo/redo history for the open study (Story 2.9) — a stack of whole [`Study`] snapshots,
/// **NOT a diff log**. This realizes the architecture's "snapshot stack, simple clones because state
/// is small" directly over the persisted `Study` blob: the app keeps no separate in-memory domain
/// state (the journal is the source of truth), so a snapshot IS a `Study` clone. Per open study,
/// reset on open, never persisted across reopen.
#[derive(Default)]
pub struct UndoHistory {
    /// States as they were BEFORE each mutation (most recent on top).
    undo: Vec<Study>,
    /// States displaced by an undo, available to redo (most recent on top).
    redo: Vec<Study>,
}

impl UndoHistory {
    /// Record the pre-mutation snapshot and invalidate the redo branch (a new edit forks history).
    fn record(&mut self, before: Study) {
        self.undo.push(before);
        if self.undo.len() > UNDO_CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn reset(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

/// The live journal session plus the injected time/identity sources. Owns the open [`Journal`] for
/// the lifetime of the window so the create/open callbacks can reach it.
pub struct JournalState {
    /// `None` only when not even the default journal could be opened/created — the app stays
    /// usable (read-only, study creation refused with [`MSG_NO_JOURNAL`]).
    journal: Option<Journal>,
    /// The resolved on-disk path (to persist into app-config), when a journal is open.
    path: Option<PathBuf>,
    /// True when the open journal is read-only (newer-schema file): writes are refused up front.
    read_only: bool,
    clock: Box<dyn Clock>,
    idgen: Box<dyn IdGen>,
    /// Undo/redo history for the currently-open study (Story 2.9). Reset on open.
    history: UndoHistory,
    /// A validated backup parked awaiting confirmation (Story 5.4): a restore is **never applied
    /// silently** (FR61) — `request_restore` parks the candidate, `confirm_restore` applies it.
    pending_restore: Option<PendingRestore>,
}

/// How a candidate backup compares to the current journal (Story 5.4, AC2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreVerdict {
    /// Same journal, backup version ≥ current — a safe forward restore.
    Ok,
    /// Same journal, backup is **older** than the current journal.
    StaleOlder { backup: u64, current: u64 },
    /// A backup belonging to a **different** journal.
    ForeignJournal,
    /// The backup was written by a schema **newer** than this build supports (hard refusal).
    NewerSchema { found: i64, supported: u32 },
    /// `PRAGMA integrity_check` failed (hard refusal).
    IntegrityFailed,
}

/// A backup assessed against the current journal (Story 5.4) — the backup's surfaced identity plus the
/// verdict that gates the confirm flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreAssessment {
    pub journal_id: Uuid,
    pub logical_version: u64,
    pub verdict: RestoreVerdict,
}

impl RestoreAssessment {
    /// A hard refusal (newer schema / failed integrity) offers **no** confirm — only the soft verdicts
    /// (Ok / StaleOlder / ForeignJournal) park a pending restore the user can confirm.
    fn is_confirmable(&self) -> bool {
        matches!(
            self.verdict,
            RestoreVerdict::Ok | RestoreVerdict::StaleOlder { .. } | RestoreVerdict::ForeignJournal
        )
    }
}

/// A validated backup parked awaiting an explicit confirm (Story 5.4). Only the path is needed to
/// apply — the assessment was already surfaced to the user by `request_restore`.
#[derive(Debug, Clone)]
struct PendingRestore {
    backup_path: PathBuf,
}

/// The result of opening/creating/switching a journal (Story 5.5) — the identity + version the caller
/// records in the recent-journals pointer, and whether a sync-folder warning applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOutcome {
    pub journal_id: Uuid,
    pub logical_version: u64,
    /// `true` when the journal lives in a detected sync folder and was opened in the sync-safe
    /// (`DELETE`) mode — the UI surfaces the warning + the recommended pattern (ADD8).
    pub sync_warning: bool,
}

/// Whether `path` (a journal file or its directory) lives in a **detected sync folder** (Story 5.5,
/// ADD8) — a path component matches a known consumer-sync provider, case-insensitively. A heuristic,
/// not an exhaustive list; a false negative just means the default WAL mode (no worse than today).
pub fn is_sync_folder(path: &Path) -> bool {
    const SYNC_MARKERS: &[&str] = &[
        "synologydrive",
        "synology drive",
        "cloudstation",
        "dropbox",
        "onedrive",
        "icloud",
        "mobile documents", // macOS iCloud Drive
        "google drive",
        "googledrive",
        "nextcloud",
        "owncloud",
    ];
    // Canonicalize first (resolve a symlink / mount like `~/sync → ~/Dropbox`, the form most likely in
    // practice) — scanning only the literal path would miss it. Falls back to the best-resolving
    // ancestor (the file itself may not exist yet on a create), then the literal path.
    let resolved = std::fs::canonicalize(path)
        .or_else(|_| {
            path.parent()
                .map_or_else(|| Err(()), |p| std::fs::canonicalize(p).map_err(|_| ()))
        })
        .unwrap_or_else(|_| path.to_path_buf());
    resolved.components().any(|c| {
        let name = c.as_os_str().to_string_lossy().to_lowercase();
        SYNC_MARKERS.iter().any(|m| name.contains(m))
    })
}

/// The [`JournalMode`] to open a journal at `path` with (Story 5.5): the sync-safe `Delete` in a
/// detected sync folder (no `-wal` to corrupt under file-level sync), else the default `Wal`.
fn sync_mode_for(path: &Path) -> JournalMode {
    if is_sync_folder(path) {
        JournalMode::Delete
    } else {
        JournalMode::Wal
    }
}

impl JournalState {
    /// Open the last-used journal (`configured`, from app-config) or, failing that, open/create the
    /// default journal in the OS data dir. Returns the state plus an optional neutral startup notice
    /// to surface in a banner. Never panics; a failure leaves a usable (journal-less) state.
    pub fn open_or_create(
        configured: Option<&Path>,
        clock: Box<dyn Clock>,
        idgen: Box<dyn IdGen>,
    ) -> (Self, Option<String>) {
        // 1) A configured journal that exists on disk → open it.
        if let Some(path) = configured {
            if path.exists() {
                // Story 5.5: a STALE lock (left by a crashed prior run — no live owner) on the
                // configured journal is auto-reclaimed at startup, so a post-crash relaunch reopens the
                // user's own journal rather than failing `LockHeld` and orphaning it onto the default. A
                // LIVE lock (a genuine second instance) is not stale → left intact → the open refuses.
                if lock_is_stale(path) {
                    let _ = clear_lock(path);
                }
                match Journal::open_with_mode(path, sync_mode_for(path)) {
                    Ok(journal) => {
                        let read_only = journal.is_read_only();
                        return (
                            Self {
                                journal: Some(journal),
                                path: Some(path.to_path_buf()),
                                read_only,
                                clock,
                                idgen,
                                history: UndoHistory::default(),
                                pending_restore: None,
                            },
                            read_only.then(|| MSG_STARTUP_READ_ONLY.to_string()),
                        );
                    }
                    Err(error) => {
                        // The configured pick is corrupt/foreign/damaged — never write our schema
                        // into it (open already refused without writing). Fall back to the default
                        // journal so the app stays usable, and surface the cause.
                        tracing::warn!("configured journal {} unreadable: {error}", path.display());
                        let (state, _) = Self::open_or_create_default(clock, idgen);
                        return (state, Some(MSG_CONFIGURED_UNREADABLE.to_string()));
                    }
                }
            }
        }
        // 2) No usable configured path → the default journal.
        Self::open_or_create_default(clock, idgen)
    }

    /// Open the default journal if its file already exists, else create it (parent dirs included),
    /// stamping identity + creation time from the injected sources.
    fn open_or_create_default(
        clock: Box<dyn Clock>,
        idgen: Box<dyn IdGen>,
    ) -> (Self, Option<String>) {
        let Some(path) = default_journal_path() else {
            return (
                Self {
                    journal: None,
                    path: None,
                    read_only: false,
                    clock,
                    idgen,
                    history: UndoHistory::default(),
                    pending_restore: None,
                },
                Some(MSG_NO_DATA_DIR.to_string()),
            );
        };

        let result = if path.exists() {
            Journal::open_with_mode(&path, sync_mode_for(&path))
        } else {
            if let Some(parent) = path.parent() {
                if let Err(error) = std::fs::create_dir_all(parent) {
                    tracing::warn!("data dir {} not created: {error}", parent.display());
                }
            }
            Journal::create(&path, idgen.new_id(), &clock.now())
        };

        match result {
            Ok(journal) => {
                let read_only = journal.is_read_only();
                (
                    Self {
                        journal: Some(journal),
                        path: Some(path),
                        read_only,
                        clock,
                        idgen,
                        history: UndoHistory::default(),
                        pending_restore: None,
                    },
                    read_only.then(|| MSG_STARTUP_READ_ONLY.to_string()),
                )
            }
            Err(error) => {
                tracing::warn!("default journal {} unavailable: {error}", path.display());
                (
                    Self {
                        journal: None,
                        path: None,
                        read_only: false,
                        clock,
                        idgen,
                        history: UndoHistory::default(),
                        pending_restore: None,
                    },
                    Some(format!("{MSG_SAVE_FAILED} {error}")),
                )
            }
        }
    }

    /// The resolved on-disk path of the open journal, for persisting into app-config.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// True when the open journal is read-only (newer-schema file).
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// The open journal's identity (UUID), or `None` when no journal is open. Used to name a
    /// whole-journal export file (Story 5.3).
    pub fn journal_id(&self) -> Option<Uuid> {
        self.journal.as_ref().map(|j| j.id())
    }

    /// The open journal's monotonic `logical_version`, or `0` when no journal is open / unreadable
    /// (Story 5.5) — for the recent-journals last-seen pointer.
    pub fn logical_version_or_zero(&self) -> u64 {
        self.journal
            .as_ref()
            .and_then(|j| j.logical_version().ok())
            .unwrap_or(0)
    }

    /// The app's "now" from the injected [`Clock`] (ADD15) — the single wall-clock source. Used by
    /// the holdings price-refresh (Story 4.4) to stamp the transient per-ticker `as_of` freshness,
    /// so tests pin it deterministically via the `FixedClock` double.
    pub fn now(&self) -> Timestamp {
        self.clock.now()
    }

    // ── Undo/redo (Story 2.9) ──

    /// Clear the undo/redo history — a different study was opened, so its edit history starts empty.
    pub fn reset_undo(&mut self) {
        self.history.reset();
    }

    /// Whether an undo / redo step is available (the UI disables its control when not).
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// The number of recorded undo steps — test-only, to prove an idempotent mutation records no
    /// phantom step (Story 3.3 AC1: a no-op refresh must not push undo state).
    #[cfg(test)]
    pub fn undo_depth(&self) -> usize {
        self.history.undo.len()
    }

    /// The journal's current logical version — test-only, to prove a true no-op writes nothing
    /// (Story 3.4: a resolve with no pending must not bump the version / append an FR51 revision).
    #[cfg(test)]
    pub fn logical_version(&self) -> u64 {
        self.journal
            .as_ref()
            .and_then(|j| j.logical_version().ok())
            .unwrap_or(0)
    }

    /// Step the open study **back** to the snapshot before the last mutation (FR32). Returns
    /// `Ok(true)` when a step was taken (the caller re-reads + re-renders), `Ok(false)` when the
    /// undo stack is empty. The restore is a real, guarded `put_study` of the whole prior `Study`.
    pub fn undo(&mut self, study_id: Uuid) -> Result<bool, String> {
        self.step(study_id, Direction::Undo)
    }

    /// Step the open study **forward** to a snapshot displaced by a prior undo (no-op if the redo
    /// stack is empty).
    pub fn redo(&mut self, study_id: Uuid) -> Result<bool, String> {
        self.step(study_id, Direction::Redo)
    }

    /// The shared undo/redo engine: pop the target snapshot, write it back, and move the present
    /// state onto the opposite stack so the step is itself reversible. On a write failure the popped
    /// snapshot is pushed back (the history is never silently lost) and a neutral notice surfaces.
    fn step(&mut self, study_id: Uuid, dir: Direction) -> Result<bool, String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let popped = match dir {
            Direction::Undo => self.history.undo.pop(),
            Direction::Redo => self.history.redo.pop(),
        };
        let Some(restored) = popped else {
            return Ok(false); // nothing to step to
        };
        let push_back = |history: &mut UndoHistory, study: Study| match dir {
            Direction::Undo => history.undo.push(study),
            Direction::Redo => history.redo.push(study),
        };
        let Some(current) = self.get_study(study_id) else {
            push_back(&mut self.history, restored);
            return Err(MSG_SAVE_FAILED.to_string());
        };
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study(&restored)
        };
        match result {
            Ok(()) => {
                // The present state becomes reversible on the opposite stack.
                match dir {
                    Direction::Undo => self.history.redo.push(current),
                    Direction::Redo => self.history.undo.push(current),
                }
                Ok(true)
            }
            Err(PersistError::NewerJournalSchema { .. }) => {
                push_back(&mut self.history, restored);
                Err(MSG_READ_ONLY_WRITE.to_string())
            }
            Err(error) => {
                push_back(&mut self.history, restored);
                Err(format!("{MSG_SAVE_FAILED} {error}"))
            }
        }
    }

    /// The deterministic `created_at, id`-ordered study summaries (empty on no-journal / read error).
    pub fn list_studies(&self) -> Vec<StudySummary> {
        let Some(journal) = self.journal.as_ref() else {
            return Vec::new();
        };
        match journal.list_studies() {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!("list_studies failed: {error}");
                Vec::new()
            }
        }
    }

    // ── Watchlist (Story 4.1, FR34) ──

    /// Every watched security, ordered by position. Empty when no journal is open.
    pub fn list_watch_items(&self) -> Vec<WatchItem> {
        let Some(journal) = self.journal.as_ref() else {
            return Vec::new();
        };
        journal.list_watch_items().unwrap_or_else(|error| {
            tracing::warn!("list_watch_items failed: {error}");
            Vec::new()
        })
    }

    /// The most-recent saved study whose ticker matches `ticker` **case-insensitively** (Story 4.1
    /// watchlist link auto-match), or `None`. `list_studies` is ascending by `created_at`, so the
    /// last match is the newest. Case-insensitive so a watched `"nesn"` still finds the `"NESN"`
    /// study (tickers are stored as entered, not normalized).
    pub fn study_id_for_ticker(&self, ticker: &str) -> Option<Uuid> {
        self.list_studies()
            .into_iter()
            .rev()
            .find(|s| s.security_ticker.eq_ignore_ascii_case(ticker))
            .map(|s| s.id)
    }

    /// Add a watched security (FR34) — appended at the end. Id/timestamp from the injected sources
    /// (ADD15). Guarded (read-only / no-journal / save-failure → a neutral notice).
    pub fn add_watch_item(&mut self, ticker: &str, study_id: Option<Uuid>) -> Result<(), String> {
        let ticker = ticker.trim();
        if ticker.is_empty() {
            return Err(MSG_BLANK_TICKER.to_string());
        }
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let id = self.idgen.new_id();
        let created_at = self.clock.now();
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .add_watch_item(id, ticker, study_id, &created_at)
            .map(|_| ())
            .map_err(watch_error)
    }

    /// Edit a watched security's ticker and/or study link (FR34). Blank ticker is refused.
    pub fn update_watch_item(
        &mut self,
        id: Uuid,
        ticker: &str,
        study_id: Option<Uuid>,
    ) -> Result<(), String> {
        let ticker = ticker.trim();
        if ticker.is_empty() {
            return Err(MSG_BLANK_TICKER.to_string());
        }
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .update_watch_item(id, ticker, study_id)
            .map_err(watch_error)
    }

    /// Remove a watched security (FR34); the remaining rows re-pack to contiguous positions.
    pub fn delete_watch_item(&mut self, id: Uuid) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal.delete_watch_item(id).map_err(watch_error)
    }

    /// Move a watched security one slot up (`up = true`) or down in the order (FR34): swap its
    /// position with its neighbour. A no-op at the list edge.
    pub fn move_watch_item(&mut self, id: Uuid, up: bool) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let items = self.list_watch_items();
        let Some(index) = items.iter().position(|w| w.id == id) else {
            return Ok(()); // gone — nothing to move
        };
        let neighbour = if up {
            index.checked_sub(1)
        } else {
            (index + 1 < items.len()).then_some(index + 1)
        };
        let Some(n) = neighbour else {
            return Ok(()); // already at the edge — a neutral no-op
        };
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .set_watch_positions(&[
                (items[index].id, items[n].position),
                (items[n].id, items[index].position),
            ])
            .map_err(watch_error)
    }

    // ── Holdings register (Story 4.3, FR36) ──

    /// The single portfolio's holdings, ordered by creation. Empty when no journal / no portfolio
    /// exists yet. A pure read — it never creates the portfolio (that happens on the first add).
    pub fn list_holdings(&self) -> Vec<HoldingItem> {
        let Some(journal) = self.journal.as_ref() else {
            return Vec::new();
        };
        let portfolio = match journal.first_portfolio() {
            Ok(Some(p)) => p,
            Ok(None) => return Vec::new(),
            Err(error) => {
                tracing::warn!("first_portfolio failed: {error}");
                return Vec::new();
            }
        };
        journal.list_holdings(portfolio.id).unwrap_or_else(|error| {
            tracing::warn!("list_holdings failed: {error}");
            Vec::new()
        })
    }

    /// The portfolio's **capital-at-risk** + **total invested** (Story 4.6, FR43) — a pure read over
    /// the holdings, summed in the single reference currency (no FX, Epic 4). Each holding maps to a
    /// `core::risk::PositionRisk` (avg_cost = `purchase_price`, stop = `trailing_stop_level`,
    /// qty = `quantity`); a holding whose persisted TEXT decimals don't parse is skipped (defensive —
    /// they always parse on write). Returns `(capital_at_risk, total_invested)`, both `≥ 0`.
    pub fn portfolio_capital_at_risk(&self) -> (Decimal, Decimal) {
        let positions: Vec<steadyinvest_core::risk::PositionRisk> = self
            .list_holdings()
            .into_iter()
            .filter_map(|h| {
                let avg_cost = Decimal::from_str_exact(&h.purchase_price).ok()?;
                let quantity = Decimal::from_str_exact(&h.quantity).ok()?;
                let stop = h
                    .trailing_stop_level
                    .as_deref()
                    .and_then(|s| Decimal::from_str_exact(s).ok());
                Some(steadyinvest_core::risk::PositionRisk {
                    avg_cost,
                    stop,
                    quantity,
                })
            })
            .collect();
        (
            steadyinvest_core::risk::capital_at_risk(&positions),
            steadyinvest_core::risk::total_invested(&positions),
        )
    }

    /// Ensure the single default portfolio exists and return it (FR36, single-portfolio). Lazily
    /// created with an injected id/timestamp (ADD15) on first use; idempotent thereafter. The id/
    /// timestamp are minted **only when the portfolio is absent** — so a repeat add doesn't burn an
    /// `IdGen` id (which would shift a deterministic test sequence) and the common path is a pure read.
    fn ensure_default_portfolio(&mut self) -> Result<PortfolioItem, String> {
        let journal = self.journal.as_ref().ok_or(MSG_NO_JOURNAL.to_string())?;
        if let Some(existing) = journal.first_portfolio().map_err(watch_error)? {
            return Ok(existing);
        }
        let id = self.idgen.new_id();
        let created_at = self.clock.now();
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .ensure_portfolio(id, DEFAULT_PORTFOLIO_NAME, &created_at)
            .map_err(watch_error)
    }

    /// Add a holding (FR36): a security symbol, a quantity and a purchase price in the reference
    /// currency. Validates the symbol (non-empty) and the two decimals (exact, quantity > 0, price
    /// ≥ 0) **in the app layer** — persistence stores faithfully. Id/timestamp from the injected
    /// sources. Guarded (read-only / no-journal / save-failure → a neutral notice).
    pub fn add_holding(
        &mut self,
        ticker: &str,
        quantity: &str,
        purchase_price: &str,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let ticker = ticker.trim();
        if ticker.is_empty() {
            return Err(MSG_HOLDING_INVALID_TICKER.to_string());
        }
        let (quantity, purchase_price) = validate_holding_amounts(quantity, purchase_price)?;
        let portfolio_id = self.ensure_default_portfolio()?.id;
        let id = self.idgen.new_id();
        let created_at = self.clock.now();
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .add_holding(
                id,
                portfolio_id,
                ticker,
                &quantity,
                &purchase_price,
                &created_at,
            )
            .map(|_| ())
            .map_err(watch_error)
    }

    /// Edit a holding's symbol, quantity and/or purchase price (FR36). Same validation as
    /// [`Self::add_holding`]. A no-op (identical values) writes nothing.
    pub fn update_holding(
        &mut self,
        id: Uuid,
        ticker: &str,
        quantity: &str,
        purchase_price: &str,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let ticker = ticker.trim();
        if ticker.is_empty() {
            return Err(MSG_HOLDING_INVALID_TICKER.to_string());
        }
        let (quantity, purchase_price) = validate_holding_amounts(quantity, purchase_price)?;
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .update_holding(id, ticker, &quantity, &purchase_price)
            .map_err(watch_error)
    }

    /// Remove a holding (FR36). Guarded; an absent id is a neutral no-op.
    pub fn delete_holding(&mut self, id: Uuid) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal.delete_holding(id).map_err(watch_error)
    }

    /// Record a **sell** chosen on a neutral trigger (Story 4.7, FR46/FR47) and remove the holding
    /// from the active register. Writes one SELL transaction — `quantity` = the holding's; `unit_price`
    /// = the matched study's `current_price` (the market fact, Story 4.4) if known, else the holding's
    /// `purchase_price`; `fees` = 0 (the fees workflow is Epic 6); `currency` = the caller's reference
    /// currency (FR63); `rationale` = the optional trimmed reason (`None` when blank). The sell row and
    /// the holding's **soft delete** are written **atomically** in one `record_sell` transaction — not
    /// a hard delete (the sell transaction's FK must keep a live referent, so the record survives; the
    /// holding just leaves the register via `sold_at`). The full ledger (partial sells, cost basis)
    /// stays Epic 6 / Story 6.3. Guarded (read-only / no-journal / save-failure → a neutral notice); an
    /// absent (or already-sold) id is refused.
    pub fn sell_holding(
        &mut self,
        holding_id: Uuid,
        rationale: &str,
        currency: &str,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let holding = self
            .list_holdings()
            .into_iter()
            .find(|h| h.id == holding_id)
            .ok_or(MSG_SAVE_FAILED.to_string())?;
        // The sale price: the matched study's current market price if known, else the cost basis.
        let unit_price = self
            .study_id_for_ticker(&holding.security_ticker)
            .and_then(|sid| self.get_study(sid))
            .and_then(|s| s.judgment.current_price)
            .map(|m| m.as_decimal().to_string())
            .unwrap_or_else(|| holding.purchase_price.clone());
        let rationale = rationale.trim();
        let rationale = (!rationale.is_empty()).then_some(rationale);
        let id = self.idgen.new_id();
        let now = self.clock.now();
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .record_sell(
                id,
                holding_id,
                &holding.quantity,
                &unit_price,
                "0",
                currency,
                rationale,
                &now,
            )
            .map(|_| ())
            .map_err(watch_error)
    }

    /// Set (or clear) a holding's trailing-stop percentage (Story 4.5, FR42). An empty `pct_input`
    /// clears the stop. Otherwise the pct is validated to `(0, 100)` and the level is **seeded fresh**
    /// from the *reference price* — the matched study's `current_price` if known, else the holding's
    /// `purchase_price` — so the user's chosen pct wins (they may tighten OR loosen the stop). The
    /// ratchet-up-only rule (FR42) governs the **automatic** price-driven trailing
    /// ([`Self::ratchet_trailing_stops_for_study`]), NOT an explicit re-parametrisation — folding the
    /// prior level here would make the displayed pct and level inconsistent (review finding). Both
    /// pct + level persist together (idempotent). Guarded.
    pub fn set_holding_trailing_stop(
        &mut self,
        holding_id: Uuid,
        pct_input: &str,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let pct_input = pct_input.trim();
        if pct_input.is_empty() {
            // Clear the stop (both fields → NULL).
            let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
            return journal
                .set_trailing_stop(holding_id, None, None)
                .map_err(watch_error);
        }
        let pct = Decimal::from_str_exact(pct_input)
            .ok()
            .filter(|p| p.is_sign_positive() && !p.is_zero() && *p < Decimal::ONE_HUNDRED)
            .ok_or(MSG_HOLDING_INVALID_STOP.to_string())?;
        let holding = self
            .list_holdings()
            .into_iter()
            .find(|h| h.id == holding_id)
            .ok_or(MSG_HOLDING_INVALID_STOP.to_string())?;
        let reference_price = self
            .study_id_for_ticker(&holding.security_ticker)
            .and_then(|sid| self.get_study(sid))
            .and_then(|s| s.judgment.current_price)
            .map(|m| m.as_decimal())
            .or_else(|| Decimal::from_str_exact(&holding.purchase_price).ok())
            .ok_or(MSG_HOLDING_INVALID_STOP.to_string())?;
        // Seed fresh (no prior level) — an explicit set is the user redefining the stop, not an
        // automatic ratchet, so it may move the level down as well as up.
        let level = steadyinvest_core::risk::ratchet_trailing_stop(None, reference_price, pct);
        // Normalize (drop trailing zeros) so the stored string is canonical — re-computing the same
        // value yields the same string, which keeps the persistence no-op idempotency guard honest.
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .set_trailing_stop(
                holding_id,
                Some(&pct.normalize().to_string()),
                Some(&level.normalize().to_string()),
            )
            .map_err(watch_error)
    }

    /// Ratchet the trailing-stop level of every holding of `study_id`'s ticker against a fresh price
    /// (Story 4.5, FR42) — called after a holdings price refresh fills the study's `current_price`
    /// ([`Self::apply_holding_price`]). Only holdings that **have** a stop set are touched; the
    /// `core::risk` ratchet (and the persistence no-op guard) ensure a falling price writes nothing.
    pub fn ratchet_trailing_stops_for_study(
        &mut self,
        study_id: Uuid,
        price: Decimal,
    ) -> Result<(), String> {
        if self.read_only {
            return Ok(()); // a read-only refresh simply doesn't ratchet — never an error
        }
        let Some(ticker) = self.get_study(study_id).map(|s| s.security_ticker) else {
            return Ok(());
        };
        let targets: Vec<(Uuid, Decimal, Option<Decimal>)> = self
            .list_holdings()
            .into_iter()
            .filter(|h| h.security_ticker.eq_ignore_ascii_case(&ticker))
            .filter_map(|h| {
                let pct = h
                    .trailing_stop_pct
                    .as_deref()
                    .and_then(|s| Decimal::from_str_exact(s).ok())?;
                let prior = h
                    .trailing_stop_level
                    .as_deref()
                    .and_then(|s| Decimal::from_str_exact(s).ok());
                Some((h.id, pct, prior))
            })
            .collect();
        for (id, pct, prior) in targets {
            let level = steadyinvest_core::risk::ratchet_trailing_stop(prior, price, pct);
            let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
            journal
                .set_trailing_stop(
                    id,
                    Some(&pct.normalize().to_string()),
                    Some(&level.normalize().to_string()),
                )
                .map_err(watch_error)?;
        }
        Ok(())
    }

    /// Archive a study (Story 2.12, FR54): flip `status` to `"archived"` so it leaves the default
    /// dashboard view. Reversible via [`Self::unarchive_study`]. Guarded (read-only / no-journal /
    /// save-failure → a neutral notice, never a silent `.ok()`). Not part of the per-open-study undo
    /// stack — it's a dashboard-lifecycle action, reversed by un-archiving, not by Ctrl+Z.
    pub fn archive_study(&mut self, study_id: Uuid) -> Result<(), String> {
        self.set_study_status(study_id, "archived")
    }

    /// Un-archive a study (Story 2.12): flip `status` back to `"active"`. The inverse of
    /// [`Self::archive_study`]; same guards.
    pub fn unarchive_study(&mut self, study_id: Uuid) -> Result<(), String> {
        self.set_study_status(study_id, "active")
    }

    /// The shared status-change rail (read-only / no-journal / save-failure guards → persist).
    fn set_study_status(&mut self, study_id: Uuid, status: &str) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let Some(journal) = self.journal.as_mut() else {
            return Err(MSG_NO_JOURNAL.to_string());
        };
        match journal.set_study_status(study_id, status) {
            Ok(()) => Ok(()),
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Permanently delete a study and its FR51 `judgments` time-series rows in one transaction
    /// (Story 2.12, FR55) — atomic, FK-safe, no orphan, other studies untouched. **Irreversible**
    /// (distinct from archive's reversible hide); not undoable. Clears the in-memory undo history so
    /// a later Ctrl+Z can't resurrect a pointer to a deleted study. Guarded (read-only / no-journal /
    /// save-failure → a neutral notice, never a silent `.ok()`).
    pub fn delete_study(&mut self, study_id: Uuid) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let Some(journal) = self.journal.as_mut() else {
            return Err(MSG_NO_JOURNAL.to_string());
        };
        match journal.delete_study(study_id) {
            Ok(()) => {
                // The deleted study must not linger in any undo/redo snapshot stack.
                self.reset_undo();
                Ok(())
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Validate inputs and create a study (FR1): id/journal_id/created_at from the injected
    /// sources, all-`None` judgment, `Study::new` schema-stamped, written with `put_study` (which
    /// bumps `logical_version`). Returns the new id on success, or a neutral notice for a banner.
    pub fn create_study(&mut self, ticker: &str, currency: &str) -> Result<Uuid, String> {
        let ticker = ticker.trim();
        let currency = currency.trim();
        if ticker.is_empty() {
            return Err(MSG_BLANK_TICKER.to_string());
        }
        if currency.is_empty() {
            return Err(MSG_BLANK_CURRENCY.to_string());
        }
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let Some(journal) = self.journal.as_mut() else {
            return Err(MSG_NO_JOURNAL.to_string());
        };

        let study = Study::new(
            self.idgen.new_id(),
            journal.id(),
            ticker,
            currency.to_uppercase(),
            empty_judgment(),
            self.clock.now(),
        );
        let id = study.id;
        match journal.put_study(&study) {
            Ok(()) => Ok(id),
            // The newer-schema guard can also fire here (defense in depth); name it neutrally.
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Export one study to its portable envelope JSON (Story 5.2, FR59) — the serialized data
    /// contract + `schema_version` + integrity hash (NOT a raw `.db`). A pure read; the caller writes
    /// the string to a user-chosen file. Guarded: no journal / missing id → a neutral notice.
    pub fn export_study(&self, id: Uuid) -> Result<String, String> {
        let study = self.get_study(id).ok_or(MSG_EXPORT_MISSING.to_string())?;
        Ok(steadyinvest_contract::to_export_json(&study))
    }

    /// Import a study from its portable envelope JSON (Story 5.2, FR59/NFR-R5): verify integrity +
    /// `schema_version`, then persist. The study's **own id is preserved** (a re-import of the same
    /// study updates in place, never duplicates); its `journal_id` is **rebound to this journal** so a
    /// study seeded/shared from another journal joins the current one (identity = the study id).
    /// Returns `(id, overwrote_existing)` — an import onto a pre-existing id **updates** it, which the
    /// caller surfaces distinctly (AC3); an overwrite onto an **archived** study also reactivates it,
    /// so an imported study is never silently left hidden. Each [`ImportError`] maps to a neutral
    /// notice; nothing is written on a rejection. Guarded.
    pub fn import_study(&mut self, json: &str) -> Result<(Uuid, bool), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let mut study = steadyinvest_contract::from_export_json(json).map_err(|e| match e {
            ImportError::Integrity => MSG_IMPORT_INTEGRITY.to_string(),
            ImportError::Version { .. } => MSG_IMPORT_VERSION.to_string(),
            ImportError::Malformed(_) => MSG_IMPORT_MALFORMED.to_string(),
        })?;
        let id = study.id;
        // Detect a pre-existing study with this id (and whether it is currently archived/hidden) so
        // the import is surfaced as an UPDATE, not a silent clobber (AC3 review finding).
        let existing_archived = self
            .list_studies()
            .into_iter()
            .find(|s| s.id == id)
            .map(|s| (true, s.status == "archived"));
        let overwrote = existing_archived.is_some();
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        study.journal_id = journal.id();
        match journal.put_study(&study) {
            Ok(()) => {}
            Err(PersistError::NewerJournalSchema { .. }) => {
                return Err(MSG_READ_ONLY_WRITE.to_string())
            }
            Err(error) => return Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
        // `put_study`'s upsert does not touch `status`; an imported study must be visible, so an
        // overwrite onto an archived id is reactivated.
        if existing_archived == Some((true, true)) {
            self.set_study_status(id, "active")?;
        }
        Ok((id, overwrote))
    }

    /// Export the **whole journal** to its portable envelope JSON (Story 5.3, FR60) — the serialized
    /// data contract for every entity + `schema_version` + `(journal_id, logical_version)` + integrity
    /// hash (NOT a raw `.db`). A pure read; the caller writes the string to a user-chosen file.
    /// Guarded: no journal → a neutral notice.
    pub fn export_journal(&self) -> Result<String, String> {
        let journal = self.journal.as_ref().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .export_journal()
            .map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))
    }

    /// Import a **whole journal** from its portable envelope JSON (Story 5.3, FR60/NFR-R5): verify
    /// integrity + `schema_version`, then apply **every entity atomically** (all-or-nothing, never
    /// partially). Entities are upserted by id (a re-import updates in place); studies are rebound to
    /// this journal. Returns an [`ImportSummary`] of what was applied; the caller surfaces the counts.
    /// Each rejection maps to a neutral notice (reusing the single-study integrity/version/malformed
    /// copy); nothing is written on a rejection. Guarded (read-only / no journal).
    pub fn import_journal(&mut self, text: &str) -> Result<ImportSummary, String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        let summary = journal.import_journal(text).map_err(|error| match error {
            PersistError::ImportIntegrity => MSG_IMPORT_INTEGRITY.to_string(),
            PersistError::ImportVersion { .. } => MSG_IMPORT_VERSION.to_string(),
            PersistError::ImportMalformed { .. } => MSG_IMPORT_MALFORMED.to_string(),
            PersistError::NewerJournalSchema { .. } => MSG_READ_ONLY_WRITE.to_string(),
            other => format!("{MSG_SAVE_FAILED} {other}"),
        })?;
        Ok(summary)
    }

    /// Create a raw `.db` backup of the live journal (Story 5.4, FR61) — checkpoint the WAL so the copy
    /// is self-contained, then copy the file to a `backups/` folder **beside the journal** (Story 5.5 —
    /// so backups follow a user-selected location; falls back to the OS data dir if the journal has no
    /// parent). Returns the written path (the caller surfaces it). Guarded: no journal → a neutral notice.
    pub fn create_backup(&self) -> Result<PathBuf, String> {
        let journal = self.journal.as_ref().ok_or(MSG_NO_JOURNAL.to_string())?;
        let live = self.path.as_ref().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .checkpoint()
            .map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))?;
        let version = journal
            .logical_version()
            .map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))?;
        // Story 5.5: backups live beside the journal (a `backups/` sibling of the `.db`), so a
        // user-selected location keeps its backups together. Fall back to the OS data dir only if the
        // journal path has no parent (degenerate).
        let dir = match live.parent() {
            // A real parent directory (an absolute journal path) → backups sit beside the journal.
            Some(parent) if !parent.as_os_str().is_empty() => parent.join("backups"),
            // A bare/relative path with no real parent → the OS data dir (never the process CWD).
            _ => directories::ProjectDirs::from("", "", "steadyinvest")
                .map(|d| d.data_dir().join("backups"))
                .ok_or(MSG_NO_DATA_DIR.to_string())?,
        };
        std::fs::create_dir_all(&dir).map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))?;
        // Key the filename on (id, version, timestamp) so two backups never silently overwrite each
        // other — a same-version backup (e.g. one taken right after a restore) keeps its own file. The
        // timestamp is filesystem-safe (no `:`).
        let stamp = self.clock.now().0.replace(':', "");
        let dest = dir.join(format!("journal-{}-v{version}-{stamp}.db", journal.id()));
        std::fs::copy(live, &dest).map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))?;
        Ok(dest)
    }

    /// Close the current journal cleanly (Story 5.5): checkpoint its WAL, then drop the handle — which
    /// releases its single-instance lock. The `path` is left as-is (the caller sets the new one).
    fn close_current(&mut self) {
        if let Some(journal) = self.journal.as_ref() {
            let _ = journal.checkpoint();
        }
        self.journal = None;
    }

    /// Open an already-opened journal at `path` into `self` with the given mode, replacing the current
    /// journal (Story 5.5). Records identity/version, resets undo. Maps the lock/open failures to
    /// neutral notices. The caller is responsible for having closed/saved the previous journal.
    fn adopt_open(&mut self, path: &Path, mode: JournalMode) -> Result<OpenOutcome, String> {
        match Journal::open_with_mode(path, mode) {
            Ok(journal) => {
                let logical_version = journal
                    .logical_version()
                    .map_err(|error| format!("{MSG_JOURNAL_OPEN_FAILED} {error}"))?;
                let outcome = OpenOutcome {
                    journal_id: journal.id(),
                    logical_version,
                    sync_warning: matches!(mode, JournalMode::Delete),
                };
                self.read_only = journal.is_read_only();
                self.journal = Some(journal);
                self.path = Some(path.to_path_buf());
                self.reset_undo();
                self.pending_restore = None;
                Ok(outcome)
            }
            Err(PersistError::LockHeld { .. }) => Err(MSG_JOURNAL_LOCKED.to_string()),
            Err(error) => Err(format!("{MSG_JOURNAL_OPEN_FAILED} {error}")),
        }
    }

    /// Re-acquire the previous journal after a failed open/create, so the app is never journal-less
    /// (Story 5.5) — best-effort (mirrors the Story 5.4 `reopen_live` discipline).
    fn restore_previous(&mut self, prev: Option<PathBuf>) {
        if let Some(prev) = prev {
            let mode = sync_mode_for(&prev);
            let _ = self.adopt_open(&prev, mode);
        }
    }

    /// Open a journal at `path`, switching away from the current one (Story 5.5, AC1). Closes the
    /// current journal cleanly first (checkpoint + release its lock), opens the target with the
    /// sync-folder-appropriate [`JournalMode`], and returns an [`OpenOutcome`] (identity, version,
    /// whether a sync-folder warning applies). A failed open leaves the **previous** journal open
    /// (never journal-less). The caller records the recent entry + persists app-config + re-renders.
    pub fn open_journal(&mut self, path: &Path) -> Result<OpenOutcome, String> {
        // Re-selecting the journal that is already open is a no-op — closing + reopening it would
        // pointlessly wipe the undo history. Return the current identity without touching anything.
        if let Some(current) = self.path.clone() {
            if self.journal.is_some() && same_file_path(&current, path) {
                return Ok(OpenOutcome {
                    journal_id: self.journal_id().unwrap_or_else(Uuid::nil),
                    logical_version: self.logical_version_or_zero(),
                    sync_warning: matches!(sync_mode_for(path), JournalMode::Delete),
                });
            }
        }
        let prev = self.path.clone();
        self.close_current();
        match self.adopt_open(path, sync_mode_for(path)) {
            Ok(outcome) => Ok(outcome),
            Err(notice) => {
                self.restore_previous(prev);
                Err(notice)
            }
        }
    }

    /// Create a new journal at `dir/<name>.db` and switch to it (Story 5.5, AC1). Closes the current
    /// journal cleanly first; mints the identity + creation time from the injected sources (ADD15);
    /// uses the sync-folder-appropriate mode. A failed create leaves the previous journal open.
    pub fn create_journal(&mut self, dir: &Path, name: &str) -> Result<OpenOutcome, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(MSG_JOURNAL_OPEN_FAILED.to_string());
        }
        let file_name = if trimmed.ends_with(".db") {
            trimmed.to_string()
        } else {
            format!("{trimmed}.db")
        };
        let path = dir.join(file_name);
        let mode = sync_mode_for(dir);
        let prev = self.path.clone();
        self.close_current();
        let id = self.idgen.new_id();
        let created_at = self.clock.now();
        match Journal::create_with_mode(&path, id, &created_at, mode) {
            Ok(journal) => {
                let logical_version = journal.logical_version().unwrap_or(0);
                let outcome = OpenOutcome {
                    journal_id: journal.id(),
                    logical_version,
                    sync_warning: matches!(mode, JournalMode::Delete),
                };
                self.read_only = journal.is_read_only();
                self.journal = Some(journal);
                self.path = Some(path);
                self.reset_undo();
                self.pending_restore = None;
                Ok(outcome)
            }
            Err(PersistError::JournalExists(_)) => {
                self.restore_previous(prev);
                Err(MSG_JOURNAL_OPEN_FAILED.to_string())
            }
            Err(PersistError::LockHeld { .. }) => {
                self.restore_previous(prev);
                Err(MSG_JOURNAL_LOCKED.to_string())
            }
            Err(error) => {
                self.restore_previous(prev);
                Err(format!("{MSG_JOURNAL_OPEN_FAILED} {error}"))
            }
        }
    }

    /// Reclaim a **stale** single-instance lock at `path` (a lock left by a crashed run) and open the
    /// journal (Story 5.5, AC3). Only clears the lock when it is actually stale — never steals a live
    /// instance's lock.
    pub fn reclaim_and_open(&mut self, path: &Path) -> Result<OpenOutcome, String> {
        if lock_is_stale(path) {
            let _ = clear_lock(path);
        }
        self.open_journal(path)
    }

    /// Assess a candidate backup `.db` against the current journal and **park** it for confirmation
    /// (Story 5.4, AC1/AC2). Validates read-only (integrity + schema-version + identity), never
    /// touching the live journal. A soft verdict (Ok / StaleOlder / ForeignJournal) parks a pending
    /// restore and returns the assessment for the UI to surface + confirm; a hard refusal (corrupt /
    /// newer-schema / unreadable / not-a-journal) parks nothing and returns the neutral cause. FR61:
    /// nothing is applied here.
    pub fn request_restore(&mut self, backup_path: &str) -> Result<RestoreAssessment, String> {
        self.pending_restore = None;
        let info = inspect_backup(backup_path).map_err(|error| match error {
            PersistError::CorruptJournalMeta { .. } => MSG_RESTORE_NOT_A_JOURNAL.to_string(),
            _ => MSG_RESTORE_UNREADABLE.to_string(),
        })?;

        let verdict = if !info.integrity_ok {
            RestoreVerdict::IntegrityFailed
        } else if info.is_newer_schema() {
            RestoreVerdict::NewerSchema {
                found: info.file_user_version,
                supported: info.supported_version,
            }
        } else {
            match self.journal.as_ref() {
                // No journal open → nothing to clash with; a forward restore.
                None => RestoreVerdict::Ok,
                Some(journal) if journal.id() != info.journal_id => RestoreVerdict::ForeignJournal,
                Some(journal) => {
                    let current = journal
                        .logical_version()
                        .map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))?;
                    if info.logical_version < current {
                        RestoreVerdict::StaleOlder {
                            backup: info.logical_version,
                            current,
                        }
                    } else {
                        RestoreVerdict::Ok
                    }
                }
            }
        };

        let assessment = RestoreAssessment {
            journal_id: info.journal_id,
            logical_version: info.logical_version,
            verdict: verdict.clone(),
        };

        if assessment.is_confirmable() {
            self.pending_restore = Some(PendingRestore {
                backup_path: PathBuf::from(backup_path),
            });
            Ok(assessment)
        } else {
            // A hard refusal — surface the cause, park nothing (confirm can't fire).
            Err(match verdict {
                RestoreVerdict::IntegrityFailed => MSG_RESTORE_INTEGRITY.to_string(),
                RestoreVerdict::NewerSchema { .. } => MSG_RESTORE_NEWER_SCHEMA.to_string(),
                _ => MSG_RESTORE_UNREADABLE.to_string(),
            })
        }
    }

    /// Apply the parked restore (Story 5.4, AC3) **safely**: re-validate the file at confirm time
    /// (TOCTOU — the parked path may have changed), checkpoint + snapshot the live journal, swap the
    /// file **atomically** (temp + rename, so a failure leaves the live journal intact), reopen, reset
    /// undo. If the restored file will not open, **roll back to the snapshot** so the user's original
    /// journal is never lost. A restore of the journal **onto itself** is a no-op. A neutral no-op
    /// error if nothing is parked.
    pub fn confirm_restore(&mut self) -> Result<(), String> {
        let pending = self
            .pending_restore
            .take()
            .ok_or(MSG_RESTORE_FAILED.to_string())?;
        let live = self.path.clone().ok_or(MSG_NO_JOURNAL.to_string())?;

        // Restoring the journal onto itself is a no-op — the live journal already IS this content (and
        // it sidesteps the `fs::copy`-onto-itself truncation hazard). The live handle stays open.
        if same_file_path(&live, &pending.backup_path) {
            return Ok(());
        }

        // Re-validate at confirm time: the file may have changed since `request_restore` parked it. A
        // now-corrupt / newer-schema / unreadable backup is refused **without touching** the live
        // journal — the "validate BEFORE overwrite" guarantee (FR61) holds against TOCTOU.
        let info = inspect_backup(&pending.backup_path).map_err(|error| match error {
            PersistError::CorruptJournalMeta { .. } => MSG_RESTORE_NOT_A_JOURNAL.to_string(),
            _ => MSG_RESTORE_UNREADABLE.to_string(),
        })?;
        if !info.integrity_ok {
            return Err(MSG_RESTORE_INTEGRITY.to_string());
        }
        if info.is_newer_schema() {
            return Err(MSG_RESTORE_NEWER_SCHEMA.to_string());
        }

        // Checkpoint the live journal so its `.db` is self-contained, then drop the handle (one
        // connection per Journal — swapping over an open file is unsafe) and snapshot it for rollback.
        if let Some(journal) = self.journal.as_ref() {
            let _ = journal.checkpoint();
        }
        self.journal = None;
        let snapshot = path_with_suffix(&live, "-prerestore");
        let have_snapshot = std::fs::copy(&live, &snapshot).is_ok();

        // Atomic swap — a failure leaves the live file untouched, so the original survives.
        if let Err(error) = restore_journal_file(&live, &pending.backup_path) {
            let _ = std::fs::remove_file(&snapshot);
            self.reopen_live(&live);
            return Err(format!("{MSG_RESTORE_FAILED} {error}"));
        }

        match Journal::open_with_mode(&live, sync_mode_for(&live)) {
            Ok(journal) => {
                let _ = std::fs::remove_file(&snapshot);
                self.read_only = journal.is_read_only();
                self.journal = Some(journal);
                self.reset_undo();
                Ok(())
            }
            Err(error) => {
                // The swap succeeded but the restored file will not open — roll the snapshot back so
                // the user's original journal is not lost, then reopen it.
                if have_snapshot {
                    let _ = restore_journal_file(&live, &snapshot);
                }
                let _ = std::fs::remove_file(&snapshot);
                self.reopen_live(&live);
                Err(format!("{MSG_RESTORE_FAILED} {error}"))
            }
        }
    }

    /// Discard a parked restore (Story 5.4) — no write.
    pub fn cancel_restore(&mut self) {
        self.pending_restore = None;
    }

    /// Test-only: whether a restore is currently parked awaiting confirmation (Story 5.4).
    #[cfg(test)]
    fn has_pending_restore(&self) -> bool {
        self.pending_restore.is_some()
    }

    /// Best-effort reopen of the live journal at `path` (used to recover after a failed restore swap so
    /// the app is never left journal-less).
    fn reopen_live(&mut self, path: &Path) {
        match Journal::open_with_mode(path, sync_mode_for(path)) {
            Ok(journal) => {
                self.read_only = journal.is_read_only();
                self.journal = Some(journal);
            }
            Err(error) => {
                tracing::warn!("could not reopen journal after a failed restore: {error}");
                self.journal = None;
            }
        }
    }
}

/// Whether two paths point at the **same file** (Story 5.4) — canonicalized to resolve symlinks /
/// relative components / a NAS path that aliases the live journal; falls back to a raw comparison when
/// a path cannot be canonicalized (e.g. it does not exist).
fn same_file_path(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

/// Append a suffix to a path's file name (e.g. `journal.db` → `journal.db-prerestore`) — used for the
/// pre-restore snapshot sibling file.
fn path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

impl JournalState {
    /// Build the manual [`Provenance`] for an edit (Story 2.4): `source = Manual`, `timestamp` from
    /// the injected [`Clock`] (ADD15 — never a scattered wall clock). For a manually-entered **leaf**
    /// input there is no app-side per-cell version counter and no upstream dependency digest, so v1
    /// uses defensible sentinels — `logical_version = 1` and `hash_of_dependencies = "manual"` —
    /// recorded in the 2.4 interpretations issue; both earn real meaning in Epic 3 reconciliation.
    /// (`Provenance` performs no validation on these strings — `contract` module doc.)
    fn manual_provenance(&self) -> Provenance {
        Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: self.clock.now(),
            hash_of_dependencies: "manual".to_string(),
        }
    }

    /// Provenance for a provider-fetched leaf (Story 3.1): `Source::Provider`, the injected clock's
    /// timestamp, and the **real** dependency digest from the fetch (#21 — no longer the `"manual"`
    /// sentinel). `logical_version` stays the app-side sentinel `1` (there is no per-cell counter;
    /// the study-level bump on `put_study` records the act's timing).
    fn provider_provenance(&self, digest: String) -> Provenance {
        Provenance {
            source: Source::Provider,
            logical_version: 1,
            timestamp: self.clock.now(),
            hash_of_dependencies: digest,
        }
    }

    /// Apply a manual provider **refresh** (Story 3.3, FR21/FR29) — the single deliberate online
    /// action that re-fetches and recomputes. Subsumes the Story-3.1 first-fetch (an empty study
    /// builds its grid) and generalises it: an already-populated study now **updates** its
    /// provider/derived cells with the new value + timestamp, on top of filling former gaps.
    ///
    /// Per cell, branching on the **current** cell's source (see [`refresh_cell`]):
    /// - a **gap** (no value) is **filled** from the provider, whatever its skeleton source;
    /// - a present **`Source::Manual`** value is **skipped** — manual wins, never overwritten here
    ///   (non-destructive dual-value reconciliation of a divergent manual cell is Story 3.4);
    /// - a present **provider/derived** value is **re-stamped via [`Cell::edited`] only when the
    ///   value actually changed** — an equal re-fetch is a no-op (idempotency: no timestamp churn,
    ///   no phantom undo step, no `✓→?` demotion). A divergent value auto-demotes a `✓` provider
    ///   cell to `?` and degrades the dependent verdict in the same frame (the Epic-1 invariant 2b).
    ///
    /// Returns a [`RefreshReport`] (updated / filled counts + the classified [`RefreshCause`]) so the
    /// caller can state *why* it recomputed (price / input / FX). Routed through the atomic
    /// [`Self::mutate_study`] rail (one `put_study`, guards, undo-only-on-real-change). Provider cells
    /// are `Review::None`, so the verdict stays Provisional/Withheld until the user validates.
    pub fn apply_provider_refresh(
        &mut self,
        study_id: Uuid,
        fetched: &FetchedFinancials,
    ) -> Result<RefreshReport, String> {
        let provenance = self.provider_provenance(fetched.digest.clone());
        let years: Vec<CanonicalYear> = fetched.canonical.years.clone();
        // Story 4.4 (AC2/AC6): the latest `/eod` close is the present market price for the §4 zone.
        // `None` for a provider with no current price → `current_price` left untouched (pre-4.4 shape).
        let latest_price = fetched.latest_price;
        let report = std::cell::Cell::new(RefreshReport::default());
        let report_ref = &report;
        self.mutate_study(study_id, move |study| {
            // A successful refresh means the provider responded → the outage (Story 3.5) is over.
            // Clear the stale flag on EVERY provider cell up front, so cells this fetch does not
            // re-visit (an omitted optional field, a year outside the fetched set) also recover —
            // not just the ones whose value is re-confirmed below. A freshness-only recovery; it is
            // not counted in the report (no value moved), it just lets the verdict come back.
            for year in &mut study.years {
                for cell in year_cells_mut(year) {
                    if cell.source == Source::Provider && cell.freshness == Freshness::Stale {
                        cell.freshness = Freshness::Current;
                    }
                }
            }
            // A fresh (never-edited) study first gets empty to-fill provider rows, so the SAME
            // per-cell accounting path then fills + classifies them — one rail, one tally.
            if study.years.is_empty() {
                study.years = years
                    .iter()
                    .map(|cy| empty_provider_year(cy.year, &provenance))
                    .collect();
            }
            let mut acc = RefreshReport::default();
            for cy in &years {
                if let Some(yd) = study.years.iter_mut().find(|y| y.year == cy.year) {
                    acc = acc.merge(refresh_year(yd, cy, &provenance));
                }
            }
            // Story 4.4 (AC2/AC6): fill `current_price` from the latest close — a present *market
            // fact*, not a user-owned judgment (the forecast high/low EPS + P/E stay strictly manual,
            // FR33-safe). Written in the SAME mutation so the §4 zone recomputes in one undo step.
            // `mutate_study`'s `before != study` guard persists/records this even when no yearly cell
            // moved (a price-only refresh). `None` → unchanged.
            if let Some(price) = latest_price {
                study.judgment.current_price = Some(Money::from(price));
            }
            report_ref.set(acc);
        })?;
        Ok(report.get())
    }

    /// Set a study's `current_price` from a price-only holdings refresh (Story 4.4 / issue #50): the
    /// latest `/eod` close, fetched WITHOUT `/fundamentals` (so it works on the free EODHD tier). A
    /// present **market fact** (not a user-owned judgment — FR33-safe; the forecast high/low EPS + P/E
    /// stay manual), written through the atomic [`Self::mutate_study`] rail so the §4 zone recomputes
    /// and it is one undo step. Unlike [`Self::apply_provider_refresh`], it touches ONLY
    /// `current_price` — never the yearly provider cells (the holding refresh is price-led).
    pub fn apply_holding_price(&mut self, study_id: Uuid, price: Decimal) -> Result<(), String> {
        self.mutate_study(study_id, move |study| {
            study.judgment.current_price = Some(Money::from(price));
        })
    }

    /// Flag the open study's **provider-sourced** cells `Freshness::Stale` after a failed (or
    /// empty) refresh (Story 3.5, FR23/NFR-R1). Only the **freshness** axis moves — `value`, `source`,
    /// `review`, `coverage`, `provenance`, and any Story-3.4 `pending` are all retained (last-known
    /// values are never cleared). Manual/derived cells are untouched (the user owns manual data). The
    /// engine already degrades a validated-but-stale load-bearing input to `Verdict::Provisional`
    /// (Story 2.6 wiring), and the form already renders the dimmed `◦` murmur (Story 2.4) — this is
    /// the first production caller that SETS the flag. Returns the count flagged; idempotent (an
    /// already-stale cell is left untouched, so `mutate_study`'s `before != study` guard records no
    /// phantom undo step). Routed through the atomic [`Self::mutate_study`] rail.
    pub fn mark_provider_stale(&mut self, study_id: Uuid) -> Result<usize, String> {
        // Pre-check: if there is nothing to flag (no provider cells, or all already stale), return a
        // true no-op WITHOUT entering `mutate_study` — so a failed refresh on an already-stale study
        // (repeated offline retries), an empty study, or a manual-only study writes no journal
        // revision and bumps no `logical_version` (mirrors the Story-3.4 accept/keep guard; the
        // Synology-sync corruption risk makes avoidable writes worth suppressing).
        let candidates = self
            .get_study(study_id)
            .map(|s| count_provider_to_stale(&s))
            .unwrap_or(0);
        if candidates == 0 {
            return Ok(0);
        }
        let count = std::cell::Cell::new(0usize);
        let count_ref = &count;
        self.mutate_study(study_id, move |study| {
            let mut flagged = 0usize;
            for year in &mut study.years {
                for cell in year_cells_mut(year) {
                    if cell.source == Source::Provider && cell.freshness != Freshness::Stale {
                        cell.freshness = Freshness::Stale;
                        flagged += 1;
                    }
                }
            }
            count_ref.set(flagged);
        })?;
        Ok(count.get())
    }

    /// Manually set/clear a cell's value (FR16): parse-side `value` (already a [`Money`] or `None`)
    /// is routed through the one mutation rail [`contract::Cell::edited`] with a manual provenance,
    /// then the whole [`Study`] is upserted via [`Journal::put_study`] (bumps `logical_version`,
    /// appends the FR51 time-series). A `None` value clears the cell to a [`Coverage::ToFill`] gap —
    /// **never `0`**. Reuses the read-only / no-journal / save-failure guards (no silent `.ok()`).
    pub fn edit_cell(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
        value: Option<Money>,
    ) -> Result<(), String> {
        // Soft-lock (Story 2.5): a validated (`✓`) cell refuses a DIRECT typed edit — the sign-off is
        // load-bearing and must be cleared deliberately first (clear-✓ → `?`), never undone by a
        // stray keystroke. This is the Rust-side backstop behind the read-only TextInput; the UI also
        // guards visually, but the rail is the testable, authoritative refusal. (Bulk `paste_column`
        // keeps the `Cell::edited` auto-demote backstop instead — recorded interpretation: typing is
        // blocked (a), paste is allowed-with-demote (b).)
        if self.current_review(study_id, year_index, field) == Some(Review::Validated) {
            return Err(MSG_SOFT_LOCKED.to_string());
        }
        self.mutate_cell(study_id, year_index, field, |base, provenance| {
            base.edited(value, provenance)
        })
    }

    /// The current review tag of a cell, or `None` if the study/year/cell is absent (a never-touched
    /// optional column reads as no cell). Used by the soft-lock guard before a direct value edit.
    fn current_review(&self, study_id: Uuid, year_index: usize, field: &str) -> Option<Review> {
        let study = self.get_study(study_id)?;
        let year = study.years.get(year_index)?;
        entry::get_cell(year, field).map(|cell| cell.review)
    }

    /// The cell's current pending provider divergence, if any (Story 3.4) — lets the resolve actions
    /// short-circuit to a true no-op (no journal write) when there is nothing to reconcile.
    fn current_pending(
        &self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
    ) -> Option<PendingProvider> {
        let study = self.get_study(study_id)?;
        let year = study.years.get(year_index)?;
        entry::get_cell(year, field).and_then(|cell| cell.pending)
    }

    /// Set a cell's **review tag only** (Story 2.5, FR20): a review-only change that preserves the
    /// value, coverage, source, freshness and provenance verbatim — it does NOT route through
    /// [`Cell::edited`] (which would re-stamp source/freshness from a fresh provenance and re-derive
    /// coverage). The cycle `none → ? → ✓ → none` and the deliberate clear-✓ (`✓ → ?`) both land
    /// here; the UI computes the target. Setting a tag on a never-touched optional column materializes
    /// a to-fill gap carrying the tag — the value stays `None`, **never `0`**. Persisted via
    /// [`Journal::put_study`]; reuses the read-only / no-journal / save-failure guards.
    ///
    /// **Interpretation (recorded):** a review-only edit changes ONLY the `review` field — the cell's
    /// provenance (and its origin timestamp) is preserved verbatim, never re-stamped. The sign-off
    /// act's timing is captured by the study-level `logical_version` bump (FR51), not a per-cell
    /// provenance overwrite, so a value's source/fetch time is never lost to a review toggle.
    pub fn set_review(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
        review: Review,
    ) -> Result<(), String> {
        // #47: a value-less cell may be flagged `?` (a gap to fill) but must NEVER be `✓`-validated.
        // Validating "nothing" — an existing to-fill cell OR a never-touched optional column that this
        // call would materialize as an empty gap — is degenerate: a later refresh gap-fills it and
        // `provider_cell` resets the review to `None`, so the `✓` vanishes `Validated → None` (NOT
        // `→ ToReview`), silently dropping the badge and escaping the Story-3.6 re-validate count.
        // Refuse it as a neutral no-op (no journal write, no undo step); `?`/`none` stay allowed.
        if review == Review::Validated {
            let value_present = self
                .get_study(study_id)
                .and_then(|study| {
                    study
                        .years
                        .get(year_index)
                        .and_then(|year| entry::get_cell(year, field))
                        .map(|cell| cell.value.is_some())
                })
                .unwrap_or(false);
            if !value_present {
                return Ok(());
            }
        }
        self.mutate_cell(study_id, year_index, field, move |base, _provenance| Cell {
            review,
            // Re-validating reconciles a pending divergence (Story 3.4 AC4): the kept value stands
            // and the "provider differs" annotation clears. A non-✓ review leaves any pending intact.
            pending: if review == Review::Validated {
                None
            } else {
                base.pending.clone()
            },
            ..base
        })
    }

    /// Resolve a pending divergence by **accepting the provider value** (Story 3.4, AC4): the cell
    /// takes its pending provider value through the edit rail (→ `Source::Provider`,
    /// `Review::ToReview` so it is re-checked, pending cleared by `edited`). A neutral no-op if there
    /// is no pending. Routed through the atomic `mutate_cell` rail (guards, undo).
    pub fn accept_provider_value(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
    ) -> Result<(), String> {
        // True no-op when there is nothing to reconcile (no journal write, no undo step).
        let Some(pending) = self.current_pending(study_id, year_index, field) else {
            return Ok(());
        };
        // A pending with no value (only representable by a future caller — the refresh path never
        // produces one) would BLANK the manual value; treat it as keep-manual instead (never destroy).
        if pending.value.is_none() {
            return self.keep_manual_value(study_id, year_index, field);
        }
        self.mutate_cell(study_id, year_index, field, move |base, _provenance| {
            base.edited(pending.value, pending.provenance)
        })
    }

    /// Resolve a pending divergence by **keeping the manual value** (Story 3.4, AC4): the live value
    /// stands; only the pending "provider differs" annotation is cleared (the `✓` was already demoted
    /// to `?` by the divergence — keep-manual just dismisses the annotation, leaving the review as-is
    /// for the user to re-validate). A neutral no-op if there is no pending.
    pub fn keep_manual_value(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
    ) -> Result<(), String> {
        // True no-op when there is no pending to dismiss (no journal write, no undo step).
        if self.current_pending(study_id, year_index, field).is_none() {
            return Ok(());
        }
        self.mutate_cell(study_id, year_index, field, |base, _provenance| Cell {
            pending: None,
            ..base
        })
    }

    /// Bulk "unlock all" (Story 2.5, FR20): flip every `Review::Validated → ToReview` within `scope`
    /// (study / year / metric), leaving `None`/`ToReview` cells untouched, in **one** persisted
    /// upsert (one `logical_version` bump). Returns the count of cells actually flipped (surfaced in a
    /// neutral notice). A review-only flip — values/coverage are never touched. Reuses the read-only /
    /// no-journal / save-failure guards.
    pub fn unlock_all(&mut self, study_id: Uuid, scope: &UnlockScope) -> Result<usize, String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let before = study.clone(); // pre-mutation snapshot for undo (Story 2.9)
        let mut flipped = 0usize;
        for (year_index, year) in study.years.iter_mut().enumerate() {
            for field in entry::ALL_FIELDS {
                if !scope.covers(year_index, field) {
                    continue;
                }
                let Some(cell) = entry::get_cell(year, field) else {
                    continue; // an absent optional column carries no sign-off
                };
                if cell.review != Review::Validated {
                    continue;
                }
                let demoted = Cell {
                    review: Review::ToReview,
                    ..cell
                };
                entry::set_cell(year, field, demoted).map_err(|()| MSG_SAVE_FAILED.to_string())?;
                flipped += 1;
            }
        }
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study(&study)
        };
        match result {
            Ok(()) => {
                // Only an unlock that actually flipped a ✓ is an undoable change.
                if flipped > 0 {
                    self.history.record(before);
                }
                Ok(flipped)
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Count the validated (`✓`) cells the given `scope` would flip — for the confirmation prompt
    /// (the bulk change is never silent). A read-only/pure query; never mutates or persists.
    pub fn count_validated(&self, study_id: Uuid, scope: &UnlockScope) -> usize {
        let Some(study) = self.get_study(study_id) else {
            return 0;
        };
        let mut count = 0usize;
        for (year_index, year) in study.years.iter().enumerate() {
            for field in entry::ALL_FIELDS {
                if !scope.covers(year_index, field) {
                    continue;
                }
                if entry::get_cell(year, field).map(|c| c.review) == Some(Review::Validated) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Mark a cell **not-available-accepted** (a deliberate, permanent gap — FR19), or clear that
    /// back to a to-fill gap (`accepted = false`). The value is cleared to `None` either way; only
    /// the coverage differs, so this is N/A vs to-fill vs 0 kept distinct. Persisted through the
    /// same upsert rail as [`Self::edit_cell`].
    pub fn set_not_available(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
        accepted: bool,
    ) -> Result<(), String> {
        // Soft-lock (Story 2.5): the not-available gesture is a value/coverage mutation, and AC 2
        // names it among the edits a `✓` cell must refuse — routed through `Cell::edited(None, …)` it
        // would both blank the value AND demote `✓ → ?` (a divergent edit), silently undoing the
        // sign-off. The UI swallows Ctrl+Space on a locked cell; this is the authoritative, testable
        // Rust backstop, symmetric with `edit_cell`, so the sign-off can never be lost by this path.
        if self.current_review(study_id, year_index, field) == Some(Review::Validated) {
            return Err(MSG_SOFT_LOCKED.to_string());
        }
        self.mutate_cell(study_id, year_index, field, move |base, provenance| {
            // Reuse the edit rail for value/source/freshness/review, then override coverage only —
            // `NotAvailableAccepted` is a coverage-only gesture, not reachable through `edited`.
            let cleared = base.edited(None, provenance);
            Cell {
                coverage: if accepted {
                    Coverage::NotAvailableAccepted
                } else {
                    Coverage::ToFill
                },
                ..cleared
            }
        })
    }

    /// The shared validate→materialize→mutate→persist path for a single cell. `make` builds the new
    /// cell from the current one (cloned, snapshot semantics) and a fresh manual provenance. The
    /// year-grid skeleton is materialized **on first edit** (not before — an untouched study is
    /// never written as all-empty rows).
    fn mutate_cell(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
        make: impl FnOnce(Cell, Provenance) -> Cell,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        // Re-read the authoritative study, mutate one cell, write it back (the 2.2/2.3 rail).
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        // The pre-mutation snapshot for undo (Story 2.9) — captured as read, before any materialize.
        let before = study.clone();
        if study.years.is_empty() {
            study.years =
                entry::materialize_year_window(&study.created_at, &self.manual_provenance());
        }
        let year = study
            .years
            .get_mut(year_index)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let base = entry::get_cell(year, field)
            .unwrap_or_else(|| entry::tofill_cell(self.manual_provenance()));
        let new_cell = make(base, self.manual_provenance());
        entry::set_cell(year, field, new_cell).map_err(|()| MSG_SAVE_FAILED.to_string())?;

        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study(&study)
        };
        match result {
            Ok(()) => {
                // Only a REAL change is undoable — a no-op edit (same value re-typed, same option
                // re-selected, same review tag) must not push a phantom step or clear redo (review P4).
                if before != study {
                    self.history.record(before);
                }
                Ok(())
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Paste a parsed column into consecutive years of the **same field**, downward from
    /// `start_year` (FR16). Each value is already a locale-parsed [`Money`] / `None`
    /// ([`entry::parse_pasted_column`]); a `None` line leaves its cell an empty to-fill gap (**never
    /// `0`**). Lines past the last year are dropped. One upsert for the whole column (one
    /// `logical_version` bump). Returns the number of cells actually filled (the caller compares it
    /// with the column length to surface a neutral "some lines dropped" notice).
    pub fn paste_column(
        &mut self,
        study_id: Uuid,
        start_year: usize,
        field: &str,
        values: &[Option<Money>],
    ) -> Result<usize, String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let before = study.clone(); // pre-mutation snapshot for undo (Story 2.9)
        if study.years.is_empty() {
            study.years =
                entry::materialize_year_window(&study.created_at, &self.manual_provenance());
        }
        let mut filled = 0usize;
        for (offset, value) in values.iter().enumerate() {
            let Some(year) = study.years.get_mut(start_year + offset) else {
                break; // ran past the grid bottom — drop the surplus line
            };
            let base = entry::get_cell(year, field)
                .unwrap_or_else(|| entry::tofill_cell(self.manual_provenance()));
            let new_cell = base.edited(*value, self.manual_provenance());
            entry::set_cell(year, field, new_cell).map_err(|()| MSG_SAVE_FAILED.to_string())?;
            filled += 1;
        }
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study(&study)
        };
        match result {
            Ok(()) => {
                // Only a real change is undoable (review P4).
                if before != study {
                    self.history.record(before);
                }
                Ok(filled)
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Set (or clear) one **numeric judgment field** (Story 2.6, FR6/FR31): re-read the study, set
    /// the one `Judgment` field on the mutation rail, then `put_study` — reusing the read-only /
    /// no-journal / save-failure guards (no silent `.ok()`). A cleared field is `None`, **never `0`**
    /// (the project's most-repeated rail). The `field` key is the Slint callback's wire identifier
    /// ([`judgment_field`] maps it to the struct field).
    pub fn set_judgment_field(
        &mut self,
        study_id: Uuid,
        field: &str,
        value: Option<Money>,
    ) -> Result<(), String> {
        self.mutate_judgment(study_id, |judgment| {
            apply_judgment_field(judgment, field, value)
        })
    }

    /// Select the §4 forecast-low option (Story 2.6) — a judgment edit through the same rail.
    pub fn set_forecast_low_option(
        &mut self,
        study_id: Uuid,
        option: ForecastLowOption,
    ) -> Result<(), String> {
        self.mutate_judgment(study_id, |judgment| {
            judgment.forecast_low_option = option;
            true
        })
    }

    /// Set (or clear) the study-level **decision rationale** (Story 2.10, FR49): the user's free-text
    /// "why I judged this way" note. Trims the incoming text and stores `Some(trimmed)`, or `None`
    /// when it is empty/whitespace-only — the project's "absence ≠ empty value" rail (never the
    /// empty-but-present `Some("")` surprise). Routed through [`Self::mutate_study`] so it is atomic
    /// (one `put_study`, `logical_version` bumped), guarded (read-only / no-journal / save-failure →
    /// a neutral notice, never a silent `.ok()`), and undoable (recorded only on a real change). The
    /// rationale is the user's own words and is therefore **never** posture-scanned — only the
    /// system-supplied label/placeholder are (FR13).
    pub fn set_rationale(&mut self, study_id: Uuid, text: Option<String>) -> Result<(), String> {
        let normalized = text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        self.mutate_study(study_id, move |study| {
            study.rationale = normalized;
        })
    }

    /// Extend the study's data window forward by one year (Story 2.11, FR3): the **annual roll-forward**
    /// ritual. Appends a fresh [`entry::tofill_year`] column for `latest_year + 1` (all cells
    /// [`Coverage::ToFill`], no value computed — adding a year is structure, not calculation). The
    /// engine then re-bases its canonical 5-year forward projection off the new latest **usable** year
    /// once that year's EPS is entered ([`core`] reads `latest_usable`), so zones/verdict recompute in
    /// the same coherence frame — **no method change** (`FORECAST_HORIZON_YEARS` stays `5`). Rides
    /// [`Self::mutate_study`]: atomic (one `put_study`, `logical_version` bumped), guarded (read-only /
    /// no-journal / save-failure → a neutral notice), and undoable (an append always changes `years`,
    /// so it always records one undo step). A never-edited study (empty `years`) first materializes the
    /// canonical window (the in-memory view the user sees), then appends — so "+ année" on a fresh study
    /// grows it 5 → 6, never errors. The degraded all-empty case (unreadable `created_at`) appends
    /// `year 0` — safe, never a panic.
    pub fn extend_history(&mut self, study_id: Uuid) -> Result<(), String> {
        let provenance = self.manual_provenance();
        self.mutate_study(study_id, move |study| {
            // Materialize the canonical window on first touch (the 2.4 materialize-on-first-edit rail),
            // so extending a never-edited study grows the window the user actually sees, not an empty one.
            if study.years.is_empty() {
                study.years = entry::materialize_year_window(&study.created_at, &provenance);
            }
            let next_year = study
                .years
                .iter()
                .map(|y| y.year)
                .max()
                // `saturating_add` keeps the year monotonic even at the i32 ceiling (defensive — real
                // fiscal years never approach it, but the value is read from the journal blob).
                .map(|latest| latest.saturating_add(1))
                .or_else(|| entry::created_year(&study.created_at))
                .unwrap_or(0);
            // Newest sits at the bottom (oldest→newest SSG order); the appended year is the new max.
            study.years.push(entry::tofill_year(next_year, provenance));
        })
    }

    /// The shared re-read → mutate-whole-study → persist path for a study-level field that is neither
    /// a review-tagged [`Cell`] nor a [`Judgment`] input (Story 2.10: the decision rationale). Mirrors
    /// [`Self::mutate_judgment`] but hands `apply` the whole [`Study`]. Records an undo snapshot only
    /// on a real change (`before != study`, the Story-2.9 guard) so a no-op re-save pushes no phantom
    /// step. Reuses the read-only / no-journal / save-failure guards.
    fn mutate_study(
        &mut self,
        study_id: Uuid,
        apply: impl FnOnce(&mut Study),
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let before = study.clone(); // pre-mutation snapshot for undo (Story 2.9)
        apply(&mut study);
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study(&study)
        };
        match result {
            Ok(()) => {
                // Only a REAL change is undoable — re-saving the same rationale must not push a
                // phantom step or clear redo (review P4).
                if before != study {
                    self.history.record(before);
                }
                Ok(())
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// The shared re-read → mutate-judgment → persist path. `apply` returns `false` for an unknown
    /// field key (a neutral save-failure notice; never a panic). Mirrors [`Self::mutate_cell`] but
    /// for the bare `Judgment` snapshot (judgment inputs are not review-tagged `Cell`s).
    fn mutate_judgment(
        &mut self,
        study_id: Uuid,
        apply: impl FnOnce(&mut Judgment) -> bool,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let before = study.clone(); // pre-mutation snapshot for undo (Story 2.9)
        if !apply(&mut study.judgment) {
            return Err(MSG_SAVE_FAILED.to_string());
        }
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study(&study)
        };
        match result {
            Ok(()) => {
                // Only a REAL change is undoable — a no-op edit (same value re-typed, same option
                // re-selected, same review tag) must not push a phantom step or clear redo (review P4).
                if before != study {
                    self.history.record(before);
                }
                Ok(())
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Compute the coherent [`StudySnapshot`] for a study (Story 2.6 — THE engine-call site): re-read
    /// the authoritative study, run the `contract` → `core` mapping, `normalize` (a [`NormalizeError`]
    /// surfaces as the neutral [`MSG_NORMALIZE_FAILED`], never `unwrap`/`.ok()`), then
    /// `StudySnapshot::new` ONCE — so the outputs and the verdict are always one coherent frame
    /// (architecture: "an incoherent frame is structurally impossible"). `Err` for an absent study /
    /// a normalize failure.
    pub fn snapshot_for(&self, study_id: Uuid) -> Result<StudySnapshot, String> {
        let study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        engine::build_snapshot(&study).map_err(|error| {
            tracing::warn!("normalize failed for study {study_id}: {error}");
            MSG_NORMALIZE_FAILED.to_string()
        })
    }

    /// Reopen a study by id with its **full** persisted state (FR2). `None` when absent or on a
    /// read error (logged).
    pub fn get_study(&self, id: Uuid) -> Option<Study> {
        let journal = self.journal.as_ref()?;
        match journal.get_study(id) {
            Ok(found) => found,
            Err(error) => {
                tracing::warn!("get_study({id}) failed: {error}");
                None
            }
        }
    }
}

/// Set one numeric judgment field by its Slint wire key (Story 2.6). Returns `false` for an unknown
/// key (the caller surfaces a neutral notice; never a panic). A `None` value clears the field —
/// **never `0`**. The `forecast_low_option` selector has its own rail ([`JournalState::
/// set_forecast_low_option`]) — it is not routed here.
pub(crate) fn apply_judgment_field(
    judgment: &mut Judgment,
    field: &str,
    value: Option<Money>,
) -> bool {
    match field {
        "sales_growth" => judgment.projected_sales_growth_pct = value,
        "eps_growth" => judgment.projected_eps_growth_pct = value,
        "est_high_eps" => judgment.estimated_high_eps = value,
        "est_low_eps" => judgment.estimated_low_eps = value,
        "high_pe" => judgment.judged_avg_high_pe = value,
        "low_pe" => judgment.judged_avg_low_pe = value,
        "recent_severe_low" => judgment.recent_severe_low = value,
        "current_price" => judgment.current_price = value,
        "dividend" => judgment.present_full_year_dividend = value,
        _ => return false,
    }
    true
}

/// A neutral RFC3339 timestamp rendered for the dashboard list: the date portion only (the time of
/// day is not meaningful in the v1 list). A non-RFC3339 string passes through unchanged — this is a
/// display transform, it never repairs a value.
pub fn created_at_date(ts: &Timestamp) -> String {
    ts.0.split('T').next().unwrap_or(&ts.0).to_string()
}

/// Map a persistence error from a watchlist write to a neutral notice (Story 4.1): a newer-schema
/// journal reads as read-only, anything else as the generic save-failure (cause appended).
fn watch_error(error: PersistError) -> String {
    match error {
        PersistError::NewerJournalSchema { .. } => MSG_READ_ONLY_WRITE.to_string(),
        other => format!("{MSG_SAVE_FAILED} {other}"),
    }
}

/// The display name of the single default portfolio (Story 4.3, FR36). Not user-editable in 4.3
/// (multi-portfolio naming is FR37/Epic 6).
const DEFAULT_PORTFOLIO_NAME: &str = "Portefeuille";

/// Validate a holding's quantity and purchase price (Story 4.3, FR36 + NFR-C1). Both must parse as
/// **exact** decimals (`Decimal::from_str_exact` — errors instead of silently rounding); quantity
/// must be strictly positive and price non-negative. On success returns their **canonical** decimal
/// spellings to store as TEXT; on any failure, the neutral [`MSG_HOLDING_INVALID_NUMBER`].
fn validate_holding_amounts(
    quantity: &str,
    purchase_price: &str,
) -> Result<(String, String), String> {
    let qty = Decimal::from_str_exact(quantity.trim())
        .ok()
        .filter(|q| q.is_sign_positive() && !q.is_zero())
        .ok_or(MSG_HOLDING_INVALID_NUMBER.to_string())?;
    let price = Decimal::from_str_exact(purchase_price.trim())
        .ok()
        .filter(|p| !p.is_sign_negative())
        .ok_or(MSG_HOLDING_INVALID_NUMBER.to_string())?;
    Ok((qty.to_string(), price.to_string()))
}

// ── Provider fetch/refresh cell helpers (Story 3.1 / 3.3) ───────────────────────────────────────

/// The outcome of an [`JournalState::apply_provider_refresh`] (Story 3.3): how many cells were
/// **updated** (a present provider/derived value changed) vs **filled** (a former gap), and the
/// classified [`RefreshCause`] of the recompute (price / input / FX). `updated + filled == 0` means
/// an idempotent no-op (the study was already current). Merged across years.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RefreshReport {
    pub updated: usize,
    pub filled: usize,
    /// Manual cells whose divergent provider value was preserved alongside (Story 3.4).
    pub reconciled: usize,
    /// Cells this refresh reset `✓ → ?` — the re-validation scope of an annual update (Story 3.6).
    pub revalidate: usize,
    pub cause: RefreshCause,
}

impl RefreshReport {
    /// Accumulate another year's report into this one (sum counts, OR-merge the cause).
    fn merge(self, other: RefreshReport) -> RefreshReport {
        RefreshReport {
            updated: self.updated + other.updated,
            filled: self.filled + other.filled,
            reconciled: self.reconciled + other.reconciled,
            revalidate: self.revalidate + other.revalidate,
            cause: self.cause.merge(other.cause),
        }
    }

    /// Whether the refresh changed anything (filled a gap, updated a provider value, or reconciled a
    /// divergent manual cell — any of which can move the verdict).
    pub fn changed(self) -> bool {
        self.updated + self.filled + self.reconciled > 0
    }
}

/// Mutable refs to every present cell of a year — the 4 load-bearing cells plus any present optional
/// cell. The shared walk for the freshness rails (Story 3.5: flag/clear `Freshness::Stale`).
fn year_cells_mut(year: &mut YearData) -> Vec<&mut Cell> {
    let mut cells: Vec<&mut Cell> = vec![
        &mut year.sales,
        &mut year.eps,
        &mut year.high_price,
        &mut year.low_price,
    ];
    for slot in [
        &mut year.dividend_per_share,
        &mut year.pre_tax_profit,
        &mut year.book_value_per_share,
    ] {
        if let Some(cell) = slot.as_mut() {
            cells.push(cell);
        }
    }
    cells
}

/// How many provider cells of `study` are not yet `Stale` — the [`JournalState::mark_provider_stale`]
/// pre-check (a `&Study` read, no mutation), so a no-op failure writes no journal revision.
fn count_provider_to_stale(study: &Study) -> usize {
    study
        .years
        .iter()
        .flat_map(|y| {
            let req = [&y.sales, &y.eps, &y.high_price, &y.low_price];
            let opt = [
                y.dividend_per_share.as_ref(),
                y.pre_tax_profit.as_ref(),
                y.book_value_per_share.as_ref(),
            ];
            req.into_iter()
                .map(Some)
                .chain(opt)
                .flatten()
                .collect::<Vec<_>>()
        })
        .filter(|c| c.source == Source::Provider && c.freshness != Freshness::Stale)
        .count()
}

/// A provider-sourced cell: `Source::Provider`, `Freshness::Current`, `Review::None` (unvalidated),
/// `Coverage::Present` for a value / `ToFill` for a gap (absent stays hand-editable, never `0`).
fn provider_cell(value: Option<Decimal>, provenance: &Provenance) -> Cell {
    Cell {
        value: value.map(Money::from),
        source: Source::Provider,
        freshness: Freshness::Current,
        review: Review::None,
        coverage: if value.is_some() {
            Coverage::Present
        } else {
            Coverage::ToFill
        },
        provenance: provenance.clone(),
        // A fresh provider cell carries no pending divergence (it IS the provider value).
        pending: None,
    }
}

/// Build an empty (all to-fill) provider year row — the fresh-study seed the [`refresh_cell`] rail
/// then fills, so one accounting path covers both the first fetch and a later refresh.
fn empty_provider_year(year: i32, provenance: &Provenance) -> YearData {
    YearData {
        year,
        sales: provider_cell(None, provenance),
        eps: provider_cell(None, provenance),
        high_price: provider_cell(None, provenance),
        low_price: provider_cell(None, provenance),
        dividend_per_share: None,
        pre_tax_profit: None,
        book_value_per_share: None,
    }
}

/// What a single cell's refresh did — drives the per-year tally + cause classification.
enum CellRefresh {
    /// A cell left untouched (a `NotAvailableAccepted` decision — never refilled or reconciled).
    Skipped,
    /// No change (an equal re-fetch, or the provider has no value for this cell).
    Unchanged,
    /// A former gap was filled from the provider.
    Filled,
    /// A present provider/derived value changed and was re-stamped.
    Updated,
    /// A present **manual** value diverged from the provider: the manual value stands, the divergent
    /// provider value is preserved alongside (pending), and a `✓` demoted (Story 3.4, FR22).
    Reconciled,
}

/// Refresh one **required** load-bearing cell. Returns `(outcome, demoted)` where `demoted` is `true`
/// iff this refresh reset the cell's `Review::Validated → ToReview` (Story 3.6: the count of cells the
/// user must re-verify after an annual update). The demotion itself is the existing
/// `Cell::edited`/`reconcile` rule — this wrapper only observes the `✓ → ?` transition around the
/// in-place mutation done by [`refresh_cell_inner`].
fn refresh_cell(
    cell: &mut Cell,
    value: Option<Decimal>,
    provenance: &Provenance,
) -> (CellRefresh, bool) {
    let was_validated = cell.review == Review::Validated;
    let outcome = refresh_cell_inner(cell, value, provenance);
    let demoted = was_validated && cell.review == Review::ToReview;
    (outcome, demoted)
}

/// The branching that actually mutates the cell (Story 3.3):
/// - empty (gap) → fill from the provider, whatever the skeleton source;
/// - present + `Source::Manual` → reconcile (manual wins, divergent provider value preserved, 3.4);
/// - present + provider/derived → re-stamp via [`Cell::edited`] **only when the value changed** (a
///   divergent value auto-demotes a `✓` and is `Current`; an equal value is a true no-op). A
///   provider that returns no value for an existing cell keeps the last-known value (FR23 spirit).
fn refresh_cell_inner(
    cell: &mut Cell,
    value: Option<Decimal>,
    provenance: &Provenance,
) -> CellRefresh {
    // A deliberate "not available" decision (FR19) is a user gesture, NOT a gap — never refilled by a
    // refresh (it would silently flip the accepted-blank back to a provider value). Checked before the
    // empty-cell gap-fill, because an N/A-accepted cell also carries `value: None`.
    if cell.coverage == Coverage::NotAvailableAccepted {
        CellRefresh::Skipped
    } else if cell.value.is_none() {
        match value {
            Some(v) => {
                *cell = provider_cell(Some(v), provenance);
                CellRefresh::Filled
            }
            None => CellRefresh::Unchanged,
        }
    } else if cell.source == Source::Manual {
        // Non-destructive reconciliation (Story 3.4, FR22/NFR-R4): the manual value wins and is
        // never overwritten. A divergent provider value is preserved ALONGSIDE (pending) and demotes
        // a `✓`; an agreeing fetch clears any stale pending. A provider with no value is no contradiction.
        match value {
            Some(v) => {
                let reconciled = cell.reconcile(Some(Money::from(v)), provenance.clone());
                if reconciled == *cell {
                    CellRefresh::Unchanged
                } else {
                    *cell = reconciled;
                    CellRefresh::Reconciled
                }
            }
            None => CellRefresh::Unchanged,
        }
    } else {
        match value {
            Some(v) => {
                let new_value = Some(Money::from(v));
                if cell.value == new_value {
                    // The value agrees → a true no-op. (Any `Stale` flag from a prior failed refresh
                    // was already cleared up front by `apply_provider_refresh`'s outage-recovery pass,
                    // Story 3.5 — so this stays a pure value-based idempotency check.)
                    CellRefresh::Unchanged
                } else {
                    *cell = cell.edited(new_value, provenance.clone());
                    CellRefresh::Updated
                }
            }
            // The provider has no value now → retain the last-known value (never blank it).
            None => CellRefresh::Unchanged,
        }
    }
}

/// Refresh one **optional** cell slot (same semantics as [`refresh_cell`]; an absent slot is a gap).
/// Any present slot — including a value-less `ToFill` or `NotAvailableAccepted` cell — delegates to
/// [`refresh_cell`] so the N/A-accepted skip and the manual-skip rules apply uniformly; only a truly
/// absent (`None`) slot is filled directly. Returns `(outcome, demoted)` like [`refresh_cell`].
fn refresh_optional(
    slot: &mut Option<Cell>,
    value: Option<Decimal>,
    provenance: &Provenance,
) -> (CellRefresh, bool) {
    match slot {
        Some(cell) => refresh_cell(cell, value, provenance),
        None => match value {
            Some(v) => {
                *slot = Some(provider_cell(Some(v), provenance));
                (CellRefresh::Filled, false)
            }
            None => (CellRefresh::Unchanged, false),
        },
    }
}

/// Refresh every cell of one matching year, tallying updated/filled counts and OR-merging the
/// recompute cause from each cell that actually changed (a fill counts toward the cause too — it
/// feeds the recompute). Field names drive [`refresh::classify_field`] (no parallel list).
fn refresh_year(yd: &mut YearData, cy: &CanonicalYear, provenance: &Provenance) -> RefreshReport {
    let mut report = RefreshReport::default();
    let mut account = |(outcome, demoted): (CellRefresh, bool), field: &str| {
        // Story 3.6: a cell this refresh reset `✓ → ?` is one the user must re-verify after the
        // annual update — the re-validation scope, independent of the value-change tally below.
        if demoted {
            report.revalidate += 1;
        }
        match outcome {
            CellRefresh::Updated => {
                report.updated += 1;
                report.cause = report
                    .cause
                    .merge(crate::viewmodel::refresh::classify_field(field));
            }
            CellRefresh::Filled => {
                report.filled += 1;
                report.cause = report
                    .cause
                    .merge(crate::viewmodel::refresh::classify_field(field));
            }
            CellRefresh::Reconciled => {
                report.reconciled += 1;
                // A reconciled divergence can degrade the verdict (a demoted load-bearing ✓) — feed
                // the cause so the recompute notice names what moved.
                report.cause = report
                    .cause
                    .merge(crate::viewmodel::refresh::classify_field(field));
            }
            CellRefresh::Skipped | CellRefresh::Unchanged => {}
        }
    };
    // Required load-bearing cells (disjoint &mut borrows of distinct struct fields).
    account(refresh_cell(&mut yd.sales, cy.sales, provenance), "sales");
    account(refresh_cell(&mut yd.eps, cy.eps, provenance), "eps");
    account(
        refresh_cell(&mut yd.high_price, cy.high_price, provenance),
        "high_price",
    );
    account(
        refresh_cell(&mut yd.low_price, cy.low_price, provenance),
        "low_price",
    );
    // Optional cells.
    account(
        refresh_optional(
            &mut yd.dividend_per_share,
            cy.dividend_per_share,
            provenance,
        ),
        "dividend_per_share",
    );
    account(
        refresh_optional(&mut yd.pre_tax_profit, cy.pre_tax_profit, provenance),
        "pre_tax_profit",
    );
    account(
        refresh_optional(
            &mut yd.book_value_per_share,
            cy.book_value_per_share,
            provenance,
        ),
        "book_value_per_share",
    );
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{FixedClock, FixedIdGen};
    use tempfile::TempDir;

    fn fixed(id: u128, ts: &str) -> (Box<dyn Clock>, Box<dyn IdGen>) {
        (
            Box::new(FixedClock(Timestamp(ts.to_string()))),
            Box::new(FixedIdGen(Uuid::from_u128(id))),
        )
    }

    // ── Story 2.9 — undo/redo history ──

    /// Open a fresh temp journal + state (creating the file on first use), with injected clock/id.
    fn undo_state(dir: &TempDir, seed: u128, ts: &str) -> JournalState {
        let path = dir.path().join("journal.db");
        if !path.exists() {
            drop(
                Journal::create(
                    &path,
                    Uuid::from_u128(0xC0FFEE),
                    &Timestamp("2026-06-14T00:00:00Z".to_string()),
                )
                .unwrap(),
            );
        }
        let (clock, idgen) = fixed(seed, ts);
        let (state, _) = JournalState::open_or_create(Some(&path), clock, idgen);
        state
    }

    /// Like [`undo_state`] but with a **sequential** id source — for tests that create several
    /// entities (Story 4.1 watchlist: each `add_watch_item` needs a distinct id).
    fn watch_state(dir: &TempDir, seed: u128) -> JournalState {
        let path = dir.path().join("journal.db");
        if !path.exists() {
            drop(
                Journal::create(
                    &path,
                    Uuid::from_u128(0xC0FFEE),
                    &Timestamp("2026-06-14T00:00:00Z".to_string()),
                )
                .unwrap(),
            );
        }
        let clock: Box<dyn Clock> =
            Box::new(FixedClock(Timestamp("2026-06-27T15:00:00Z".to_string())));
        let idgen: Box<dyn IdGen> = Box::new(crate::clock::SeqIdGen::starting_at(seed));
        let (state, _) = JournalState::open_or_create(Some(&path), clock, idgen);
        state
    }

    fn und_money(v: i64) -> Money {
        Money::from(rust_decimal::Decimal::new(v, 0))
    }

    // ── Story 3.1 — provider fetch pipeline (FakeProvider-style, offline) ──

    /// A normalized provider result covering the given fiscal years, every load-bearing field present.
    fn fetched_for(years: &[i32]) -> FetchedFinancials {
        fetched_custom(years, 1000, 5, 100, 50, "deadbeefcafe")
    }

    /// A normalized provider result with caller-chosen load-bearing values + digest — for refresh
    /// divergence/idempotency tests (Story 3.3).
    fn fetched_custom(
        years: &[i32],
        sales: i64,
        eps: i64,
        high: i64,
        low: i64,
        digest: &str,
    ) -> FetchedFinancials {
        use steadyinvest_core::normalize::{normalize, RawAmount, RawFinancials, RawYear};
        let amt = |v: i64| {
            Some(RawAmount {
                value: rust_decimal::Decimal::new(v, 0),
                currency: "CHF".to_string(),
            })
        };
        let rows = years
            .iter()
            .map(|&y| RawYear {
                sales: amt(sales),
                eps: amt(eps),
                high_price: amt(high),
                low_price: amt(low),
                ..RawYear::empty(y)
            })
            .collect();
        let raw = RawFinancials {
            native_currency: "CHF".to_string(),
            years: rows,
            splits: vec![],
        };
        FetchedFinancials {
            canonical: normalize(raw).expect("the test raw normalizes"),
            digest: digest.to_string(),
            latest_price: None,
        }
    }

    /// A normalized provider result that also carries a latest `/eod` close (Story 4.4) — drives the
    /// §4 zone recompute deterministically in `apply_provider_refresh` tests.
    fn fetched_with_price(years: &[i32], latest_price: i64) -> FetchedFinancials {
        FetchedFinancials {
            latest_price: Some(rust_decimal::Decimal::new(latest_price, 0)),
            ..fetched_for(years)
        }
    }

    #[test]
    fn provider_fetch_fills_a_fresh_study_with_provider_stamped_cells() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x3F, "2026-06-15T10:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();

        let fetched = fetched_for(&[2020, 2021, 2022, 2023, 2024]);
        let report = state.apply_provider_refresh(id, &fetched).unwrap();
        assert_eq!(
            report.filled, 20,
            "5 years × 4 load-bearing cells were filled"
        );
        assert_eq!(report.updated, 0, "a first fetch fills, it does not update");
        assert!(
            report.cause.price && report.cause.input,
            "filling prices + fundamentals classifies as both"
        );

        let study = state.get_study(id).unwrap();
        assert_eq!(study.years.len(), 5);
        let sales = &study.years[0].sales;
        assert_eq!(sales.source, Source::Provider);
        assert_eq!(
            sales.review,
            Review::None,
            "fresh provider data is unvalidated"
        );
        assert_eq!(sales.freshness, Freshness::Current);
        assert_eq!(sales.coverage, Coverage::Present);
        assert_eq!(sales.provenance.source, Source::Provider);
        assert_eq!(
            sales.provenance.hash_of_dependencies, "deadbeefcafe",
            "the real fetch digest replaces the manual placeholder (#21)"
        );
    }

    // ── Story 5.2 — export / import a single study ──

    #[test]
    fn export_import_round_trips_an_equal_study_preserving_identity() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x520);
        let id = state.create_study("NESN", "CHF").unwrap();
        let original = state.get_study(id).expect("the study exists");

        let envelope = state.export_study(id).expect("export succeeds");
        state.delete_study(id).expect("delete the study");
        assert!(
            state.get_study(id).is_none(),
            "the study is gone before import"
        );

        let (imported_id, overwrote) = state.import_study(&envelope).expect("import succeeds");
        assert_eq!(imported_id, id, "the study id is preserved on round-trip");
        assert!(
            !overwrote,
            "a fresh import (the study was deleted) is not an overwrite"
        );
        assert_eq!(
            state.get_study(id).expect("the study is back"),
            original,
            "export → import yields an equal study"
        );

        // A second import of the same envelope is an idempotent update, surfaced as an overwrite.
        let (_id, overwrote_again) = state
            .import_study(&envelope)
            .expect("re-import updates in place");
        assert!(
            overwrote_again,
            "re-import onto an existing id is surfaced as an overwrite"
        );
        assert_eq!(state.list_studies().len(), 1, "no duplicate study");
    }

    #[test]
    fn importing_onto_an_archived_study_reactivates_it() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x523);
        let id = state.create_study("NESN", "CHF").unwrap();
        let envelope = state.export_study(id).unwrap();
        state.archive_study(id).expect("archive the study");
        assert_eq!(
            state.list_studies()[0].status,
            "archived",
            "the study is hidden before re-import"
        );

        let (_id, overwrote) = state.import_study(&envelope).expect("re-import succeeds");
        assert!(overwrote, "re-import onto the archived id is an overwrite");
        assert_eq!(
            state.list_studies()[0].status,
            "active",
            "an imported study is reactivated, never left silently hidden"
        );
    }

    #[test]
    fn export_of_a_missing_study_is_a_neutral_refusal() {
        let dir = TempDir::new().unwrap();
        let state = watch_state(&dir, 0x521);
        assert_eq!(
            state.export_study(Uuid::from_u128(0xDEAD)),
            Err(MSG_EXPORT_MISSING.to_string())
        );
    }

    #[test]
    fn import_maps_each_rejection_to_its_neutral_notice_and_writes_nothing() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x522);
        let id = state.create_study("NESN", "CHF").unwrap();
        let good = state.export_study(id).unwrap();

        // Tamper → integrity refusal.
        let tampered = good.replacen("NESN", "ROG0", 1);
        assert_eq!(
            state.import_study(&tampered),
            Err(MSG_IMPORT_INTEGRITY.to_string())
        );
        // Garbage → malformed refusal.
        assert_eq!(
            state.import_study("not an envelope"),
            Err(MSG_IMPORT_MALFORMED.to_string())
        );
        assert_eq!(
            state.list_studies().len(),
            1,
            "a rejected import wrote nothing"
        );
    }

    // ── Story 5.3 — export / import the whole journal ──

    #[test]
    fn journal_export_import_round_trips_into_a_fresh_journal() {
        // Populate journal A with a study, a linked watchlist row and a holding.
        let dir_a = TempDir::new().unwrap();
        let mut state_a = watch_state(&dir_a, 0x530);
        let study_id = state_a.create_study("NESN", "CHF").unwrap();
        state_a.add_watch_item("NESN", Some(study_id)).unwrap();
        state_a.add_holding("NESN", "10", "100.00").unwrap();
        let envelope = state_a.export_journal().expect("export succeeds");

        // Import into a fresh, empty journal B (a different dir → a different journal_id).
        let dir_b = TempDir::new().unwrap();
        let mut state_b = watch_state(&dir_b, 0x531);
        assert!(state_b.list_studies().is_empty(), "B starts empty");
        let summary = state_b.import_journal(&envelope).expect("import succeeds");

        assert_eq!(summary.studies, 1);
        assert_eq!(summary.watch_items, 1);
        assert_eq!(summary.holdings, 1);
        assert_eq!(state_b.list_studies().len(), 1, "the study landed in B");
        assert_eq!(state_b.list_watch_items().len(), 1, "the watch row landed");
        assert_eq!(state_b.list_holdings().len(), 1, "the holding landed");
        // The study's journal_id is rebound to B (seed semantics), id preserved.
        assert!(state_b.get_study(study_id).is_some(), "study id preserved");
    }

    #[test]
    fn journal_import_maps_each_rejection_to_its_neutral_notice_and_writes_nothing() {
        let dir_a = TempDir::new().unwrap();
        let mut state_a = watch_state(&dir_a, 0x532);
        state_a.create_study("NESN", "CHF").unwrap();
        let good = state_a.export_journal().unwrap();

        let dir_b = TempDir::new().unwrap();
        let mut state_b = watch_state(&dir_b, 0x533);

        let tampered = good.replacen("NESN", "ROG0", 1);
        assert_eq!(
            state_b.import_journal(&tampered),
            Err(MSG_IMPORT_INTEGRITY.to_string())
        );
        assert_eq!(
            state_b.import_journal("not an envelope"),
            Err(MSG_IMPORT_MALFORMED.to_string())
        );
        assert!(
            state_b.list_studies().is_empty(),
            "a rejected whole-journal import wrote nothing"
        );
    }

    #[test]
    fn journal_imported_message_fills_the_counts() {
        let summary = ImportSummary {
            source_journal_id: Uuid::from_u128(1),
            source_logical_version: 7,
            studies: 3,
            watch_items: 2,
            portfolios: 1,
            holdings: 5,
            transactions: 4,
        };
        let msg = journal_imported_message(&summary);
        assert!(msg.contains("3 étude"));
        assert!(msg.contains("2 valeur"));
        assert!(msg.contains("5 ligne"));
        assert!(msg.contains("4 mouvement"));
    }

    // ── Story 5.4 — restore from backup ──

    /// Create a standalone backup journal at `dir/<name>` with a chosen identity + an optional study,
    /// then drop the handle (so it is a static file to inspect/restore).
    fn make_backup(dir: &TempDir, name: &str, jid: u128, with_study: bool) {
        let path = dir.path().join(name);
        let mut j = Journal::create(
            &path,
            Uuid::from_u128(jid),
            &Timestamp("2026-06-20T00:00:00Z".to_string()),
        )
        .unwrap();
        if with_study {
            let s = Study::new(
                Uuid::from_u128(0xDA7A),
                Uuid::from_u128(jid),
                "ROG",
                "CHF",
                empty_judgment(),
                Timestamp("2026-06-20T00:00:00Z".to_string()),
            );
            j.put_study(&s).unwrap();
        }
        drop(j);
    }

    #[test]
    fn request_restore_classifies_a_foreign_backup_and_parks_it() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x540); // live journal_id = 0xC0FFEE
        make_backup(&dir, "foreign.db", 0xBEEF, true);
        let assessment = state
            .request_restore(dir.path().join("foreign.db").to_str().unwrap())
            .unwrap();
        assert_eq!(assessment.verdict, RestoreVerdict::ForeignJournal);
        assert_eq!(assessment.journal_id, Uuid::from_u128(0xBEEF));
        assert!(
            state.has_pending_restore(),
            "a confirmable restore is parked"
        );
    }

    #[test]
    fn request_restore_flags_an_older_same_journal_backup_as_stale() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x541);
        // Advance the live journal so it is newer than a fresh same-id backup (version 0).
        state.create_study("NESN", "CHF").unwrap();
        make_backup(&dir, "old.db", 0xC0FFEE, false); // same id, version 0
        let assessment = state
            .request_restore(dir.path().join("old.db").to_str().unwrap())
            .unwrap();
        assert!(
            matches!(assessment.verdict, RestoreVerdict::StaleOlder { backup: 0, current } if current >= 1),
            "an older same-journal backup is StaleOlder, got {:?}",
            assessment.verdict
        );
        // The confirm prompt surfaces the identity + the stale warning.
        let prompt = restore_confirm_message(&assessment);
        assert!(prompt.contains("plus ancienne"));
    }

    #[test]
    fn request_restore_refuses_a_non_journal_file_and_parks_nothing() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x542);
        let garbage = dir.path().join("notjournal.txt");
        std::fs::write(&garbage, b"definitely not a sqlite journal").unwrap();
        let result = state.request_restore(garbage.to_str().unwrap());
        assert!(result.is_err(), "a non-journal file is refused");
        assert!(
            !state.has_pending_restore(),
            "a hard refusal parks no pending restore (confirm cannot fire)"
        );
    }

    #[test]
    fn confirm_restore_swaps_the_live_journal_then_cancel_clears() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x543); // live id 0xC0FFEE, empty
        assert!(state.list_studies().is_empty());
        make_backup(&dir, "src.db", 0xBEEF, true); // foreign backup carrying one study

        // Cancel path: park then cancel → nothing applied.
        state
            .request_restore(dir.path().join("src.db").to_str().unwrap())
            .unwrap();
        state.cancel_restore();
        assert!(!state.has_pending_restore());
        assert!(state.list_studies().is_empty(), "cancel applied nothing");

        // Confirm path: park then confirm → the live journal becomes the backup.
        state
            .request_restore(dir.path().join("src.db").to_str().unwrap())
            .unwrap();
        state.confirm_restore().unwrap();
        assert_eq!(
            state.journal_id(),
            Some(Uuid::from_u128(0xBEEF)),
            "the live journal is now the restored backup"
        );
        assert_eq!(
            state.list_studies().len(),
            1,
            "the backup's study is now live"
        );
        assert!(!state.has_pending_restore(), "pending cleared");
    }

    #[test]
    fn restoring_the_journal_onto_itself_is_a_safe_no_op() {
        // Review CRITICAL: fs::copy(live, live) truncates to 0 bytes — the same-path guard must make a
        // self-restore a no-op that loses nothing.
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x544);
        let id = state.create_study("NESN", "CHF").unwrap();
        let live_path = dir.path().join("journal.db");
        state.request_restore(live_path.to_str().unwrap()).unwrap();
        state.confirm_restore().unwrap();
        assert_eq!(state.journal_id(), Some(Uuid::from_u128(0xC0FFEE)));
        assert!(state.get_study(id).is_some(), "the study was not zeroed");
    }

    #[test]
    fn confirm_re_validates_and_refuses_a_tampered_backup_without_touching_the_journal() {
        // Review HIGH (TOCTOU): a backup validated at request time but replaced before confirm must be
        // re-checked — and a now-garbage file refused without overwriting the live journal.
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x545); // live id 0xC0FFEE, empty
        let backup = dir.path().join("src.db");
        make_backup(&dir, "src.db", 0xBEEF, true);
        state.request_restore(backup.to_str().unwrap()).unwrap(); // ForeignJournal, parked
        std::fs::write(&backup, b"no longer a journal").unwrap(); // tamper after validation
        let result = state.confirm_restore();
        assert!(
            result.is_err(),
            "the re-validation refuses the tampered file"
        );
        assert_eq!(
            state.journal_id(),
            Some(Uuid::from_u128(0xC0FFEE)),
            "the live journal was not overwritten"
        );
        assert!(state.list_studies().is_empty(), "nothing was applied");
    }

    // ── Story 5.5 — journal location, recent journals & sync-safety ──

    #[test]
    fn is_sync_folder_matches_known_providers_and_rejects_a_plain_path() {
        assert!(is_sync_folder(Path::new(
            "/home/g/SynologyDrive/journal.db"
        )));
        assert!(is_sync_folder(Path::new("/home/g/Dropbox/sub/journal.db")));
        assert!(is_sync_folder(Path::new("/home/g/OneDrive/journal.db")));
        assert!(is_sync_folder(Path::new(
            "/Users/g/Library/Mobile Documents/journal.db"
        )));
        assert!(!is_sync_folder(Path::new(
            "/home/g/.local/share/steadyinvest/journal.db"
        )));
    }

    #[test]
    fn open_and_create_journal_switch_between_journals() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x550); // journal A at dir/journal.db (id 0xC0FFEE)
        let id_in_a = state.create_study("NESN", "CHF").unwrap();

        // Create a second journal in a subdir → switches to it (empty).
        let sub = dir.path().join("other");
        std::fs::create_dir_all(&sub).unwrap();
        let outcome = state.create_journal(&sub, "second").unwrap();
        assert!(
            state.list_studies().is_empty(),
            "the new journal B is empty"
        );
        assert_eq!(state.journal_id(), Some(outcome.journal_id));
        assert!(
            !outcome.sync_warning,
            "a plain temp dir is not a sync folder"
        );

        // Open journal A back → its study is there (a clean switch round-trip).
        let path_a = dir.path().join("journal.db");
        state.open_journal(&path_a).unwrap();
        assert!(
            state.get_study(id_in_a).is_some(),
            "switched back to journal A"
        );
    }

    #[test]
    fn open_journal_failure_leaves_the_previous_journal_open() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x551);
        state.create_study("NESN", "CHF").unwrap();

        // A journal held by a foreign, live process (forged lock with PID 1 = init).
        let sub = dir.path().join("locked");
        std::fs::create_dir_all(&sub).unwrap();
        let locked = sub.join("j.db");
        drop(
            Journal::create(
                &locked,
                Uuid::from_u128(0xBEEF),
                &Timestamp("2026-06-20T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut lock = locked.as_os_str().to_os_string();
        lock.push("-lock");
        std::fs::write(&lock, "1").unwrap();

        let result = state.open_journal(&locked);
        assert_eq!(result, Err(MSG_JOURNAL_LOCKED.to_string()));
        // The previous journal stayed open with its study (never journal-less).
        assert_eq!(
            state.list_studies().len(),
            1,
            "the previous journal stayed open after a refused switch"
        );
    }

    #[test]
    fn journal_stale_message_surfaces_both_versions() {
        let msg = journal_stale_message(57, 41);
        assert!(msg.contains("57"));
        assert!(msg.contains("41"));
    }

    #[test]
    fn create_backup_lands_beside_the_journal() {
        // Review patch (5.4 deferral): backups follow the journal's location.
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x552); // journal at dir/journal.db
        state.create_study("NESN", "CHF").unwrap();
        let backup = state.create_backup().unwrap();
        assert_eq!(
            backup.parent().unwrap(),
            dir.path().join("backups"),
            "the backup sits in a backups/ folder beside the journal"
        );
        assert!(backup.exists());
    }

    #[test]
    fn reopening_the_currently_open_journal_is_a_no_op() {
        // Review patch (E7): re-selecting the open journal must not close+reopen (which would wipe undo).
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x553);
        let id = state.create_study("NESN", "CHF").unwrap();
        let path = dir.path().join("journal.db");
        let outcome = state.open_journal(&path).unwrap();
        assert_eq!(outcome.journal_id, Uuid::from_u128(0xC0FFEE));
        assert!(
            state.get_study(id).is_some(),
            "the journal stayed open, study intact"
        );
    }

    #[test]
    fn provider_data_is_unvalidated_so_the_verdict_is_not_full() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x41, "2026-06-15T10:30:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        state
            .apply_provider_refresh(id, &fetched_for(&[2020, 2021, 2022, 2023, 2024]))
            .unwrap();

        let study = state.get_study(id).unwrap();
        let snapshot = engine::build_snapshot(&study).expect("normalizes");
        assert!(
            !matches!(
                snapshot.verdict(),
                steadyinvest_core::verdict::Verdict::Full(_)
            ),
            "unvalidated (Review::None) provider cells can never yield a Full verdict"
        );
    }

    #[test]
    fn provider_fetch_does_not_overwrite_a_manual_value() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x42, "2026-06-15T11:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();

        // A manual edit on year 0, field "a" (high_price) — this also materializes the year grid.
        state.edit_cell(id, 0, "a", Some(und_money(999))).unwrap();
        let years: Vec<i32> = state
            .get_study(id)
            .unwrap()
            .years
            .iter()
            .map(|y| y.year)
            .collect();

        // Fetch covering those exact years; year-0 high_price is held manually, low_price is empty.
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();

        let study = state.get_study(id).unwrap();
        assert_eq!(
            study.years[0].high_price.value,
            Some(und_money(999)),
            "the manual value survives the fetch (fill-gaps-only)"
        );
        assert_eq!(study.years[0].high_price.source, Source::Manual);
        assert_eq!(
            study.years[0].low_price.source,
            Source::Provider,
            "the empty sibling cell was filled by the provider"
        );
    }

    // ── Story 3.3 — manual refresh: update / freshness / cause / idempotency ──

    /// A refresh re-stamps a present **provider** cell whose value changed (new value + provenance
    /// digest), and reports it as `updated` (not `filled`). (AC1/AC2)
    #[test]
    fn refresh_updates_a_changed_provider_cell() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x51, "2026-06-20T10:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];

        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        // Second refresh: high_price 100 → 200 (price diverges), everything else identical.
        let report = state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 200, 50, "feed0042"))
            .unwrap();

        assert_eq!(report.filled, 0, "no gaps remain to fill");
        assert_eq!(report.updated, 5, "one high_price per year changed");
        assert!(
            report.cause.price && !report.cause.input,
            "only a price moved → price cause only"
        );

        let high = &state.get_study(id).unwrap().years[0].high_price;
        assert_eq!(high.value, Some(und_money(200)), "the new value is stamped");
        assert_eq!(high.source, Source::Provider);
        assert_eq!(
            high.provenance.hash_of_dependencies, "feed0042",
            "the cell carries the new fetch digest (re-stamped)"
        );
    }

    /// An identical re-fetch is a true no-op: nothing changes, the cause is empty, and **no phantom
    /// undo step** is recorded (the timestamp-churn trap). (AC1 idempotency)
    #[test]
    fn idempotent_refresh_changes_nothing_and_records_no_undo_step() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x52, "2026-06-20T11:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];

        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        let depth_after_fill = state.undo_depth();
        assert_eq!(depth_after_fill, 1, "the first fill is one undo step");

        // Re-run the SAME refresh (same values, same digest) — must be a no-op.
        let report = state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        assert!(!report.changed(), "an identical re-fetch changes nothing");
        assert!(!report.cause.price && !report.cause.input);
        assert_eq!(
            state.undo_depth(),
            depth_after_fill,
            "a no-op refresh records no phantom undo step"
        );
    }

    /// A present **manual** cell is never overwritten by a refresh — even a divergent one (manual
    /// wins; the divergent dual-value case is Story 3.4). (AC2)
    #[test]
    fn refresh_skips_a_manual_cell_even_when_divergent() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x53, "2026-06-20T12:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();

        // Manual high_price on year 0 (also materializes the grid).
        state.edit_cell(id, 0, "a", Some(und_money(999))).unwrap();
        let years: Vec<i32> = state
            .get_study(id)
            .unwrap()
            .years
            .iter()
            .map(|y| y.year)
            .collect();

        // Refresh with a DIVERGENT high_price (100 ≠ 999) for those years.
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();

        let high = &state.get_study(id).unwrap().years[0].high_price;
        assert_eq!(
            high.value,
            Some(und_money(999)),
            "the manual value stands; the divergent fetch never overwrites it"
        );
        assert_eq!(high.source, Source::Manual);
    }

    /// A deliberate "not available" decision (FR19) is never refilled by a refresh — neither a
    /// load-bearing cell nor an optional one (it carries `value: None` but is a user choice, not a
    /// gap). Regression guard for the code-review HIGH finding. (AC2)
    #[test]
    fn refresh_never_refills_a_not_available_accepted_cell() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x57, "2026-06-20T16:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();

        // Mark a load-bearing cell (year-0 sales) AND an optional cell (year-0 dividend, "f")
        // as not-available-accepted (this also materializes the grid).
        state
            .set_not_available(id, 0, entry::FIELD_SALES, true)
            .unwrap();
        state
            .set_not_available(id, 0, entry::FIELD_DIVIDEND, true)
            .unwrap();
        let years: Vec<i32> = state
            .get_study(id)
            .unwrap()
            .years
            .iter()
            .map(|y| y.year)
            .collect();

        // A refresh that supplies values for those exact cells must NOT refill them.
        let report = state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();

        let y0 = &state.get_study(id).unwrap().years[0];
        assert_eq!(
            y0.sales.coverage,
            Coverage::NotAvailableAccepted,
            "an N/A-accepted load-bearing cell is preserved, never refilled"
        );
        assert_eq!(y0.sales.value, None);
        assert_eq!(y0.sales.source, Source::Manual);
        // `fetched_for` leaves dividend absent, but assert the optional N/A slot is preserved anyway.
        assert!(
            y0.dividend_per_share
                .as_ref()
                .is_some_and(|c| c.coverage == Coverage::NotAvailableAccepted),
            "an N/A-accepted optional cell is preserved too"
        );
        // The empty sibling (low_price) was still filled — only the N/A decisions are protected.
        assert_eq!(y0.low_price.source, Source::Provider);
        assert!(report.filled > 0, "ordinary gaps still fill");
    }

    /// A divergent refresh of a **validated** provider cell auto-demotes `✓ → ?` (FR20, AC3); a
    /// non-divergent re-fetch keeps the human `✓`.
    #[test]
    fn refresh_demotes_a_divergent_validated_provider_cell() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x54, "2026-06-20T13:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];

        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        // The user reviews & validates year-0 high_price (a provider cell).
        state.set_review(id, 0, "a", Review::Validated).unwrap();
        assert_eq!(
            state.get_study(id).unwrap().years[0].high_price.review,
            Review::Validated
        );

        // A non-divergent re-fetch (same 100) keeps the ✓.
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        assert_eq!(
            state.get_study(id).unwrap().years[0].high_price.review,
            Review::Validated,
            "an equal re-fetch keeps the human ✓"
        );

        // A divergent re-fetch (100 → 250) auto-demotes the ✓ to ?.
        state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 250, 50, "beadfeed"))
            .unwrap();
        let high = &state.get_study(id).unwrap().years[0].high_price;
        assert_eq!(
            high.review,
            Review::ToReview,
            "a divergent provider value auto-tags ✓ → ?"
        );
        assert_eq!(high.value, Some(und_money(250)));
    }

    /// The recompute cause distinguishes a pure-fundamental change from a pure-price change (FR29,
    /// AC5) — driven through the real `apply_provider_refresh`, not a hand-built diff.
    #[test]
    fn refresh_classifies_input_only_vs_price_only_cause() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x55, "2026-06-20T14:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];

        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();

        // Only EPS moves (5 → 6): an input cause, no price cause.
        let input_only = state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 6, 100, 50, "d1"))
            .unwrap();
        assert!(
            input_only.cause.input && !input_only.cause.price,
            "an EPS-only change is an input cause"
        );
        assert_eq!(refresh_notice(input_only), MSG_REFRESH_INPUT);

        // Only low_price moves (50 → 40): a price cause, no input cause.
        let price_only = state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 6, 100, 40, "d2"))
            .unwrap();
        assert!(
            price_only.cause.price && !price_only.cause.input,
            "a price-only change is a price cause"
        );
        assert_eq!(refresh_notice(price_only), MSG_REFRESH_PRICE);
    }

    /// End-to-end (the Story-3.3 invariant 2b through a REAL refresh, not a hand-set freshness):
    /// a fully-validated provider study reads `Full`; a divergent refresh of a load-bearing provider
    /// cell auto-demotes it and the verdict degrades to `Provisional` in the same frame. (AC3,
    /// complements `seam_check.rs` SEAM 3 which sets the flag by hand.)
    #[test]
    fn a_divergent_refresh_degrades_a_full_verdict_to_provisional() {
        use steadyinvest_core::verdict::Verdict;
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x56, "2026-06-20T15:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];

        // Fill from the provider, then the user validates every load-bearing year cell …
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        for y in 0..years.len() {
            for field in [
                entry::FIELD_SALES,
                entry::FIELD_HIGH,
                entry::FIELD_LOW,
                entry::FIELD_EPS,
            ] {
                state.set_review(id, y, field, Review::Validated).unwrap();
            }
        }
        // … and completes the judgment (the five load-bearing judgment inputs).
        for (field, v) in [
            ("est_high_eps", 8),
            ("est_low_eps", 6),
            ("high_pe", 20),
            ("low_pe", 10),
            ("current_price", 60),
        ] {
            state
                .set_judgment_field(id, field, Some(und_money(v)))
                .unwrap();
        }

        let study = state.get_study(id).unwrap();
        assert!(
            matches!(
                engine::build_snapshot(&study)
                    .expect("normalizes")
                    .verdict(),
                Verdict::Full(_)
            ),
            "an all-validated provider study with a complete judgment reads Full"
        );

        // A divergent refresh of the (validated, provider) high_price demotes ✓ → ? …
        state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 250, 50, "deg"))
            .unwrap();
        let study = state.get_study(id).unwrap();
        assert_eq!(
            study.years[0].high_price.review,
            Review::ToReview,
            "the divergent provider value auto-demotes the ✓"
        );
        assert!(
            matches!(
                engine::build_snapshot(&study)
                    .expect("normalizes")
                    .verdict(),
                Verdict::Provisional(_)
            ),
            "a demoted load-bearing input degrades Full → Provisional in the same frame"
        );
    }

    // ── Story 3.4 — non-destructive reconciliation ──

    /// Set up a study with a single manual, validated high_price cell that the provider will
    /// diverge from. Returns (state, id, years).
    fn study_with_validated_manual_high(
        dir: &TempDir,
        seed: u128,
        manual_high: i64,
    ) -> (JournalState, Uuid, Vec<i32>) {
        let mut state = undo_state(dir, seed, "2026-06-27T10:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        // Manual high_price on year 0 (materializes the grid), then validate it.
        state
            .edit_cell(id, 0, entry::FIELD_HIGH, Some(und_money(manual_high)))
            .unwrap();
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
        let years: Vec<i32> = state
            .get_study(id)
            .unwrap()
            .years
            .iter()
            .map(|y| y.year)
            .collect();
        (state, id, years)
    }

    /// A divergent refresh of a validated MANUAL cell: the manual value stands, the provider value is
    /// preserved alongside (pending), and the `✓` demotes to `?` — never merged. (AC1, AC2, AC3)
    #[test]
    fn refresh_reconciles_a_divergent_manual_cell_non_destructively() {
        let dir = TempDir::new().unwrap();
        let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x60, 999);

        // Provider diverges on high_price (100 ≠ 999).
        let report = state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "recon"))
            .unwrap();
        assert!(
            report.reconciled >= 1,
            "the manual divergence is reconciled"
        );
        assert!(report.changed(), "a reconciliation is a change");

        let high = &state.get_study(id).unwrap().years[0].high_price;
        assert_eq!(high.value, Some(und_money(999)), "manual value stands");
        assert_eq!(high.source, Source::Manual);
        assert_eq!(high.review, Review::ToReview, "the ✓ demotes on divergence");
        let pending = high
            .pending
            .as_ref()
            .expect("the provider value is preserved");
        assert_eq!(pending.value, Some(und_money(100)));
        assert_eq!(pending.provenance.source, Source::Provider);
    }

    /// An agreeing refresh on a manual cell records no pending and keeps `✓` — and an identical
    /// re-run is a no-op (idempotency, no phantom undo step). (AC1)
    #[test]
    fn refresh_agreement_on_a_manual_cell_keeps_validation_and_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x61, 100);

        // Provider AGREES with the manual high_price (100 == 100).
        state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "agree"))
            .unwrap();
        let high = &state.get_study(id).unwrap().years[0].high_price;
        assert_eq!(high.review, Review::Validated, "agreement keeps ✓");
        assert!(high.pending.is_none(), "no divergence → no pending");

        let depth = state.undo_depth();
        // Re-run the same agreeing refresh — a true no-op.
        let report = state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "agree"))
            .unwrap();
        assert_eq!(report.reconciled, 0);
        assert_eq!(
            state.undo_depth(),
            depth,
            "an agreeing re-refresh records no phantom undo step"
        );
    }

    /// Accept-provider resolution: the cell takes the pending provider value (Source::Provider,
    /// Review::ToReview, pending cleared). (AC4)
    #[test]
    fn accept_provider_value_takes_the_pending_and_clears_it() {
        let dir = TempDir::new().unwrap();
        let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x62, 999);
        state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "recon"))
            .unwrap();

        state
            .accept_provider_value(id, 0, entry::FIELD_HIGH)
            .unwrap();
        let high = &state.get_study(id).unwrap().years[0].high_price;
        assert_eq!(
            high.value,
            Some(und_money(100)),
            "the provider value is taken"
        );
        assert_eq!(high.source, Source::Provider);
        assert_eq!(high.review, Review::ToReview, "re-check the accepted value");
        assert!(high.pending.is_none(), "the pending is cleared");
    }

    /// Keep-manual resolution: the manual value stands, only the pending is dismissed. (AC4)
    #[test]
    fn keep_manual_value_dismisses_the_pending_only() {
        let dir = TempDir::new().unwrap();
        let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x63, 999);
        state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "recon"))
            .unwrap();

        state.keep_manual_value(id, 0, entry::FIELD_HIGH).unwrap();
        let high = &state.get_study(id).unwrap().years[0].high_price;
        assert_eq!(high.value, Some(und_money(999)), "manual value stands");
        assert_eq!(high.source, Source::Manual);
        assert!(high.pending.is_none(), "the pending is dismissed");
    }

    /// Re-validating a cell with a pending clears the pending (the user reconciled). (AC4)
    #[test]
    fn revalidating_a_cell_clears_its_pending() {
        let dir = TempDir::new().unwrap();
        let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x64, 999);
        state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "recon"))
            .unwrap();
        // The divergence demoted it to ?; the user re-validates their kept value.
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
        let high = &state.get_study(id).unwrap().years[0].high_price;
        assert_eq!(high.review, Review::Validated);
        assert!(high.pending.is_none(), "re-validating clears the pending");
    }

    /// AC6 guard: the engine ignores `pending` — a cell carrying a pending yields the SAME frame as
    /// the same cell with `pending = None`.
    #[test]
    fn the_engine_ignores_a_pending_divergence() {
        let dir = TempDir::new().unwrap();
        let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x65, 999);
        state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "recon"))
            .unwrap();
        let mut with_pending = state.get_study(id).unwrap();
        assert!(
            with_pending.years[0].high_price.pending.is_some(),
            "precondition: a pending exists"
        );
        let frame_with = engine::build_snapshot(&with_pending).expect("normalizes");

        // Strip the pending and rebuild — the verdict frame must be identical.
        with_pending.years[0].high_price.pending = None;
        let frame_without = engine::build_snapshot(&with_pending).expect("normalizes");
        assert_eq!(
            frame_with.verdict(),
            frame_without.verdict(),
            "the engine reads only the live value, never `pending`"
        );
    }

    /// A pending divergence survives a journal close + reopen (AC5 — NFR-R4 "preserved").
    #[test]
    fn a_pending_divergence_survives_reopen() {
        let dir = TempDir::new().unwrap();
        let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x66, 999);
        state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "recon"))
            .unwrap();
        drop(state);

        // Reopen the journal from disk and confirm the pending is intact.
        let reopened = open_state(&dir.path().join("journal.db"));
        let high = reopened.get_study(id).unwrap().years[0].high_price.clone();
        assert_eq!(
            high.value,
            Some(und_money(999)),
            "manual value survives reopen"
        );
        assert_eq!(high.review, Review::ToReview);
        let pending = high.pending.expect("the pending survives reopen");
        assert_eq!(pending.value, Some(und_money(100)));
    }

    /// accept/keep on a cell with NO pending is a true no-op — no undo step, no journal write
    /// (the resolve buttons can linger; re-clicking them must not churn the journal). (review fix)
    #[test]
    fn accept_or_keep_with_no_pending_is_a_true_noop() {
        let dir = TempDir::new().unwrap();
        let (mut state, id, _years) = study_with_validated_manual_high(&dir, 0x67, 999);
        let depth = state.undo_depth();
        let version = state.logical_version();

        state
            .accept_provider_value(id, 0, entry::FIELD_HIGH)
            .unwrap();
        state.keep_manual_value(id, 0, entry::FIELD_HIGH).unwrap();

        assert_eq!(state.undo_depth(), depth, "no pending → no undo step");
        assert_eq!(
            state.logical_version(),
            version,
            "no pending → no journal revision (no phantom logical_version bump)"
        );
    }

    /// A repeated DIVERGENT refresh (same provider value, a later fetch timestamp) is idempotent —
    /// the pending is not re-stamped, so no phantom undo step accrues. (review fix)
    #[test]
    fn a_repeated_divergent_refresh_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x68, 999);
        state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "fetch-a"))
            .unwrap();
        let depth = state.undo_depth();

        // Re-fetch the SAME divergent value with a DIFFERENT digest (a later fetch) — a no-op.
        let report = state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "fetch-b"))
            .unwrap();
        assert_eq!(
            report.reconciled, 0,
            "the same divergence is not re-reconciled"
        );
        assert_eq!(
            state.undo_depth(),
            depth,
            "a repeated divergence records no phantom undo step"
        );
    }

    // ── Story 3.5 — graceful provider failure ──

    #[test]
    fn provider_failure_notice_maps_each_cause() {
        use steadyinvest_ingestion::{IngestionError, ProviderError};
        let p = |e: ProviderError| provider_failure_notice(&IngestionError::Provider(e));
        assert_eq!(
            p(ProviderError::Network {
                detail: "dns".into()
            }),
            MSG_PROVIDER_OFFLINE
        );
        assert_eq!(
            p(ProviderError::Quota {
                retry_after_secs: Some(60)
            }),
            MSG_PROVIDER_QUOTA
        );
        assert_eq!(p(ProviderError::InvalidOrAbsentKey), MSG_KEY_INVALID);
        assert_eq!(
            p(ProviderError::Forbidden {
                detail: "plan".into()
            }),
            MSG_KEY_FORBIDDEN
        );
        assert_eq!(
            p(ProviderError::TickerNotFound {
                ticker: "AAPL.US".into()
            }),
            MSG_PROVIDER_NO_DATA
        );
        assert_eq!(
            p(ProviderError::Parse {
                detail: "shape".into()
            }),
            MSG_NORMALIZE_FAILED
        );
        let normalize = IngestionError::Normalize(
            steadyinvest_core::normalize::NormalizeError::DuplicateYear { year: 2020 },
        );
        assert_eq!(provider_failure_notice(&normalize), MSG_NORMALIZE_FAILED);
    }

    #[test]
    fn mark_provider_stale_flags_provider_cells_and_retains_values() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x70, "2026-06-27T10:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();

        let flagged = state.mark_provider_stale(id).unwrap();
        assert_eq!(
            flagged, 20,
            "5 years × 4 load-bearing provider cells flagged"
        );
        let high = &state.get_study(id).unwrap().years[0].high_price;
        assert_eq!(high.freshness, Freshness::Stale);
        assert_eq!(
            high.value,
            Some(und_money(100)),
            "the last-known value is retained (NFR-R1)"
        );
        assert_eq!(high.source, Source::Provider);
    }

    #[test]
    fn mark_provider_stale_leaves_manual_cells_current_and_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x71, "2026-06-27T10:30:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        // A manual high_price on year 0 (materializes the grid); the rest are empty (manual to-fill).
        state.edit_cell(id, 0, "a", Some(und_money(999))).unwrap();

        let flagged = state.mark_provider_stale(id).unwrap();
        assert_eq!(flagged, 0, "a study with no provider cells flags nothing");
        assert_eq!(
            state.get_study(id).unwrap().years[0].high_price.freshness,
            Freshness::Current,
            "a manual cell is never flagged stale (the user owns it)"
        );

        // Now fill the rest from the provider, flag, then RE-flag — the second is a no-op.
        let years: Vec<i32> = state
            .get_study(id)
            .unwrap()
            .years
            .iter()
            .map(|y| y.year)
            .collect();
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        state.mark_provider_stale(id).unwrap();
        let depth = state.undo_depth();
        let version = state.logical_version();
        let again = state.mark_provider_stale(id).unwrap();
        assert_eq!(again, 0, "already-stale cells are not re-flagged");
        assert_eq!(
            state.undo_depth(),
            depth,
            "an idempotent re-flag records no phantom undo step"
        );
        assert_eq!(
            state.logical_version(),
            version,
            "an idempotent re-flag writes no journal revision (no version bump)"
        );
        // The manually-held cell is still Current after both flags.
        assert_eq!(
            state.get_study(id).unwrap().years[0].high_price.source,
            Source::Manual
        );
        assert_eq!(
            state.get_study(id).unwrap().years[0].high_price.freshness,
            Freshness::Current
        );
    }

    /// A failed refresh that flags a validated provider study stale degrades the verdict to
    /// Provisional in the same frame (the production path through `mark_provider_stale`). (AC3)
    #[test]
    fn a_stale_flag_degrades_a_full_verdict_to_provisional() {
        use steadyinvest_core::verdict::Verdict;
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x72, "2026-06-27T11:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        for y in 0..years.len() {
            for field in [
                entry::FIELD_SALES,
                entry::FIELD_HIGH,
                entry::FIELD_LOW,
                entry::FIELD_EPS,
            ] {
                state.set_review(id, y, field, Review::Validated).unwrap();
            }
        }
        for (field, v) in [
            ("est_high_eps", 8),
            ("est_low_eps", 6),
            ("high_pe", 20),
            ("low_pe", 10),
            ("current_price", 60),
        ] {
            state
                .set_judgment_field(id, field, Some(und_money(v)))
                .unwrap();
        }
        assert!(
            matches!(
                engine::build_snapshot(&state.get_study(id).unwrap())
                    .unwrap()
                    .verdict(),
                Verdict::Full(_)
            ),
            "precondition: a validated provider study reads Full"
        );

        // A failed refresh flags the provider cells stale → the validated inputs degrade.
        state.mark_provider_stale(id).unwrap();
        assert!(
            matches!(
                engine::build_snapshot(&state.get_study(id).unwrap())
                    .unwrap()
                    .verdict(),
                Verdict::Provisional(_)
            ),
            "a stale validated load-bearing input degrades Full → Provisional"
        );
    }

    /// A later successful refresh re-confirms currency and clears the stale flag, even when the
    /// provider returns the SAME values. (AC2 lifecycle)
    #[test]
    fn a_successful_refresh_clears_the_stale_flag() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x73, "2026-06-27T11:30:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        state.mark_provider_stale(id).unwrap();
        assert_eq!(
            state.get_study(id).unwrap().years[0].high_price.freshness,
            Freshness::Stale
        );

        // The same data comes back on a successful retry — currency confirmed → Current again.
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        assert_eq!(
            state.get_study(id).unwrap().years[0].high_price.freshness,
            Freshness::Current,
            "a successful refresh clears the stale flag (even on unchanged values)"
        );
    }

    /// A successful refresh that covers only a SUBSET of the grid's years still clears the stale flag
    /// on the years it omits — the outage is over, so the recovery is study-wide, not per-fetched-cell
    /// (review-fix: a year/field the fetch omits must not stay stale forever). (AC2)
    #[test]
    fn a_successful_refresh_clears_stale_on_years_it_omits() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x74, "2026-06-27T12:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let all_years = [2020, 2021, 2022, 2023, 2024];
        state
            .apply_provider_refresh(id, &fetched_for(&all_years))
            .unwrap();
        state.mark_provider_stale(id).unwrap();

        // A narrower successful refresh (only the last 3 years) — the omitted 2020/2021 must recover.
        state
            .apply_provider_refresh(id, &fetched_for(&[2022, 2023, 2024]))
            .unwrap();
        let study = state.get_study(id).unwrap();
        let year_2020 = study.years.iter().find(|y| y.year == 2020).unwrap();
        assert_eq!(
            year_2020.high_price.freshness,
            Freshness::Current,
            "a year the successful fetch omitted still recovers from stale (outage over)"
        );
    }

    // ── Story 3.6 — annual update journey ──

    #[test]
    fn revalidate_counts_only_demoted_validated_cells() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x80, "2026-06-27T13:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        // Validate every high_price; leave the rest unvalidated.
        for y in 0..years.len() {
            state.set_review(id, y, "a", Review::Validated).unwrap();
        }
        // Refresh: high_price 100 → 200 diverges (demotes the 5 validated ✓); eps/sales/low unchanged.
        let report = state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 200, 50, "annual"))
            .unwrap();
        assert_eq!(
            report.revalidate, 5,
            "the 5 validated high_price cells that diverged are the re-validation scope"
        );
        // A second identical refresh demotes nothing (already ? + value agrees) → revalidate 0.
        let again = state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 200, 50, "annual"))
            .unwrap();
        assert_eq!(again.revalidate, 0, "an agreeing re-fetch demotes nothing");
    }

    #[test]
    fn refresh_summary_appends_the_revalidate_clause_only_when_needed() {
        let no_demote = RefreshReport {
            updated: 1,
            cause: crate::viewmodel::refresh::RefreshCause {
                price: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            refresh_summary(no_demote),
            refresh_notice(no_demote),
            "with no demotions the summary is exactly the cause notice (no regression)"
        );
        let with_demote = RefreshReport {
            revalidate: 3,
            ..no_demote
        };
        let summary = refresh_summary(with_demote);
        assert!(summary.starts_with(refresh_notice(with_demote)));
        assert!(
            summary.contains("3 cellule(s) à revérifier"),
            "the re-validation scope is named: {summary}"
        );
    }

    /// The Journey-2b ritual end-to-end through the real rails: reopen a saved validated study, re-fetch
    /// new annual data, and confirm manual + judgment preserved, changed ✓ → ?, unchanged ✓ kept, the
    /// re-validation count correct, and the projection extends. (AC1, AC2, AC3, AC4)
    #[test]
    fn the_annual_update_journey_preserves_manual_and_judgment_and_demotes_only_what_moved() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x81, "2026-06-27T13:30:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];

        // A saved study: provider-fetched, with a MANUAL override on year-0 sales, fully validated.
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        state
            .edit_cell(id, 0, entry::FIELD_SALES, Some(und_money(5000)))
            .unwrap();
        for y in 0..years.len() {
            for field in [
                entry::FIELD_SALES,
                entry::FIELD_HIGH,
                entry::FIELD_LOW,
                entry::FIELD_EPS,
            ] {
                state.set_review(id, y, field, Review::Validated).unwrap();
            }
        }
        for (field, v) in [
            ("est_high_eps", 8),
            ("est_low_eps", 6),
            ("high_pe", 20),
            ("low_pe", 10),
            ("current_price", 60),
        ] {
            state
                .set_judgment_field(id, field, Some(und_money(v)))
                .unwrap();
        }
        let judgment_before = state.get_study(id).unwrap().judgment;

        // A year later: the annual report lands. high_price 100 → 200 (diverges, provider cells);
        // sales 1000 (year-0 sales is held manually at 5000 → diverges → reconcile); eps/low agree.
        let report = state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 200, 50, "annual-2027"))
            .unwrap();

        let study = state.get_study(id).unwrap();
        // AC1 — the manual value stands (never overwritten); the judgment is untouched.
        let y0 = &study.years[0];
        assert_eq!(
            y0.sales.value,
            Some(und_money(5000)),
            "manual sales preserved"
        );
        assert_eq!(y0.sales.source, Source::Manual);
        assert_eq!(
            y0.sales.pending.as_ref().map(|p| p.value),
            Some(Some(und_money(1000))),
            "the divergent provider sales is preserved alongside (Story 3.4)"
        );
        assert_eq!(study.judgment, judgment_before, "judgment lines preserved");
        // AC2 — a changed validated provider cell is now ?; an unchanged one keeps ✓.
        assert_eq!(y0.high_price.review, Review::ToReview, "changed high ✓ → ?");
        assert_eq!(y0.high_price.value, Some(und_money(200)));
        assert_eq!(y0.eps.review, Review::Validated, "unchanged eps keeps ✓");
        assert_eq!(
            y0.sales.review,
            Review::ToReview,
            "diverged manual sales → ?"
        );
        // AC3 — the re-validation scope: 5 high_price + the 1 manual sales = 6.
        assert_eq!(report.revalidate, 6, "only what moved needs re-validation");
        assert!(refresh_summary(report).contains("6 cellule(s) à revérifier"));

        // AC4 — extend the projection: the new fiscal year row appends, prior years intact.
        let max_before = study.years.iter().map(|y| y.year).max().unwrap();
        state.extend_history(id).unwrap();
        let extended = state.get_study(id).unwrap();
        assert_eq!(
            extended.years.iter().map(|y| y.year).max().unwrap(),
            max_before + 1,
            "the projection extends by one fiscal year"
        );
        assert_eq!(
            extended.years[0].sales.value,
            Some(und_money(5000)),
            "extending leaves the existing years intact"
        );
    }

    /// AC5 — the "unlock all → re-fetch" path: after unlocking, a refresh demotes nothing (nothing
    /// was ✓) and the manual values are still preserved.
    #[test]
    fn unlock_all_then_refresh_demotes_nothing_and_preserves_manual() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x82, "2026-06-27T14:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        state
            .edit_cell(id, 0, entry::FIELD_SALES, Some(und_money(5000)))
            .unwrap();
        for y in 0..years.len() {
            state.set_review(id, y, "a", Review::Validated).unwrap();
        }
        // Unlock the whole study, THEN refresh with divergent data.
        state.unlock_all(id, &UnlockScope::Study).unwrap();
        let report = state
            .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 200, 50, "annual"))
            .unwrap();
        assert_eq!(
            report.revalidate, 0,
            "nothing was ✓ after unlock → no demotions to re-validate"
        );
        assert_eq!(
            state.get_study(id).unwrap().years[0].sales.value,
            Some(und_money(5000)),
            "the manual value is still preserved after unlock + refresh"
        );
    }

    // ── Story 4.1 — watchlist app rails ──

    fn watch_id(state: &JournalState, ticker: &str) -> Uuid {
        state
            .list_watch_items()
            .into_iter()
            .find(|w| w.security_ticker == ticker)
            .map(|w| w.id)
            .expect("the watch item exists")
    }

    #[test]
    fn watchlist_add_list_move_delete_through_the_app_rails() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x900);
        state.add_watch_item("NESN", None).unwrap();
        state.add_watch_item("ROG", None).unwrap();
        state.add_watch_item("NOVN", None).unwrap();
        let order = |s: &JournalState| {
            s.list_watch_items()
                .into_iter()
                .map(|w| w.security_ticker)
                .collect::<Vec<_>>()
        };
        assert_eq!(order(&state), ["NESN", "ROG", "NOVN"]);

        // Move ROG up → ROG, NESN, NOVN.
        let rog = watch_id(&state, "ROG");
        state.move_watch_item(rog, true).unwrap();
        assert_eq!(order(&state), ["ROG", "NESN", "NOVN"]);

        // Move ROG up again at the top edge → no-op.
        state.move_watch_item(rog, true).unwrap();
        assert_eq!(order(&state), ["ROG", "NESN", "NOVN"]);

        // Delete NESN → re-packed contiguous.
        state.delete_watch_item(watch_id(&state, "NESN")).unwrap();
        assert_eq!(order(&state), ["ROG", "NOVN"]);
        assert_eq!(
            state
                .list_watch_items()
                .into_iter()
                .map(|w| w.position)
                .collect::<Vec<_>>(),
            [0, 1]
        );
    }

    #[test]
    fn add_watch_blank_ticker_is_refused_and_link_round_trips() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x910);
        assert!(
            state.add_watch_item("   ", None).is_err(),
            "blank ticker refused"
        );

        let study = state.create_study("NESN", "CHF").unwrap();
        state.add_watch_item("NESN", Some(study)).unwrap();
        assert_eq!(
            state.list_watch_items()[0].study_id,
            Some(study),
            "the study link round-trips through the app rail"
        );
        // Clearing it via update.
        let wid = watch_id(&state, "NESN");
        state.update_watch_item(wid, "NESN", None).unwrap();
        assert_eq!(state.list_watch_items()[0].study_id, None);
    }

    #[test]
    fn study_id_for_ticker_matches_case_insensitively_and_picks_most_recent() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x920);
        let first = state.create_study("NESN", "CHF").unwrap();
        let second = state.create_study("NESN", "CHF").unwrap();
        // A lowercase watched ticker still resolves to the (most recent) "NESN" study.
        assert_eq!(
            state.study_id_for_ticker("nesn"),
            Some(second),
            "case-insensitive + most-recent"
        );
        assert_ne!(state.study_id_for_ticker("nesn"), Some(first));
        assert_eq!(state.study_id_for_ticker("UNKNOWN"), None);
    }

    // ── Story 4.2 — buy-zone alert (the app-surface read of the engine zone) ──

    #[test]
    fn study_in_buy_zone_reflects_the_current_price_and_is_verdict_independent() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x95, "2026-06-27T16:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];
        // Provider-fill (cells stay Review::None → the verdict is NOT Full) + a complete judgment so
        // the §4 forecast band exists; est_low_eps 6 × low_pe 10 ⇒ forecast low ≈ 60.
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        // Forecast band ≈ [low 50–60, high 160] (high = est_high_eps 8 × high_pe 20). The buy third
        // is ≈ [low, 93] (buy_top = low + (high − low)/3). A current_price of 70 sits in it.
        for (field, v) in [
            ("est_high_eps", 8),
            ("est_low_eps", 6),
            ("high_pe", 20),
            ("low_pe", 10),
            ("current_price", 70),
        ] {
            state
                .set_judgment_field(id, field, Some(und_money(v)))
                .unwrap();
        }
        // The verdict is NOT Full (unvalidated provider cells), yet the buy-zone fact still holds
        // (AC6 — verdict-independent).
        assert!(
            engine::study_in_buy_zone(&state.get_study(id).unwrap()),
            "a current price in the bottom third of the band is in the buy zone, regardless of verdict"
        );

        // Move the price into the upper band (sell third) → not in the buy zone.
        state
            .set_judgment_field(id, "current_price", Some(und_money(150)))
            .unwrap();
        assert!(!engine::study_in_buy_zone(&state.get_study(id).unwrap()));

        // No current price → no defined zone → not in the buy zone.
        state.set_judgment_field(id, "current_price", None).unwrap();
        assert!(!engine::study_in_buy_zone(&state.get_study(id).unwrap()));
    }

    // ── Story 4.4 — manual price refresh fills current_price from the latest close ──

    #[test]
    fn provider_refresh_fills_current_price_from_latest_close_and_moves_the_zone() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x4C, "2026-06-27T16:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];
        // Provider-fill the yearly cells, then a complete forecast band — but NO current_price yet, so
        // the §4 zone is undefined (band ≈ [low 60, high 160]; buy third ≈ [60, 93]).
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        for (field, v) in [
            ("est_high_eps", 8),
            ("est_low_eps", 6),
            ("high_pe", 20),
            ("low_pe", 10),
        ] {
            state
                .set_judgment_field(id, field, Some(und_money(v)))
                .unwrap();
        }
        assert!(
            state
                .get_study(id)
                .unwrap()
                .judgment
                .current_price
                .is_none(),
            "no current_price yet → no defined zone"
        );
        assert!(!engine::study_in_buy_zone(&state.get_study(id).unwrap()));

        // A refresh carrying a latest close of 70 sets current_price (a market fact, AC6) and the buy
        // third ≈ [60, 93] now brackets it → in the buy zone, verdict-independent.
        state
            .apply_provider_refresh(id, &fetched_with_price(&years, 70))
            .unwrap();
        assert_eq!(
            state.get_study(id).unwrap().judgment.current_price,
            Some(und_money(70)),
            "the latest /eod close fills current_price"
        );
        assert!(
            engine::study_in_buy_zone(&state.get_study(id).unwrap()),
            "current_price 70 sits in the buy third → in the buy zone"
        );

        // A later refresh with no latest price (the pre-4.4 shape) leaves current_price untouched.
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        assert_eq!(
            state.get_study(id).unwrap().judgment.current_price,
            Some(und_money(70)),
            "latest_price = None must not clear the last-known current_price"
        );
    }

    #[test]
    fn study_zone_reports_the_full_buy_neutral_sell_zone_for_holdings() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x4D, "2026-06-27T16:00:00Z");
        let id = state.create_study("ROG", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        for (field, v) in [
            ("est_high_eps", 8),
            ("est_low_eps", 6),
            ("high_pe", 20),
            ("low_pe", 10),
        ] {
            state
                .set_judgment_field(id, field, Some(und_money(v)))
                .unwrap();
        }
        // Band ≈ [60, 160]; thirds: buy ≤ ~93, neutral ~93–127, sell ≥ ~127. The holdings register
        // reads the FULL zone (Achat/Neutre/Vente), not just "in the buy zone".
        let zone =
            |st: &JournalState| engine::zone_key(engine::study_zone(&st.get_study(id).unwrap()));
        assert_eq!(zone(&state), "", "no current_price yet → undefined zone");
        for (price, expected) in [(70, "buy"), (110, "neutral"), (150, "sell")] {
            state
                .set_judgment_field(id, "current_price", Some(und_money(price)))
                .unwrap();
            assert_eq!(zone(&state), expected, "current_price {price}");
        }
        // A price outside `[forecast_low, forecast_high]` has no defined zone (the register shows "—").
        state
            .set_judgment_field(id, "current_price", Some(und_money(300)))
            .unwrap();
        assert_eq!(zone(&state), "", "a price above the band → no zone");
    }

    #[test]
    fn apply_holding_price_sets_current_price_only_and_moves_the_zone() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x4E, "2026-06-27T16:00:00Z");
        let id = state.create_study("ROG", "CHF").unwrap();
        let years = [2020, 2021, 2022, 2023, 2024];
        state
            .apply_provider_refresh(id, &fetched_for(&years))
            .unwrap();
        for (field, v) in [
            ("est_high_eps", 8),
            ("est_low_eps", 6),
            ("high_pe", 20),
            ("low_pe", 10),
        ] {
            state
                .set_judgment_field(id, field, Some(und_money(v)))
                .unwrap();
        }
        // Snapshot the yearly cells to prove the price-only refresh (issue #50) leaves them untouched.
        let before_years = state.get_study(id).unwrap().years.clone();

        state
            .apply_holding_price(id, rust_decimal::Decimal::new(70, 0))
            .unwrap();

        let after = state.get_study(id).unwrap();
        assert_eq!(
            after.judgment.current_price,
            Some(und_money(70)),
            "the price-only fill sets current_price"
        );
        assert_eq!(
            after.years, before_years,
            "a price-only holding refresh must NOT touch the yearly provider cells"
        );
        assert!(
            engine::study_in_buy_zone(&after),
            "price 70 sits in the buy third → the zone recomputes"
        );
    }

    // ── Story 4.5 — trailing stop per holding (validate, seed, ratchet) ──

    #[test]
    fn set_holding_trailing_stop_validates_seeds_from_purchase_price_and_clears() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x55, "2026-06-28T10:00:00Z");
        state.add_holding("NESN", "10", "100").unwrap();
        let id = state.list_holdings()[0].id;

        // Out-of-range / non-numeric pct → refused, nothing written.
        for bad in ["0", "100", "150", "-5", "abc", "1.2.3"] {
            assert_eq!(
                state.set_holding_trailing_stop(id, bad),
                Err(MSG_HOLDING_INVALID_STOP.to_string()),
                "pct {bad:?} is refused"
            );
        }
        assert!(state.list_holdings()[0].trailing_stop_pct.is_none());

        // No linked study → the level seeds from the purchase price 100: 100 × (1 − 0.15) = 85.
        state.set_holding_trailing_stop(id, "15").unwrap();
        let h = state.list_holdings().into_iter().next().unwrap();
        assert_eq!(h.trailing_stop_pct.as_deref(), Some("15"));
        assert_eq!(h.trailing_stop_level.as_deref(), Some("85"));

        // Review fix: an EXPLICIT re-set seeds FRESH (the user's pct wins) — a looser 50% LOWERS the
        // level to 100 × (1 − 0.50) = 50, even though 50 < the prior 85 (ratchet-up-only governs only
        // the automatic refresh path, not an explicit re-parametrisation).
        state.set_holding_trailing_stop(id, "50").unwrap();
        let h = state.list_holdings().into_iter().next().unwrap();
        assert_eq!(h.trailing_stop_pct.as_deref(), Some("50"));
        assert_eq!(h.trailing_stop_level.as_deref(), Some("50"));

        // An empty pct clears the stop (both fields → None).
        state.set_holding_trailing_stop(id, "").unwrap();
        let h = state.list_holdings().into_iter().next().unwrap();
        assert!(h.trailing_stop_pct.is_none() && h.trailing_stop_level.is_none());
    }

    #[test]
    fn ratchet_trailing_stops_moves_up_only_on_a_price_refresh() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x56, "2026-06-28T10:00:00Z");
        // A holding linked to a study of the same ticker (so the ratchet keys on the study's price).
        let study = state.create_study("NESN", "CHF").unwrap();
        state.add_holding("NESN", "10", "100").unwrap();
        let id = state.list_holdings()[0].id;
        // Seed a 20% stop → level 80 (from purchase 100, no current_price yet).
        state.set_holding_trailing_stop(id, "20").unwrap();
        assert_eq!(
            state.list_holdings()[0].trailing_stop_level.as_deref(),
            Some("80")
        );

        // A refresh to 150 ratchets the level up: 150 × 0.80 = 120.
        state
            .ratchet_trailing_stops_for_study(study, Decimal::from(150))
            .unwrap();
        assert_eq!(
            state.list_holdings()[0].trailing_stop_level.as_deref(),
            Some("120")
        );

        // A refresh to a LOWER 90 leaves the level at 120 (ratchet-up only).
        state
            .ratchet_trailing_stops_for_study(study, Decimal::from(90))
            .unwrap();
        assert_eq!(
            state.list_holdings()[0].trailing_stop_level.as_deref(),
            Some("120"),
            "a falling price never lowers the stop"
        );
    }

    // ── Story 4.6 — simple capital-at-risk (the portfolio downside figure) ──

    #[test]
    fn portfolio_capital_at_risk_sums_below_cost_stops_and_invested() {
        let dir = TempDir::new().unwrap();
        // `watch_state` uses a SEQUENTIAL idgen — two holdings get distinct ids (a FixedIdGen would
        // collide on the second insert).
        let mut state = watch_state(&dir, 0x570);
        state.add_holding("NESN", "10", "100").unwrap();
        state.add_holding("ROG", "20", "50").unwrap();
        let ids: Vec<_> = state.list_holdings().iter().map(|h| h.id).collect();
        // NESN: a 15% stop with no study → level 85 (below cost 100) → (100−85)×10 = 150.
        state.set_holding_trailing_stop(ids[0], "15").unwrap();
        // ROG: no stop → contributes 0 to capital-at-risk (but to invested).

        let (car, invested) = state.portfolio_capital_at_risk();
        assert_eq!(
            car,
            Decimal::from(150),
            "only the below-cost stop contributes"
        );
        assert_eq!(
            invested,
            Decimal::from(100 * 10 + 50 * 20),
            "invested = Σ cost × qty"
        );
    }

    // ── Story 4.7 — recorded sell on a neutral trigger ──

    #[test]
    fn sell_holding_records_the_sell_and_drops_it_from_the_register() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x470);
        state.add_holding("NESN", "10", "100").unwrap();
        state.add_holding("ROG", "20", "50").unwrap();
        let ids: Vec<_> = state.list_holdings().iter().map(|h| h.id).collect();
        // NESN gets a 15% stop (no study) → level 85, below cost 100 → CaR 150 before the sell.
        state.set_holding_trailing_stop(ids[0], "15").unwrap();
        assert_eq!(state.portfolio_capital_at_risk().0, Decimal::from(150));

        state
            .sell_holding(ids[0], "  stop touché  ", "CHF")
            .expect("the sell records");

        let remaining: Vec<_> = state
            .list_holdings()
            .iter()
            .map(|h| h.security_ticker.clone())
            .collect();
        assert_eq!(remaining, vec!["ROG".to_string()], "NESN left the register");
        assert_eq!(
            state.portfolio_capital_at_risk().0,
            Decimal::ZERO,
            "the only at-risk holding is gone → capital-at-risk drops to 0"
        );
    }

    #[test]
    fn sell_holding_refuses_an_absent_id() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x471);
        state.add_holding("NESN", "10", "100").unwrap();
        let ghost = Uuid::from_u128(0xDEAD);
        assert!(
            state.sell_holding(ghost, "", "CHF").is_err(),
            "selling a non-existent holding is refused, nothing written"
        );
        assert_eq!(state.list_holdings().len(), 1, "the register is untouched");
    }

    // ── Story 4.3 — holdings register (single-portfolio CRUD + decimal validation) ──

    #[test]
    fn add_holding_persists_lazily_creates_one_portfolio_and_lists_in_order() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x430);
        assert!(
            state.list_holdings().is_empty(),
            "no holdings, no portfolio yet"
        );

        state.add_holding("NESN", "10", "95.40").unwrap();
        state.add_holding("ROG", "5", "248.10").unwrap();
        let rows = state.list_holdings();
        assert_eq!(
            rows.iter()
                .map(|h| h.security_ticker.as_str())
                .collect::<Vec<_>>(),
            ["NESN", "ROG"],
            "both holdings persist, in creation order"
        );
        assert_eq!(rows[0].quantity, "10");
        assert_eq!(rows[0].purchase_price, "95.40");
        // All holdings share the single lazily-created portfolio.
        assert_eq!(rows[0].portfolio_id, rows[1].portfolio_id, "one portfolio");
    }

    #[test]
    fn holding_amounts_are_validated_and_bad_input_writes_nothing() {
        let dir = TempDir::new().unwrap();
        let mut state = watch_state(&dir, 0x431);
        assert_eq!(
            state.add_holding("  ", "10", "5").unwrap_err(),
            MSG_HOLDING_INVALID_TICKER
        );
        assert_eq!(
            state.add_holding("NESN", "abc", "5").unwrap_err(),
            MSG_HOLDING_INVALID_NUMBER
        );
        assert_eq!(
            state.add_holding("NESN", "0", "5").unwrap_err(),
            MSG_HOLDING_INVALID_NUMBER,
            "quantity must be strictly positive"
        );
        assert_eq!(
            state.add_holding("NESN", "-2", "5").unwrap_err(),
            MSG_HOLDING_INVALID_NUMBER
        );
        assert_eq!(
            state.add_holding("NESN", "2", "-5").unwrap_err(),
            MSG_HOLDING_INVALID_NUMBER,
            "price must be non-negative"
        );
        assert!(
            state.list_holdings().is_empty(),
            "no invalid input wrote a row"
        );
        // A free purchase (price 0) is allowed (e.g. a gift/spin-off).
        state.add_holding("FREE", "1", "0").unwrap();
        assert_eq!(state.list_holdings().len(), 1);
    }

    #[test]
    fn edit_and_delete_holding_round_trip_and_survive_reopen() {
        let dir = TempDir::new().unwrap();
        let id = {
            let mut state = watch_state(&dir, 0x432);
            state.add_holding("NESN", "10", "95.40").unwrap();
            state.add_holding("ROG", "5", "248.10").unwrap();
            let nesn = state.list_holdings()[0].id;
            state
                .update_holding(nesn, "NESN.SW", "12", "96.00")
                .unwrap();
            let rog = state.list_holdings()[1].id;
            state.delete_holding(rog).unwrap();
            nesn
        };
        // Reopen the same on-disk journal → the edit and the delete persisted.
        let reopened = watch_state(&dir, 0x999);
        let rows = reopened.list_holdings();
        assert_eq!(rows.len(), 1, "the deleted holding stayed deleted");
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].security_ticker, "NESN.SW");
        assert_eq!(rows[0].quantity, "12");
        assert_eq!(rows[0].purchase_price, "96.00");
    }

    #[test]
    fn undo_redo_steps_back_and_forward_and_a_new_edit_clears_redo() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x1D, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        assert!(
            !state.can_undo() && !state.can_redo(),
            "a fresh study has empty history"
        );

        // Edit one §3 cell (field "a" = high price) on year 0.
        state.edit_cell(id, 0, "a", Some(und_money(100))).unwrap();
        assert!(state.can_undo(), "an edit is undoable");
        assert!(!state.can_redo());
        assert!(state.get_study(id).unwrap().years[0]
            .high_price
            .value
            .is_some());

        // Undo → the pre-edit (fresh, no-value) state returns.
        assert_eq!(state.undo(id), Ok(true));
        assert!(state.can_redo());
        let undone = state.get_study(id).unwrap();
        assert!(
            undone.years.is_empty() || undone.years[0].high_price.value.is_none(),
            "undo restores the pre-edit state (no value)"
        );

        // Redo → the value comes back.
        assert_eq!(state.redo(id), Ok(true));
        assert!(state.get_study(id).unwrap().years[0]
            .high_price
            .value
            .is_some());

        // A NEW edit after an undo forks history → the redo branch is cleared.
        assert_eq!(state.undo(id), Ok(true));
        assert!(state.can_redo());
        state.edit_cell(id, 0, "b", Some(und_money(50))).unwrap();
        assert!(
            !state.can_redo(),
            "a new edit after an undo clears the redo branch"
        );
        assert!(state.can_undo());
    }

    #[test]
    fn undo_restores_a_judgment_edit() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x2D, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        state
            .set_judgment_field(id, "est_high_eps", Some(und_money(9)))
            .unwrap();
        assert!(state
            .get_study(id)
            .unwrap()
            .judgment
            .estimated_high_eps
            .is_some());
        assert_eq!(state.undo(id), Ok(true));
        assert!(
            state
                .get_study(id)
                .unwrap()
                .judgment
                .estimated_high_eps
                .is_none(),
            "undo restores the prior (unset) judgment — FR32, never destroys a saved input"
        );
    }

    #[test]
    fn undo_redo_on_empty_history_are_noops() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x3D, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        assert_eq!(state.undo(id), Ok(false), "nothing to undo");
        assert_eq!(state.redo(id), Ok(false), "nothing to redo");
        assert!(!state.can_undo() && !state.can_redo());
    }

    #[test]
    fn reset_undo_clears_history() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x4D, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        state
            .set_judgment_field(id, "est_high_eps", Some(und_money(9)))
            .unwrap();
        assert!(state.can_undo());
        state.reset_undo(); // a different study is opened
        assert!(
            !state.can_undo() && !state.can_redo(),
            "opening a study starts from an empty history"
        );
    }

    // ── Story 2.10 — decision rationale: set → reopen restores; clear → None; trim; undo restores ──

    #[test]
    fn rationale_round_trips_through_reopen_and_clears_to_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xC0FFEE),
                &Timestamp("2026-06-14T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut state = open_state(&path);
        let id = state.create_study("NESN", "CHF").unwrap();

        // Set a rationale → it is restored on reopen (a fresh JournalState on the same journal, FR49).
        state
            .set_rationale(id, Some("Marge en hausse, dette faible".to_string()))
            .unwrap();
        assert_eq!(
            open_state(&path)
                .get_study(id)
                .unwrap()
                .rationale
                .as_deref(),
            Some("Marge en hausse, dette faible"),
            "a saved rationale survives reopen (FR49)"
        );

        // Whitespace-only clears to None (absence ≠ empty value) — never Some("").
        state.set_rationale(id, Some("   ".to_string())).unwrap();
        assert_eq!(
            open_state(&path).get_study(id).unwrap().rationale,
            None,
            "an empty/whitespace rationale stores None, never Some(\"\")"
        );

        // A bare `None` clears it too.
        state
            .set_rationale(id, Some("re-rempli".to_string()))
            .unwrap();
        state.set_rationale(id, None).unwrap();
        assert_eq!(open_state(&path).get_study(id).unwrap().rationale, None);
    }

    #[test]
    fn rationale_is_trimmed_before_storage() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xA),
                &Timestamp("2026-06-14T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut state = open_state(&path);
        let id = state.create_study("NESN", "CHF").unwrap();
        state
            .set_rationale(id, Some("  garde le texte  ".to_string()))
            .unwrap();
        assert_eq!(
            state.get_study(id).unwrap().rationale.as_deref(),
            Some("garde le texte"),
            "surrounding whitespace is trimmed before storage"
        );
    }

    #[test]
    fn undo_restores_the_prior_rationale() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x6A, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();

        state
            .set_rationale(id, Some("première raison".to_string()))
            .unwrap();
        state
            .set_rationale(id, Some("raison révisée".to_string()))
            .unwrap();
        assert_eq!(
            state.get_study(id).unwrap().rationale.as_deref(),
            Some("raison révisée")
        );

        // Undo restores the prior rationale (FR32 — a rationale edit is "any edit", never destroyed).
        assert_eq!(state.undo(id), Ok(true));
        assert_eq!(
            state.get_study(id).unwrap().rationale.as_deref(),
            Some("première raison"),
            "undo restores the prior rationale, never destroys it"
        );
    }

    #[test]
    fn re_saving_the_same_rationale_records_no_phantom_undo_step() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x6B, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        state
            .set_rationale(id, Some("inchangé".to_string()))
            .unwrap();
        state.reset_undo();
        // Re-saving the identical rationale (after trimming) is a no-op → no undo step recorded (P4).
        state
            .set_rationale(id, Some("  inchangé  ".to_string()))
            .unwrap();
        assert!(
            !state.can_undo(),
            "re-saving the same rationale records no phantom undo step (review P4)"
        );
    }

    // ── Story 2.11 — update an existing study & extend its projection (roll the window forward) ──

    #[test]
    fn extend_history_appends_next_year_and_survives_reopen() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        // The window is materialized (2021..=2025); extending rolls it forward by one (2026).
        state.extend_history(id).expect("extend persists");
        let back = open_state(&path).get_study(id).expect("study reopens");
        assert_eq!(
            back.years.len(),
            entry::YEAR_WINDOW + 1,
            "the data window grew forward by one year"
        );
        let added = back.years.last().unwrap();
        assert_eq!(
            added.year, 2026,
            "the appended year is latest+1 (newest at the bottom, SSG order)"
        );
        assert_eq!(
            added.eps.value, None,
            "the appended year is a to-fill gap, never 0"
        );
        assert_eq!(added.eps.coverage, Coverage::ToFill);
    }

    #[test]
    fn extend_history_rolls_the_window_forward_each_call() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        state.extend_history(id).unwrap(); // 2026
        state.extend_history(id).unwrap(); // 2027
        let years: Vec<i32> = state
            .get_study(id)
            .unwrap()
            .years
            .iter()
            .map(|y| y.year)
            .collect();
        assert_eq!(
            years,
            vec![2021, 2022, 2023, 2024, 2025, 2026, 2027],
            "each call appends the next year (oldest→newest, horizon re-bases off the new latest)"
        );
    }

    #[test]
    fn undo_restores_the_pre_extend_year_window() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);
        let before = state.get_study(id).unwrap().years.len();

        state.extend_history(id).unwrap();
        assert_eq!(state.get_study(id).unwrap().years.len(), before + 1);

        // Adding a year is "any edit" — one undo step restores the prior window (FR32, never destroys).
        assert_eq!(state.undo(id), Ok(true));
        assert_eq!(
            state.get_study(id).unwrap().years.len(),
            before,
            "undo restores the pre-add window"
        );
    }

    #[test]
    fn extend_history_on_a_read_only_journal_is_refused() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let id = study_with_entry(&path).1; // seeds + materializes the 5-year window, then drops the state

        let mut state = open_state(&path);
        state.read_only = true;
        assert_eq!(
            state.extend_history(id),
            Err(MSG_READ_ONLY_WRITE.to_string())
        );
        assert_eq!(
            open_state(&path).get_study(id).unwrap().years.len(),
            entry::YEAR_WINDOW,
            "a refused extend appended nothing"
        );
    }

    #[test]
    fn editing_and_the_soft_lock_hold_across_a_reopen() {
        // AC1/AC2 regression: the existing edit + soft-lock rails behave correctly when the study is
        // edited through a fresh reopen (a new JournalState on the same journal), not just in-session.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let id = study_with_entry(&path).1; // high_price@0 = 120.5, window materialized

        // Validate the cell, then REOPEN a fresh state on the same journal.
        {
            let mut state = open_state(&path);
            state
                .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
                .unwrap();
        }
        let mut reopened = open_state(&path);

        // AC2: the soft-lock survives the reopen — a typed edit on the ✓ cell is still refused.
        assert_eq!(
            reopened.edit_cell(id, 0, entry::FIELD_HIGH, Some(money("999"))),
            Err(MSG_SOFT_LOCKED.to_string())
        );

        // AC1: after the deliberate clear-✓, an edit on the reopened study persists (recompute frame).
        reopened
            .set_review(id, 0, entry::FIELD_HIGH, Review::ToReview)
            .unwrap();
        reopened
            .edit_cell(id, 0, entry::FIELD_HIGH, Some(money("130")))
            .expect("a ? cell edits normally after reopen");
        assert_eq!(
            open_state(&path).get_study(id).unwrap().years[0]
                .high_price
                .value,
            Some(money("130")),
            "an edit on a reopened study persists"
        );
    }

    #[test]
    fn undo_restores_a_review_tag_without_destroying_the_value() {
        let dir = TempDir::new().unwrap();
        let mut state = undo_state(&dir, 0x5E, "2026-06-14T09:00:00Z");
        let id = state.create_study("NESN", "CHF").unwrap();
        state.edit_cell(id, 0, "a", Some(und_money(100))).unwrap();
        state.set_review(id, 0, "a", Review::Validated).unwrap();
        assert_eq!(
            state.get_study(id).unwrap().years[0].high_price.review,
            Review::Validated
        );
        assert_eq!(state.undo(id), Ok(true)); // undo the review change only
        let undone = state.get_study(id).unwrap();
        assert_eq!(
            undone.years[0].high_price.review,
            Review::None,
            "undo restores the prior review tag"
        );
        assert!(
            undone.years[0].high_price.value.is_some(),
            "undoing the review tag never destroys the value"
        );
    }

    #[test]
    fn create_then_list_then_reopen_restores_full_state() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (clock, idgen) = fixed(0x5D, "2026-06-13T09:00:00Z");
        // Pre-create a journal at a known path so `open_or_create` opens it (rather than falling
        // through to the OS data dir, which is unavailable / undesirable under test).
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xC0FFEE),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );

        let (mut state, notice) = JournalState::open_or_create(Some(&path), clock, idgen);
        assert!(notice.is_none(), "clean open has no notice");
        assert!(!state.is_read_only());
        assert_eq!(state.path(), Some(path.as_path()));
        assert_eq!(state.list_studies().len(), 0, "no studies yet");

        let id = state
            .create_study("  NESN ", " chf ")
            .expect("a valid study is created");
        let rows = state.list_studies();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].security_ticker, "NESN",
            "ticker trimmed, case preserved"
        );
        assert_eq!(rows[0].status, "active");

        let back = state.get_study(id).expect("the study reopens");
        assert_eq!(back.security_ticker, "NESN");
        assert_eq!(
            back.native_currency, "CHF",
            "currency trimmed + upper-cased"
        );
        assert!(back.years.is_empty(), "a fresh study has no years");
        assert_eq!(back.created_at.0, "2026-06-13T09:00:00Z", "injected clock");
        assert_eq!(id, Uuid::from_u128(0x5D), "injected id");
    }

    #[test]
    fn blank_ticker_is_refused_with_a_neutral_message_and_writes_nothing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xA),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let (clock, idgen) = fixed(0x1, "2026-06-13T09:00:00Z");
        let (mut state, _) = JournalState::open_or_create(Some(&path), clock, idgen);

        assert_eq!(
            state.create_study("   ", "CHF"),
            Err(MSG_BLANK_TICKER.into())
        );
        assert_eq!(
            state.create_study("NESN", "  "),
            Err(MSG_BLANK_CURRENCY.into())
        );
        assert_eq!(state.list_studies().len(), 0, "no study was written");
    }

    #[test]
    fn missing_configured_file_falls_through_to_a_created_default_or_none() {
        // A configured path that does NOT exist must not be opened as an empty journal; the code
        // falls through to the default. In a sandbox the data dir may be unavailable — either a
        // created default (Some path) or a clean no-journal state is acceptable, never a panic.
        let (clock, idgen) = fixed(0x2, "2026-06-13T09:00:00Z");
        let missing = PathBuf::from("/nonexistent/steadyinvest/journal.db");
        let (state, _notice) = JournalState::open_or_create(Some(&missing), clock, idgen);
        assert_ne!(
            state.path(),
            Some(missing.as_path()),
            "a missing configured file is never adopted as-is"
        );
    }

    #[test]
    fn created_at_date_takes_the_date_portion() {
        assert_eq!(
            created_at_date(&Timestamp("2026-06-13T09:00:00Z".to_string())),
            "2026-06-13"
        );
        assert_eq!(created_at_date(&Timestamp("weird".to_string())), "weird");
    }

    // ── Story 2.4: manual entry → `Cell::edited` → `put_study` → reopen round-trip ──

    fn open_state(path: &Path) -> JournalState {
        let (clock, idgen) = fixed(0x5D, "2026-06-13T09:00:00Z");
        let (state, _) = JournalState::open_or_create(Some(path), clock, idgen);
        state
    }

    fn money(s: &str) -> Money {
        Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
    }

    #[test]
    fn manual_edit_stamps_source_manual_present_and_survives_reopen() {
        use steadyinvest_contract::{Freshness, Source};
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xC0FFEE),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut state = open_state(&path);
        let id = state.create_study("NESN", "CHF").unwrap();

        // A fresh study has no years; the first edit materializes the window then sets the cell.
        state
            .edit_cell(id, 0, entry::FIELD_HIGH, Some(money("120.5")))
            .expect("edit persists");

        // Reopen from disk: the entered value, its manual source/freshness and Present coverage survive.
        let back = open_state(&path).get_study(id).expect("study reopens");
        assert_eq!(
            back.years.len(),
            entry::YEAR_WINDOW,
            "the window was materialized"
        );
        let cell = &back.years[0].high_price;
        assert_eq!(cell.value, Some(money("120.5")));
        assert_eq!(
            cell.source,
            Source::Manual,
            "a manual edit is stamped source=manual"
        );
        assert_eq!(
            cell.freshness,
            Freshness::Current,
            "a fresh edit is current"
        );
        assert_eq!(cell.coverage, Coverage::Present);
        assert_eq!(cell.provenance.source, Source::Manual);
        assert_eq!(
            cell.provenance.timestamp.0, "2026-06-13T09:00:00Z",
            "the provenance timestamp comes from the injected clock"
        );
    }

    #[test]
    fn clearing_a_cell_reopens_a_to_fill_gap_never_zero() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xA),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut state = open_state(&path);
        let id = state.create_study("NESN", "CHF").unwrap();

        state
            .edit_cell(id, 1, entry::FIELD_EPS, Some(money("4.2")))
            .unwrap();
        state.edit_cell(id, 1, entry::FIELD_EPS, None).unwrap(); // clear it

        let cell = open_state(&path).get_study(id).unwrap().years[1]
            .eps
            .clone();
        assert_eq!(cell.value, None, "a cleared cell holds no value — never 0");
        assert_eq!(
            cell.coverage,
            Coverage::ToFill,
            "a cleared cell is a to-fill gap"
        );
    }

    #[test]
    fn not_available_is_a_distinct_quiet_gap_that_survives_reopen() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xB),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut state = open_state(&path);
        let id = state.create_study("NESN", "CHF").unwrap();

        // An optional column (dividend) marked not-available: distinct from to-fill and from 0.
        state
            .set_not_available(id, 2, entry::FIELD_DIVIDEND, true)
            .unwrap();
        let cell = open_state(&path).get_study(id).unwrap().years[2]
            .dividend_per_share
            .clone()
            .expect("the cell now exists");
        assert_eq!(cell.value, None);
        assert_eq!(cell.coverage, Coverage::NotAvailableAccepted);

        // Clearing it back returns a to-fill gap.
        state
            .set_not_available(id, 2, entry::FIELD_DIVIDEND, false)
            .unwrap();
        let back = open_state(&path).get_study(id).unwrap().years[2]
            .dividend_per_share
            .clone()
            .unwrap();
        assert_eq!(back.coverage, Coverage::ToFill);
    }

    // ── Story 2.5: tri-state review tag set/clear → persist → reopen; soft-lock; bulk unlock ──

    /// Open a journal at `path`, create a study, and fill A/C on year 0 so there is a present cell to
    /// review. Returns the state (still open) and the study id.
    fn study_with_entry(path: &Path) -> (JournalState, Uuid) {
        drop(
            Journal::create(
                path,
                Uuid::from_u128(0xC0FFEE),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let mut state = open_state(path);
        let id = state.create_study("NESN", "CHF").unwrap();
        state
            .edit_cell(id, 0, entry::FIELD_HIGH, Some(money("120.5")))
            .unwrap();
        (state, id)
    }

    #[test]
    fn set_review_survives_reopen_and_leaves_value_and_coverage_untouched() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        // none → ? → ✓, each persisted; the value and coverage never move (review-only edits).
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::ToReview)
            .unwrap();
        let cell = open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .clone();
        assert_eq!(cell.review, Review::ToReview);
        assert_eq!(cell.value, Some(money("120.5")), "value untouched by ?");
        assert_eq!(cell.coverage, Coverage::Present, "coverage untouched by ?");

        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
        let cell = open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .clone();
        assert_eq!(cell.review, Review::Validated, "✓ survives reopen");
        assert_eq!(cell.value, Some(money("120.5")));
        assert_eq!(cell.coverage, Coverage::Present);

        // ✓ → none clears the tag; still a review-only change.
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::None)
            .unwrap();
        let cell = open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .clone();
        assert_eq!(cell.review, Review::None);
        assert_eq!(
            cell.value,
            Some(money("120.5")),
            "clearing ✓ keeps the value"
        );
    }

    #[test]
    fn reviewing_a_to_fill_gap_keeps_the_value_none_never_zero() {
        // Setting a tag on a never-entered optional column materializes a to-fill cell carrying the
        // tag — the value stays None (the project's most-repeated rail: unknown is never 0).
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        state
            .set_review(id, 2, entry::FIELD_DIVIDEND, Review::ToReview)
            .unwrap();
        let cell = open_state(&path).get_study(id).unwrap().years[2]
            .dividend_per_share
            .clone()
            .expect("the cell now exists");
        assert_eq!(cell.review, Review::ToReview);
        assert_eq!(cell.value, None, "a reviewed gap holds no value — never 0");
        assert_eq!(cell.coverage, Coverage::ToFill);
    }

    #[test]
    fn an_empty_cell_cannot_be_validated_issue_47() {
        // #47: validating a value-less cell must be refused (a neutral no-op) — otherwise a later
        // refresh gap-fills it, resets the review to None, and the ✓ vanishes silently (escaping the
        // ✓→? re-validate count). `?` on a gap stays allowed (flag a column to fill).
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        // A never-touched optional column: `?` materializes a to-fill gap (allowed)…
        state
            .set_review(id, 2, entry::FIELD_DIVIDEND, Review::ToReview)
            .unwrap();
        // …but `✓` on the still-empty cell is refused — review stays `?`, value stays None.
        state
            .set_review(id, 2, entry::FIELD_DIVIDEND, Review::Validated)
            .unwrap();
        let cell = open_state(&path).get_study(id).unwrap().years[2]
            .dividend_per_share
            .clone()
            .expect("the gap cell exists");
        assert_eq!(
            cell.review,
            Review::ToReview,
            "an empty cell cannot reach ✓ — the validate is a no-op"
        );
        assert_eq!(cell.value, None, "still no value — never materialized to 0");

        // Validating a never-touched column (cell does not exist yet) is likewise refused: it must
        // not materialize a Validated empty gap (the same bug, via materialization).
        state
            .set_review(id, 1, entry::FIELD_DIVIDEND, Review::Validated)
            .unwrap();
        assert!(
            open_state(&path).get_study(id).unwrap().years[1]
                .dividend_per_share
                .is_none(),
            "validating a never-touched empty column materializes nothing"
        );
    }

    #[test]
    fn a_validated_cell_is_soft_locked_until_the_tag_is_cleared() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();

        // A direct typed edit on the ✓ cell is refused with the neutral soft-lock notice, and the
        // on-disk value is unchanged (never silently blanked or overwritten).
        assert_eq!(
            state.edit_cell(id, 0, entry::FIELD_HIGH, Some(money("999"))),
            Err(MSG_SOFT_LOCKED.to_string())
        );
        let cell = open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .clone();
        assert_eq!(
            cell.value,
            Some(money("120.5")),
            "the refused edit wrote nothing"
        );
        assert_eq!(cell.review, Review::Validated, "the sign-off is intact");

        // The deliberate clear-✓ → ? releases the lock (recheck status preserved, not blanked).
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::ToReview)
            .unwrap();
        // Now the cell edits normally again.
        state
            .edit_cell(id, 0, entry::FIELD_HIGH, Some(money("130")))
            .expect("a ? cell edits normally");
        let cell = open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .clone();
        assert_eq!(cell.value, Some(money("130")));
        assert_eq!(cell.review, Review::ToReview, "editing a ? cell keeps ?");
    }

    #[test]
    fn set_not_available_is_refused_on_a_validated_cell_so_the_sign_off_is_never_blanked() {
        // The not-available gesture (Ctrl+Space) is a value/coverage mutation; on a `✓` cell it would
        // otherwise route through `Cell::edited(None, …)`, blanking the value AND demoting `✓ → ?`.
        // AC 2 forbids that — the soft-lock backstop must refuse it just like a typed edit does.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
        assert_eq!(
            state.set_not_available(id, 0, entry::FIELD_HIGH, true),
            Err(MSG_SOFT_LOCKED.to_string()),
            "not-available on a ✓ cell is refused"
        );
        // The on-disk cell is untouched: value kept, sign-off intact, still a present cell.
        let cell = open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .clone();
        assert_eq!(cell.value, Some(money("120.5")), "value untouched");
        assert_eq!(cell.review, Review::Validated, "sign-off intact");
        assert_eq!(cell.coverage, Coverage::Present, "coverage untouched");
    }

    #[test]
    fn unlock_all_flips_only_validated_cells_at_each_scope_and_persists() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        // Build a mixed field of tags across years/fields:
        //   (y0, A) ✓   (y1, A) ✓   (y0, C) ?   (y2, B) ✓
        state
            .edit_cell(id, 1, entry::FIELD_HIGH, Some(money("1")))
            .unwrap();
        state
            .edit_cell(id, 0, entry::FIELD_EPS, Some(money("2")))
            .unwrap();
        state
            .edit_cell(id, 2, entry::FIELD_LOW, Some(money("3")))
            .unwrap();
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
        state
            .set_review(id, 1, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
        state
            .set_review(id, 0, entry::FIELD_EPS, Review::ToReview)
            .unwrap();
        state
            .set_review(id, 2, entry::FIELD_LOW, Review::Validated)
            .unwrap();

        // ── Per-metric scope (field A): flips (y0,A) and (y1,A) only; (y2,B) ✓ is left.
        assert_eq!(
            state.count_validated(id, &UnlockScope::Metric(entry::FIELD_HIGH.to_string())),
            2
        );
        let flipped = state
            .unlock_all(id, &UnlockScope::Metric(entry::FIELD_HIGH.to_string()))
            .unwrap();
        assert_eq!(flipped, 2, "two A cells flipped");
        let back = open_state(&path).get_study(id).unwrap();
        assert_eq!(back.years[0].high_price.review, Review::ToReview);
        assert_eq!(back.years[1].high_price.review, Review::ToReview);
        assert_eq!(
            back.years[0].eps.review,
            Review::ToReview,
            "the ? cell is untouched"
        );
        assert_eq!(
            back.years[2].low_price.review,
            Review::Validated,
            "a different metric keeps its ✓"
        );

        // ── Per-year scope (year 2): flips (y2,B) only.
        let flipped = state.unlock_all(id, &UnlockScope::Year(2)).unwrap();
        assert_eq!(flipped, 1);
        assert_eq!(
            open_state(&path).get_study(id).unwrap().years[2]
                .low_price
                .review,
            Review::ToReview
        );

        // ── Study scope on an already-cleared study: nothing left to flip.
        assert_eq!(state.unlock_all(id, &UnlockScope::Study).unwrap(), 0);
    }

    #[test]
    fn unlock_all_study_scope_flips_every_validated_cell_in_one_upsert() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);
        state
            .edit_cell(id, 3, entry::FIELD_EPS, Some(money("9")))
            .unwrap();
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
        state
            .set_review(id, 3, entry::FIELD_EPS, Review::Validated)
            .unwrap();

        assert_eq!(state.count_validated(id, &UnlockScope::Study), 2);
        let flipped = state.unlock_all(id, &UnlockScope::Study).unwrap();
        assert_eq!(flipped, 2);
        let back = open_state(&path).get_study(id).unwrap();
        assert_eq!(back.years[0].high_price.review, Review::ToReview);
        assert_eq!(back.years[3].eps.review, Review::ToReview);
        assert_eq!(
            back.years[0].high_price.value,
            Some(money("120.5")),
            "values untouched"
        );
    }

    #[test]
    fn unlock_messages_substitute_the_count() {
        assert_eq!(
            unlock_confirm_message(3),
            "Cette action retire la validation de 3 cellule(s)."
        );
        assert_eq!(
            unlock_done_message(1),
            "Validation retirée de 1 cellule(s)."
        );
    }

    // ── Story 2.6: numeric judgment editing → persist → reopen; snapshot_for engine wiring ──

    #[test]
    fn judgment_fields_round_trip_and_clear_to_none_never_zero() {
        use steadyinvest_contract::ForecastLowOption;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);

        // Set each numeric judgment field; the option selector; then reopen and verify each survives.
        for (field, value) in [
            ("sales_growth", "12.5"),
            ("eps_growth", "10"),
            ("est_high_eps", "8.4"),
            ("est_low_eps", "3.1"),
            ("high_pe", "22"),
            ("low_pe", "11"),
            ("recent_severe_low", "44.5"),
            ("current_price", "60"),
            ("dividend", "2.25"),
        ] {
            state
                .set_judgment_field(id, field, Some(money(value)))
                .unwrap();
        }
        state
            .set_forecast_low_option(id, ForecastLowOption::RecentSevereLow)
            .unwrap();

        let j = open_state(&path).get_study(id).unwrap().judgment;
        assert_eq!(j.projected_sales_growth_pct, Some(money("12.5")));
        assert_eq!(j.estimated_high_eps, Some(money("8.4")));
        assert_eq!(j.judged_avg_high_pe, Some(money("22")));
        assert_eq!(j.current_price, Some(money("60")));
        assert_eq!(j.present_full_year_dividend, Some(money("2.25")));
        assert_eq!(j.forecast_low_option, ForecastLowOption::RecentSevereLow);

        // Clearing a field stores None — never 0.
        state.set_judgment_field(id, "current_price", None).unwrap();
        let j = open_state(&path).get_study(id).unwrap().judgment;
        assert_eq!(
            j.current_price, None,
            "a cleared judgment field is None, never 0"
        );
    }

    #[test]
    fn snapshot_for_runs_the_engine_and_matches_build_snapshot() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);
        // Fill the load-bearing cells + judgment so the snapshot is computable.
        for y in 0..entry::YEAR_WINDOW {
            for (field, v) in [
                (entry::FIELD_HIGH, "100"),
                (entry::FIELD_LOW, "50"),
                (entry::FIELD_EPS, "5"),
            ] {
                state.edit_cell(id, y, field, Some(money(v))).unwrap();
            }
        }
        for (field, v) in [
            ("est_high_eps", "8"),
            ("est_low_eps", "3"),
            ("high_pe", "20"),
            ("low_pe", "10"),
            ("current_price", "60"),
        ] {
            state.set_judgment_field(id, field, Some(money(v))).unwrap();
        }

        let snap = state.snapshot_for(id).expect("snapshot computes");
        // No drift: the state-level snapshot equals the pure adapter snapshot on the same study.
        let study = state.get_study(id).unwrap();
        let direct = crate::viewmodel::engine::build_snapshot(&study).unwrap();
        assert_eq!(snap.outputs(), direct.outputs());
        assert_eq!(snap.verdict(), direct.verdict());
    }

    #[test]
    fn an_edit_on_a_read_only_journal_is_refused_and_writes_nothing() {
        // A study created in a writable journal, then reopened read-only: the edit is refused with the
        // neutral notice and the on-disk value is unchanged.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xC),
                &Timestamp("2026-06-13T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
        let id = {
            let mut state = open_state(&path);
            state.create_study("NESN", "CHF").unwrap()
        };
        // Force a read-only state by constructing one whose `read_only` flag is set.
        let mut state = open_state(&path);
        state.read_only = true;
        assert_eq!(
            state.edit_cell(id, 0, entry::FIELD_HIGH, Some(money("1"))),
            Err(MSG_READ_ONLY_WRITE.to_string())
        );
        // Nothing was written: the cell is still absent/empty.
        let back = open_state(&path).get_study(id).unwrap();
        assert!(back.years.is_empty(), "a refused edit materialized nothing");
    }

    // ── Story 2.12 — dashboard archive (soft) & delete (hard) state wrappers ──

    fn status_in_list(state: &JournalState, id: Uuid) -> Option<String> {
        state
            .list_studies()
            .into_iter()
            .find(|s| s.id == id)
            .map(|s| s.status)
    }

    #[test]
    fn archive_then_unarchive_flips_status_reversibly() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);
        assert_eq!(status_in_list(&state, id).as_deref(), Some("active"));

        state.archive_study(id).expect("archive");
        assert_eq!(status_in_list(&state, id).as_deref(), Some("archived"));

        state.unarchive_study(id).expect("un-archive");
        assert_eq!(status_in_list(&state, id).as_deref(), Some("active"));
    }

    #[test]
    fn delete_removes_the_study_from_the_list() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let (mut state, id) = study_with_entry(&path);
        assert!(
            status_in_list(&state, id).is_some(),
            "present before delete"
        );

        state.delete_study(id).expect("delete");
        assert!(
            status_in_list(&state, id).is_none(),
            "the deleted study is gone from the list"
        );
        assert!(
            state.get_study(id).is_none(),
            "the deleted study is unreadable"
        );
    }

    #[test]
    fn archive_and_delete_are_refused_on_a_read_only_journal() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("journal.db");
        let id = study_with_entry(&path).1;

        let mut state = open_state(&path);
        state.read_only = true;
        assert_eq!(
            state.archive_study(id),
            Err(MSG_READ_ONLY_WRITE.to_string())
        );
        assert_eq!(state.delete_study(id), Err(MSG_READ_ONLY_WRITE.to_string()));
        // Nothing changed on disk: the study is still present and active.
        assert_eq!(
            status_in_list(&open_state(&path), id).as_deref(),
            Some("active"),
            "a refused archive/delete mutated nothing"
        );
    }
}
