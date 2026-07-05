//! Replacement-candidates wiring (Story 6.8, FR48): the portfolio-level pushed panel — built
//! from the pure `state::replacement_candidates` read, auto-opened after a successful sell
//! (both the 4.7 trigger path and the 6.3 ledger form), opened manually from a trigger's
//! « Candidats » action, session-only, cleared on journal switch, re-synced while open.

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::state::{CandidateData, JournalState};
use crate::viewmodel::format::{NumberFormat, format_scaled};
use crate::wiring::Session;
use crate::{CandidateRow, Holdings, MainWindow, Prefs, Studies};

/// Build + push the candidate rows (display strings via the locale path; the murmurs use the
/// 6.7 threshold band — an absent share never flags). Reads the format and threshold from
/// their UI mirrors so every caller (sell hooks, sync sites) stays signature-light.
fn push_candidates(ui: &MainWindow, state: &JournalState) {
    use steadyinvest_core::rounding::DisplayField;
    let holdings = ui.global::<Holdings>();
    let reference = holdings.get_reference_currency().to_string();
    let format = NumberFormat::parse(&ui.global::<Prefs>().get_number_format()).unwrap_or_default();
    let threshold =
        rust_decimal::Decimal::from_str_exact(holdings.get_concentration_threshold_pct().as_ref())
            .or_else(|_| {
                rust_decimal::Decimal::from_str_exact(
                    crate::config::DEFAULT_CONCENTRATION_THRESHOLD_PCT,
                )
            })
            .unwrap_or_default();
    let fmt_pct = |d: rust_decimal::Decimal| format_scaled(d, DisplayField::Percent, format);
    // A zero exposure is never a near-breach — the murmur fires on POSITIVE present shares
    // only (2026-07-03 review: a ≤10 threshold floors the band at 0).
    let flagged = |share: Option<rust_decimal::Decimal>| {
        share.is_some_and(|s| {
            s > rust_decimal::Decimal::ZERO
                && steadyinvest_core::risk::concentration_flagged(s, threshold)
        })
    };
    // A failed watchlist read is « indisponible », never « liste vide » (the 6.6/6.7 IO rule).
    let Some(candidates) = state.replacement_candidates(&reference) else {
        holdings.set_candidate_rows(ModelRc::new(VecModel::from(Vec::<CandidateRow>::new())));
        holdings.set_candidates_unavailable(true);
        holdings.set_candidates_rates(SharedString::new());
        return;
    };
    holdings.set_candidates_unavailable(false);
    // The FR28 footnote for the exposure shares (rate + date + source, pure data entries).
    holdings.set_candidates_rates(
        state
            .journal_currency_exposure(&reference)
            .map(|e| {
                e.rates_used
                    .iter()
                    .map(|r| {
                        format!(
                            "{} → {} {} ({}, {})",
                            r.base_currency, r.quote_currency, r.rate, r.rate_date, r.source
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_default()
            .into(),
    );
    let rows: Vec<CandidateRow> = candidates
        .into_iter()
        .map(|c| CandidateRow {
            ticker: c.ticker.into(),
            study_id: c
                .study_id
                .map(|id| id.to_string())
                .unwrap_or_default()
                .into(),
            data: match c.data {
                CandidateData::Ok => "ok",
                CandidateData::Insufficient => "insufficient",
                CandidateData::NoStudy => "no-study",
            }
            .into(),
            zone: c.zone_key.into(),
            in_buy_zone: c.in_buy_zone,
            distance: c
                .distance_above_buy_pct
                .map(&fmt_pct)
                .unwrap_or_default()
                .into(),
            // The same "N:1" rendering as the §4 U/D slot; absent states stay absent.
            ud: c
                .ud_ratio
                .map(|r| format!("{}:1", format_scaled(r, DisplayField::Ratio, format)))
                .unwrap_or_default()
                .into(),
            currency: c.currency.unwrap_or_default().into(),
            held_flagged: flagged(c.held_share_pct),
            held_share: c.held_share_pct.map(&fmt_pct).unwrap_or_default().into(),
            currency_flagged: flagged(c.currency_share_pct),
            currency_share: c
                .currency_share_pct
                .map(&fmt_pct)
                .unwrap_or_default()
                .into(),
            currency_missing: c.currency_missing_pair.unwrap_or_default().into(),
        })
        .collect();
    holdings.set_candidate_rows(ModelRc::new(VecModel::from(rows)));
}

/// Open the panel with `context` as its heading ticker. `sold` is true ONLY from the two sell
/// hooks — the « Vente enregistrée » header must never assert a sale that did not happen
/// (2026-07-03 review: the trigger « Candidats » opens BEFORE deciding).
pub(crate) fn open_candidates(ui: &MainWindow, state: &JournalState, context: &str, sold: bool) {
    push_candidates(ui, state);
    let holdings = ui.global::<Holdings>();
    holdings.set_candidates_context(context.into());
    holdings.set_candidates_sold(sold);
    holdings.set_candidates_visible(true);
}

/// Re-sync an OPEN panel after a holdings/watchlist mutation (the `sync_ledger_panel`
/// precedent) — a closed panel costs nothing.
pub(crate) fn sync_candidates(ui: &MainWindow, state: &JournalState) {
    if ui.global::<Holdings>().get_candidates_visible() {
        push_candidates(ui, state);
    }
}

/// Clear the panel entirely — « Fermer », and every journal switch/open/restore site (stale
/// cross-journal candidates must not survive, the 6.6 FX-panel lesson).
pub(crate) fn clear_candidates(ui: &MainWindow) {
    let holdings = ui.global::<Holdings>();
    holdings.set_candidates_visible(false);
    holdings.set_candidates_context(SharedString::new());
    holdings.set_candidates_sold(false);
    holdings.set_candidate_rows(ModelRc::new(VecModel::from(Vec::<CandidateRow>::new())));
    holdings.set_candidates_rates(SharedString::new());
    holdings.set_candidates_unavailable(false);
}

/// Wire the panel intents: manual open (the trigger « Candidats »), close, and the two
/// hand-offs — « Ouvrir l'étude » navigates to Études and drives the EXISTING open rail
/// (`invoke_open_study` — undo reset, push_form, study-open: one code path); « Études »
/// lands on the list/create form (study-open cleared, the nav-rail gesture).
pub(crate) fn wire_replacement(ui: &MainWindow, s: &Session) {
    let Session { journal_state, .. } = s;
    {
        let ui_weak = ui.as_weak();
        let journal_state = std::rc::Rc::clone(journal_state);
        ui.global::<Holdings>().on_open_candidates(move |ticker| {
            let ui = ui_weak.unwrap();
            open_candidates(&ui, &journal_state.borrow(), &ticker, false);
        });
    }
    {
        let ui_weak = ui.as_weak();
        ui.global::<Holdings>().on_close_candidates(move || {
            clear_candidates(&ui_weak.unwrap());
        });
    }
    {
        let ui_weak = ui.as_weak();
        ui.global::<Holdings>()
            .on_open_candidate_study(move |study_id| {
                let ui = ui_weak.unwrap();
                ui.set_current_screen(0);
                ui.global::<Studies>().invoke_open_study(study_id);
            });
    }
    {
        let ui_weak = ui.as_weak();
        ui.global::<Holdings>().on_go_to_studies(move || {
            let ui = ui_weak.unwrap();
            ui.global::<Studies>().set_study_open(false);
            ui.set_current_screen(0);
        });
    }
}
