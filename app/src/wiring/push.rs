//! Shared UI pushers — the single sources of truth that mirror journal/view state into the
//! generated Slint globals, used by several wiring domains: `push_form` (the faithful open-form
//! rebuild, Stories 2.3/2.4/2.6), `push_live_preview` (the §1 drag's non-persisted recompute,
//! Story 2.8 / NFR-P1), `push_view_state` (fold/regime restore + toggles), and the holdings
//! freshness `display_timestamp` (Story 4.4). Moved verbatim from `main.rs` — no logic change.

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::config::StudyViewState;
use crate::state::JournalState;
use crate::viewmodel::format::NumberFormat;
use crate::{
    GrowthComputed, MainWindow, MgmtComputed, PeComputed, ReturnComputed, RiskComputed, Studies,
    VerdictState, ZoneBarState,
};
use crate::{regime, state, viewmodel};

/// A compact display form of an RFC3339 timestamp for the holdings freshness caption (Story 4.4):
/// `YYYY-MM-DD HH:MM` (drop seconds + zone; the journal stores the full RFC3339 string).
pub(crate) fn display_timestamp(ts: &steadyinvest_contract::Timestamp) -> String {
    let s = &ts.0;
    s.get(..16).unwrap_or(s).replacen('T', " ", 1)
}

/// Mirror a per-study view-state (regime + fold flags) into the `Studies` global and swap the
/// regime-driven token snapshot. The single place the UI's fold/regime state is pushed, so the
/// open-study restore and the toggle/regime callbacks stay consistent (one source of truth).
pub(crate) fn push_view_state(ui: &MainWindow, view_state: &StudyViewState) {
    let studies = ui.global::<Studies>();
    studies.set_regime(view_state.regime.as_str().into());
    studies.set_folds(ModelRc::new(VecModel::from(view_state.folds.to_vec())));
    regime::apply(ui, view_state.regime);
}

/// Rebuild the faithful-form structs from a (re-read) `Study` and push them into the `Studies`
/// global — the single source of truth for the open form (Story 2.3 header + §3 rows, Story 2.4 §2
/// management grid + year headers, **Story 2.6 the engine outputs + judgment inputs + §4 zone bar +
/// verdict**). Called on open and after every persisted edit so the UI always renders exactly what is
/// on disk + the coherent snapshot recomputed from it. Money/ratios cross only as formatted strings
/// (the adapter boundary); the verdict crosses as an enum-derived string.
///
/// The engine call goes through the single construction path [`engine::build_snapshot`] (ONE
/// `StudySnapshot::new`), so the §2–§5 results, the §4 zone bar and the verdict are always one
/// coherent frame. A `NormalizeError` (unreachable from a well-formed manual study, but handled, never
/// `unwrap`) surfaces as a neutral notice and leaves every computed slot the faithful em-dash.
pub(crate) fn push_form(
    ui: &MainWindow,
    state: &JournalState,
    study: &steadyinvest_contract::Study,
    format: NumberFormat,
) {
    use viewmodel::engine;
    let studies = ui.global::<Studies>();
    // Story 2.9 — mirror undo/redo availability so the header controls enable/disable in step with
    // every persisted edit (an edit grows undo + clears redo; undo/redo move between the stacks).
    studies.set_can_undo(state.can_undo());
    studies.set_can_redo(state.can_redo());
    studies.set_form_header(viewmodel::form::header(study));
    studies.set_year_headers(ModelRc::new(VecModel::from(viewmodel::form::year_headers(
        study,
    ))));
    // The current judgment-input values (restored on reopen; "" for a cleared input, never "0").
    studies.set_judgment(engine::judgment_fields(study, format));
    // Story 2.10 — the study-level decision rationale (FR49), restored on reopen; "" when unset
    // (the note re-seeds from this only while it does NOT have focus, the keep-input discipline).
    studies.set_rationale(study.rationale.clone().unwrap_or_default().into());

    let years = viewmodel::form::materialized_year_numbers(study);
    match engine::build_frame(study) {
        Ok(frame) => {
            let snapshot = &frame.snapshot;
            let outputs = snapshot.outputs();
            // Story 2.7 — map BOTH finding sets (input-shape off the frame + calc-time off the
            // outputs) to per-cell / study-level warnings against the SAME materialized window the
            // grids render, so the verdict and the warnings descend from one coherent frame.
            let warnings = engine::plausibility(&frame.plausibility, &outputs.findings, &years);
            studies.set_pe_rows(ModelRc::new(VecModel::from(viewmodel::form::pe_rows(
                study,
                format,
                Some(outputs),
                &warnings,
            ))));
            studies.set_mgmt_rows(ModelRc::new(VecModel::from(viewmodel::form::mgmt_rows(
                study, format, &warnings,
            ))));
            studies.set_growth_computed(engine::growth_computed(outputs, format));
            studies.set_mgmt_computed(engine::mgmt_computed(outputs, &years, format));
            studies.set_pe_computed(engine::pe_computed(outputs, format));
            studies.set_risk_computed(engine::risk_computed(outputs, format));
            studies.set_return_computed(engine::return_computed(outputs, format));
            studies.set_zone_bar(engine::zone_bar(study, snapshot, format));
            studies.set_verdict(engine::verdict_badge(study, snapshot, format));
            // Issue #114: the load-bearing judgment inputs still to fill (drives the field highlight).
            studies.set_required_fields(ModelRc::new(VecModel::from(
                engine::required_judgment_fields(snapshot),
            )));
            // Story 2.8 — the §1 interactive growth chart geometry (from the SAME coherent frame).
            studies.set_growth_chart(viewmodel::chart::growth_chart(&frame, format));
            // Issue #115 — the §3 P/E-history chart (historical high/low P/E + draggable judged levels).
            studies.set_pe_chart(viewmodel::chart::pe_chart(&frame, &study.judgment, format));
            // The study-level (§4) warning key — `low_price_above_current`, anchored near forecast-low.
            studies.set_section4_warning_key(
                warnings
                    .study_key()
                    .map(|k| k.as_str())
                    .unwrap_or("")
                    .into(),
            );
        }
        Err(error) => {
            // Degraded-but-safe: the form still renders, every computed slot the em-dash; the verdict
            // and zone bar fall back to their calm empty states; no warning channel speaks.
            tracing::warn!("snapshot normalize failed: {error}");
            let no_warnings = engine::PlausibilityWarnings::default();
            studies.set_pe_rows(ModelRc::new(VecModel::from(viewmodel::form::pe_rows(
                study,
                format,
                None,
                &no_warnings,
            ))));
            studies.set_mgmt_rows(ModelRc::new(VecModel::from(viewmodel::form::mgmt_rows(
                study,
                format,
                &no_warnings,
            ))));
            studies.set_growth_computed(GrowthComputed::default());
            studies.set_mgmt_computed(MgmtComputed::default());
            studies.set_pe_computed(PeComputed::default());
            studies.set_risk_computed(RiskComputed::default());
            studies.set_return_computed(ReturnComputed::default());
            studies.set_zone_bar(ZoneBarState::default());
            studies.set_verdict(VerdictState::default());
            studies.set_growth_chart(viewmodel::chart::unavailable());
            studies.set_pe_chart(viewmodel::chart::pe_chart_unavailable());
            studies.set_section4_warning_key(SharedString::new());
            studies.set_notice(state::MSG_NORMALIZE_FAILED.into());
        }
    }

    // Issue #34 (FR51, PR 2): an OPEN « Historique » panel re-syncs with every persisted edit —
    // the timeline must never sit stale beside the fresh form. A closed panel costs nothing.
    if ui.global::<Studies>().get_history_open() {
        push_history(ui, state, study.id, format);
    }
}

/// Rebuild the « Historique » timeline rows for a study (issue #34, FR51 — PR 2): list the
/// snapshot series, load each state, and push the newest-first day-grouped entries. A read
/// failure ANYWHERE (listing or a payload) marks the panel « indisponible » (the #95 discipline —
/// never an empty-looking timeline over a failure); an old study's truly empty series renders the
/// honest empty state. The expanded detail is collapsed on every rebuild (its entry may have
/// shifted); the user re-expands at will.
pub(crate) fn push_history(
    ui: &MainWindow,
    state: &JournalState,
    study_id: uuid::Uuid,
    format: NumberFormat,
) {
    let studies = ui.global::<Studies>();
    let loaded = (|| -> Result<Vec<(uuid::Uuid, String, steadyinvest_contract::Study)>, String> {
        let summaries = state.try_list_study_history(study_id)?;
        let mut loaded = Vec::with_capacity(summaries.len());
        for summary in summaries {
            let study = state
                .try_get_history_snapshot(summary.id)?
                // Vanished between the list and the read — a failure, not an absence.
                .ok_or_else(String::new)?;
            loaded.push((summary.id, summary.created_at.0, study));
        }
        Ok(loaded)
    })();
    match loaded {
        Ok(loaded) => {
            let rows: Vec<crate::HistoryEntryRow> =
                viewmodel::history::history_entries(&loaded, format)
                    .into_iter()
                    .map(|e| crate::HistoryEntryRow {
                        id: e.id.to_string().into(),
                        day: e.day.into(),
                        first_of_day: e.first_of_day,
                        time: e.time.into(),
                        summary: e.summary.into(),
                    })
                    .collect();
            studies.set_history_unavailable(false);
            studies.set_history_rows(ModelRc::new(VecModel::from(rows)));
        }
        Err(_) => {
            studies.set_history_unavailable(true);
            studies.set_history_rows(ModelRc::new(VecModel::from(
                Vec::<crate::HistoryEntryRow>::new(),
            )));
        }
    }
    studies.set_history_detail_id(SharedString::new());
    studies.set_history_detail_lines(ModelRc::new(VecModel::from(Vec::<SharedString>::new())));
}

/// A LIVE, NON-persisted recompute frame for a §1 judgment-line drag (Story 2.8, NFR-P1). Builds ONE
/// coherent [`engine::build_frame`] from the in-memory (un-saved) study and pushes only the surfaces a
/// drag moves — the judgment fields (so the exact-value field mirrors the line, FR31), the §1 chart
/// line, the §4 zone bar and the verdict. Deliberately does NOT touch the journal or rebuild the whole
/// form (`push_form`'s per-edit `put_study` + full rebuild is far too heavy per `moved` event — the
/// recompute itself is sub-millisecond, the cost to avoid is the per-event write). A transient
/// normalize error mid-drag leaves the last good frame untouched (never a flash of blanked outputs).
pub(crate) fn push_live_preview(
    ui: &MainWindow,
    study: &steadyinvest_contract::Study,
    format: NumberFormat,
) {
    use viewmodel::engine;
    let studies = ui.global::<Studies>();
    if let Ok(frame) = engine::build_frame(study) {
        let snapshot = &frame.snapshot;
        let outputs = snapshot.outputs();
        let years = viewmodel::form::materialized_year_numbers(study);
        let warnings = engine::plausibility(&frame.plausibility, &outputs.findings, &years);
        studies.set_judgment(engine::judgment_fields(study, format));
        studies.set_growth_chart(viewmodel::chart::growth_chart(&frame, format));
        // Issue #115 — the §3 P/E line moves live too (a P/E drag or an est-EPS drag both recompute it).
        studies.set_pe_chart(viewmodel::chart::pe_chart(&frame, &study.judgment, format));
        studies.set_zone_bar(engine::zone_bar(study, snapshot, format));
        studies.set_verdict(engine::verdict_badge(study, snapshot, format));
        // §4/§5 judgment-dependent numbers stay in step with the recolouring bar (review P1) — the
        // forecast high/low + U/D, the projected return, and the §4 study-level warning all move
        // with the est-high-EPS the drag sets, so the §4 surface never disagrees with itself.
        studies.set_risk_computed(engine::risk_computed(outputs, format));
        studies.set_return_computed(engine::return_computed(outputs, format));
        studies.set_section4_warning_key(
            warnings
                .study_key()
                .map(|k| k.as_str())
                .unwrap_or("")
                .into(),
        );
    }
}
