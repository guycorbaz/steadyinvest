//! User-facing neutral notices + their substitution helpers (FR13). One `MSG_*` const per notice,
//! French (the UI source language), all fact-stating; the crate-local posture gate scans the
//! test-only [`USER_FACING_MESSAGES`] inventory for banned verbs, exactly like the `@tr()` literals
//! — keep it in sync with the consts. The `{n}`-substitution helpers keep the scanned const and the
//! runtime string one source.

use steadyinvest_persistence::ImportSummary;

use super::{RefreshReport, RestoreAssessment, RestoreVerdict};

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
/// Shown when the chosen provider is "Aucun" (Story 7.4 review) — distinct from a missing key, so the
/// user is told there is no provider to fetch from rather than a misleading "invalid key".
pub const MSG_PROVIDER_NONE: &str =
    "Aucun fournisseur de données n'est sélectionné ; la récupération n'a pas eu lieu.";
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
/// Raised when a holding currency is not one of the supported set (Story 6.2, FR38); nothing is
/// written. A defensive guard — the UI only offers allow-list currencies.
pub const MSG_HOLDING_INVALID_CURRENCY: &str =
    "La devise doit faire partie des devises prises en charge ; aucune position n'a été enregistrée.";

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

/// Multiple-portfolio copy (Story 6.1, FR37) — fact-stating, posture-gated. The name guard, and the
/// two guarded-delete refusals (the register never orphans a holding nor drops its last portfolio).
pub const MSG_PORTFOLIO_INVALID_NAME: &str =
    "Le nom du portefeuille est vide ; aucun portefeuille n'a été créé.";
pub const MSG_PORTFOLIO_HAS_HOLDINGS: &str =
    "Ce portefeuille contient un historique de positions ; il n'a pas été supprimé.";
pub const MSG_PORTFOLIO_LAST: &str = "C'est le dernier portefeuille ; il n'a pas été supprimé.";

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
    MSG_PROVIDER_NONE,
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
    MSG_HOLDING_INVALID_CURRENCY,
    MSG_HOLDING_INVALID_STOP,
    MSG_HOLDINGS_REFRESHING,
    MSG_HOLDINGS_REFRESH_NONE,
    MSG_HOLDING_SOLD,
    MSG_PORTFOLIO_INVALID_NAME,
    MSG_PORTFOLIO_HAS_HOLDINGS,
    MSG_PORTFOLIO_LAST,
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
