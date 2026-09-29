//! The open study's pending AI judgment proposals on the §1 / §3 charts and fields, and the
//! « placée par l'IA » captions of validated ones (Story 8.6 — FR33, FR72; UX spec §5.5, Q13).
//!
//! Pure and read-only over `&Study` + the inbox's pending drafts: nothing here is persisted, and the
//! owner chart geometry is never touched — the AI lines are computed on the SAME scale
//! ([`chart::eps_scale`] / [`chart::pe_scale`]) and the proposal never enters it (AC 4). An implied
//! est-high (a projected EPS-growth proposal) is computed on a local CLONE of the study through the
//! app's one engine path and dropped after drawing.
//!
//! No AI-written text: the chips and labels carry the field's French label and the proposed value
//! parsed and formatted by the app — never the draft's comment or origin (Q15).

use std::collections::HashMap;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use steadyinvest_contract::{
    DraftField, DraftFieldKind, DraftKind, DraftPayload, DraftStatus, DraftTarget, DraftValue,
    Study,
};
use steadyinvest_core::rounding::{DisplayField, round_for_display};
use steadyinvest_persistence::DraftRecord;
use uuid::Uuid;

use crate::state::{DraftFreshness, draft_freshness};
use crate::viewmodel::chart::{self, AiGeometry};
use crate::viewmodel::drafts::{field_label, judgment_label_display, same_decimals, value_display};
use crate::viewmodel::engine::{self, StudyFrame};
use crate::viewmodel::format::{NumberFormat, format_amount, format_scaled};
use crate::viewmodel::notes::date_fr;

/// The chip's field label of the forecast-low option (spec §5.5 — « ★ IA · Option du bas
/// prévisionnel : {libellé} »).
pub const AI_CHIP_OPTION_FIELD: &str = "Option du bas prévisionnel";

/// Every app string of the AI judgment surfaces built in Rust, scanned by the posture gate (FR13).
#[cfg(test)]
pub const AI_LINES_USER_FACING_LABELS: &[&str] = &[AI_CHIP_OPTION_FIELD];

/// One pending judgment proposal of the open study, parsed and labelled (AC 1–3).
#[derive(Debug, Clone, PartialEq)]
pub struct JudgmentOverlay {
    pub field: DraftField,
    pub draft_id: Uuid,
    pub value: DraftValue,
    /// The chip's « {champ} » — app text.
    pub field_label: String,
    /// The chip's « {valeur} » — app-formatted (the owner's number format, the field's scale).
    pub value_label: String,
    /// The target changed since the proposal (the chip ends « · périmée »).
    pub stale: bool,
}

/// The pending JUDGMENT proposals of `study` among `drafts` (the inbox's pending read), one per
/// field. A proposal that does not read (payload, target, value) is dropped and logged — never a
/// guessed value. Two pending proposals on one field (a data error: D4 refuses the second at
/// submission) drop that field's overlay — the inbox still lists both (AC 3).
pub fn pending_judgment_overlays(
    study: &Study,
    drafts: &[DraftRecord],
    format: NumberFormat,
) -> Vec<JudgmentOverlay> {
    let mut by_field: HashMap<DraftField, Vec<JudgmentOverlay>> = HashMap::new();
    for d in drafts.iter().filter(|d| {
        d.kind == DraftKind::Judgment
            && d.status == DraftStatus::Pending
            && d.study_id == Some(study.id)
    }) {
        match overlay_of(study, d, format) {
            Ok(o) => by_field.entry(o.field).or_default().push(o),
            Err(why) => tracing::warn!("judgment draft {} not drawn: {why}", d.id),
        }
    }
    let mut out: Vec<JudgmentOverlay> = Vec::new();
    for (field, mut list) in by_field {
        if list.len() > 1 {
            tracing::warn!(
                "{} pending judgment drafts on {} of study {}: the field's AI overlay is dropped",
                list.len(),
                field.key(),
                study.id
            );
            continue;
        }
        out.append(&mut list);
    }
    // A stable order (the registry's) — the chip rows never reshuffle between reads.
    out.sort_by_key(|o| DraftField::ALL.iter().position(|f| *f == o.field));
    out
}

fn overlay_of(
    study: &Study,
    d: &DraftRecord,
    format: NumberFormat,
) -> Result<JudgmentOverlay, &'static str> {
    let payload: DraftPayload = serde_json::from_str(&d.payload).map_err(|error| {
        tracing::debug!(%error, "judgment draft {}: payload", d.id);
        "payload does not parse"
    })?;
    let target = payload.target.as_ref().ok_or("no target")?;
    if !matches!(target, DraftTarget::Judgment { .. }) {
        return Err("not a judgment target");
    }
    let (field, _) = DraftField::of_target(target).ok_or("unknown judgment field")?;
    if field.kind() != DraftFieldKind::Judgment {
        return Err("not a judgment field");
    }
    let text = payload
        .proposed_value
        .as_deref()
        .ok_or("no proposed value")?;
    let value = field.parse_value(text).map_err(|problem| {
        tracing::debug!(?problem, "judgment draft {}: {}", d.id, field.key());
        "proposed value is no value of its field"
    })?;
    let stale =
        draft_freshness(Some(study), DraftKind::Judgment, &payload) != DraftFreshness::Fresh;
    let current = value_display(field, field.judgment_value(&study.judgment), format);
    // The field's display rounding must never make a proposal read as another value (« 20,0 » for
    // a proposed 20.04 beside the owner's 20,0): a proposal finer than the display scale shows its
    // exact digits.
    let proposed = match (value, judgment_label_display(field)) {
        (DraftValue::Number(m), Some((_, Some(display))))
            if round_for_display(m.as_decimal(), display) != m.as_decimal() =>
        {
            format_amount(&m.as_decimal().normalize().to_string(), format)
        }
        _ => value_display(field, Some(value), format),
    };
    let field_label = if field == DraftField::ForecastLowOption {
        AI_CHIP_OPTION_FIELD.to_string()
    } else {
        field_label(field).to_string()
    };
    Ok(JudgmentOverlay {
        field,
        draft_id: d.id,
        value,
        field_label,
        value_label: same_decimals(&current, &proposed, format),
        stale,
    })
}

/// A pending AI line as drawn: its geometry and its label value (« IA {valeur} »).
#[derive(Debug, Clone, PartialEq)]
pub struct AiLineView {
    pub geometry: AiGeometry,
    pub value: String,
}

/// Where each pending proposal of the open study shows, and the validated captions (AC 1, 2, 6).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AiJudgmentsView {
    pub est_high: Option<AiLineView>,
    pub est_low: Option<AiLineView>,
    pub pe_high: Option<AiLineView>,
    pub pe_low: Option<AiLineView>,
    /// The « Propositions de l'IA : » row under the §1 legend.
    pub growth_chips: Vec<JudgmentOverlay>,
    /// The « Propositions de l'IA : » row under the §3 legend.
    pub pe_chips: Vec<JudgmentOverlay>,
    /// Every other proposal's chip, under its `JudgmentField` (or after the option chips).
    pub field_chips: Vec<JudgmentOverlay>,
    /// « placée par l'IA · validée le {date} » — the date per validated field.
    pub captions: Vec<(DraftField, String)>,
}

/// The EPS value a projected-EPS-growth proposal IMPLIES for the est-high line (Decision 2): the
/// engine's derived est-high on a local clone carrying the proposed growth and no direct est-high —
/// never persisted, never a verdict. `None` when it does not compute.
fn implied_est_high(study: &Study, growth: DraftValue) -> Option<Decimal> {
    let DraftValue::Number(pct) = growth else {
        return None;
    };
    let mut clone = study.clone();
    clone.judgment.projected_eps_growth_pct = Some(pct);
    clone.judgment.estimated_high_eps = None;
    let frame = engine::build_frame(&clone).ok()?;
    frame.snapshot.outputs().growth.estimated_high_eps
}

/// Place the open study's pending proposals (AC 1, 2): a chart-draggable field on its chart (with
/// its chip in the chart's row) while that chart is drawn, else its chip under its field; a
/// projected-EPS-growth proposal drawn as the est-high line it implies only when the owner has no
/// direct est-high and no est-high proposal is pending (Decision 2); every other field's chip under
/// its field. `frame`: the open study's coherent frame (`None` = its charts are not drawn).
pub fn ai_judgments(
    study: &Study,
    frame: Option<&StudyFrame>,
    overlays: &[JudgmentOverlay],
    format: NumberFormat,
) -> AiJudgmentsView {
    let mut view = AiJudgmentsView::default();
    let eps = frame.and_then(chart::eps_scale);
    let pe = frame.and_then(chart::pe_scale);
    let number = |o: &JudgmentOverlay| match o.value {
        DraftValue::Number(m) => Some(m.as_decimal()),
        DraftValue::Option(_) => None,
    };
    // A direct proposal's line label is its chip's value (same digits on the chart and in the row);
    // the implied est-high of a growth proposal is formatted as the axis unit (Decision 5).
    let eps_line = |value: Decimal, label: Option<&str>| {
        eps.as_ref().map(|s| AiLineView {
            geometry: chart::ai_growth_geometry(s, value.to_f64().unwrap_or(0.0)),
            value: label.map_or_else(
                || format_scaled(value, DisplayField::PerShare, format),
                str::to_string,
            ),
        })
    };
    let pe_line = |value: Decimal, label: &str| {
        pe.map(|axis| AiLineView {
            geometry: chart::ai_pe_geometry(axis, value.to_f64().unwrap_or(0.0)),
            value: label.to_string(),
        })
    };
    let est_high_pending = overlays
        .iter()
        .any(|o| o.field == DraftField::EstimatedHighEps);
    for o in overlays {
        let drawn = match o.field {
            DraftField::EstimatedHighEps => {
                view.est_high = number(o).and_then(|v| eps_line(v, Some(&o.value_label)));
                view.est_high.is_some()
            }
            DraftField::EstimatedLowEps => {
                view.est_low = number(o).and_then(|v| eps_line(v, Some(&o.value_label)));
                view.est_low.is_some()
            }
            DraftField::ProjectedEpsGrowthPct
                if study.judgment.estimated_high_eps.is_none() && !est_high_pending =>
            {
                view.est_high = eps
                    .as_ref()
                    .and_then(|_| implied_est_high(study, o.value))
                    .and_then(|v| eps_line(v, None));
                view.est_high.is_some()
            }
            DraftField::JudgedAvgHighPe => {
                view.pe_high = number(o).and_then(|v| pe_line(v, &o.value_label));
                view.pe_high.is_some()
            }
            DraftField::JudgedAvgLowPe => {
                view.pe_low = number(o).and_then(|v| pe_line(v, &o.value_label));
                view.pe_low.is_some()
            }
            _ => false,
        };
        match (drawn, o.field) {
            (true, DraftField::JudgedAvgHighPe | DraftField::JudgedAvgLowPe) => {
                view.pe_chips.push(o.clone())
            }
            (true, _) => view.growth_chips.push(o.clone()),
            (false, _) => view.field_chips.push(o.clone()),
        }
    }
    view.captions = ai_placed_captions(study);
    view
}

/// « placée par l'IA · validée le {JJ/MM/AAAA} » — the local date of each judgment field whose
/// `ai_placed` slot is set (AC 6). App text only (no client, no model — Decision 8).
pub fn ai_placed_captions(study: &Study) -> Vec<(DraftField, String)> {
    DraftField::ALL
        .iter()
        .filter_map(|f| {
            f.ai_slot(&study.judgment.ai_placed)
                .and_then(|slot| slot.as_ref())
                .map(|origin| (*f, date_fr(&origin.validated_at)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::{AiOrigin, Money, Timestamp};

    /// The bundled worked example (a full study: EPS history, P/E history, judgments).
    fn study() -> Study {
        crate::viewmodel::verify::demo_study().expect("the demo study")
    }

    fn draft(study_id: Uuid, field: &str, value: &str) -> DraftRecord {
        let payload = DraftPayload {
            version: steadyinvest_contract::DRAFT_PAYLOAD_VERSION,
            target: Some(DraftTarget::Judgment {
                field: field.to_string(),
            }),
            proposed_value: Some(value.to_string()),
            note_text: None,
            company_name: None,
            base_fingerprint: None,
        };
        DraftRecord {
            id: Uuid::new_v4(),
            kind: DraftKind::Judgment,
            study_id: Some(study_id),
            security_ticker: "TEST".into(),
            native_currency: None,
            status: DraftStatus::Pending,
            created_at: Timestamp("2026-09-28T10:00:00Z".into()),
            decided_at: None,
            comment: "c".into(),
            origin_client: "x".into(),
            origin_model: "y".into(),
            stale_at_decision: None,
            edited_before_validation: None,
            created_study_id: None,
            payload: serde_json::to_string(&payload).expect("payload"),
        }
    }

    /// A draft carrying the fingerprint of today's target — shown fresh.
    fn fresh(study: &Study, field: &str, value: &str) -> DraftRecord {
        let mut d = draft(study.id, field, value);
        let mut payload: DraftPayload = serde_json::from_str(&d.payload).expect("payload");
        payload.base_fingerprint = crate::state::seen_fingerprint(study, &payload);
        d.payload = serde_json::to_string(&payload).expect("payload");
        d
    }

    #[test]
    fn only_the_open_studys_pending_judgment_drafts_are_overlaid() {
        let s = study();
        let mine = fresh(&s, "estimated_high_eps", "4.5");
        let other = draft(Uuid::new_v4(), "estimated_high_eps", "4.5");
        let mut decided = fresh(&s, "estimated_low_eps", "1.5");
        decided.status = DraftStatus::Rejected;
        let mut cell = draft(s.id, "eps", "1.0");
        cell.kind = DraftKind::Cell;
        let got = pending_judgment_overlays(
            &s,
            &[mine.clone(), other, decided, cell],
            NumberFormat::Comma,
        );
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].draft_id, mine.id);
        assert_eq!(got[0].field, DraftField::EstimatedHighEps);
        assert!(!got[0].stale);
    }

    #[test]
    fn a_stale_proposal_is_flagged_and_an_unreadable_one_dropped() {
        let s = study();
        // No base fingerprint: it cannot prove freshness → stale (8.2b rule).
        let stale = draft(s.id, "judged_avg_high_pe", "20");
        let bad = draft(s.id, "judged_avg_low_pe", "douze");
        let got = pending_judgment_overlays(&s, &[stale, bad], NumberFormat::Comma);
        assert_eq!(got.len(), 1);
        assert!(got[0].stale);
    }

    #[test]
    fn two_pending_proposals_on_one_field_drop_that_fields_overlay() {
        let s = study();
        let a = fresh(&s, "estimated_low_eps", "1.5");
        let b = fresh(&s, "estimated_low_eps", "1.6");
        let c = fresh(&s, "judged_avg_high_pe", "20");
        let got = pending_judgment_overlays(&s, &[a, b, c], NumberFormat::Comma);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].field, DraftField::JudgedAvgHighPe);
    }

    #[test]
    fn the_option_chip_names_the_option_by_its_label() {
        let s = study();
        let d = fresh(&s, "forecast_low_option", "dividend_supported");
        let got = pending_judgment_overlays(&s, &[d], NumberFormat::Comma);
        assert_eq!(got[0].field_label, AI_CHIP_OPTION_FIELD);
        assert_eq!(got[0].value_label, "Soutenu par dividende");
    }

    #[test]
    fn chart_fields_are_drawn_and_chipped_under_their_chart_the_rest_under_their_field() {
        let s = study();
        let frame = engine::build_frame(&s).expect("frame");
        let drafts = [
            fresh(&s, "estimated_high_eps", "4.5"),
            fresh(&s, "estimated_low_eps", "1.5"),
            fresh(&s, "judged_avg_high_pe", "20"),
            fresh(&s, "judged_avg_low_pe", "9"),
            fresh(&s, "recent_severe_low", "10"),
            fresh(&s, "forecast_low_option", "dividend_supported"),
        ];
        let overlays = pending_judgment_overlays(&s, &drafts, NumberFormat::Comma);
        let v = ai_judgments(&s, Some(&frame), &overlays, NumberFormat::Comma);
        assert!(v.est_high.is_some() && v.est_low.is_some());
        assert!(v.pe_high.is_some() && v.pe_low.is_some());
        assert_eq!(v.growth_chips.len(), 2);
        assert_eq!(v.pe_chips.len(), 2);
        assert_eq!(v.field_chips.len(), 2);
        // Without a frame (the charts not drawn) every chip goes under its field.
        let v = ai_judgments(&s, None, &overlays, NumberFormat::Comma);
        assert!(v.est_high.is_none() && v.pe_high.is_none());
        assert_eq!(v.field_chips.len(), 6);
    }

    #[test]
    fn an_ai_line_is_drawn_on_the_owners_scale_and_never_moves_it() {
        let s = study();
        let frame = engine::build_frame(&s).expect("frame");
        let before = chart::eps_scale(&frame).expect("scale");
        let owner_before = chart::growth_chart(&frame, NumberFormat::Point);
        // A proposal far off the scale: clamped to the edge like a drag, label exact.
        let overlays = pending_judgment_overlays(
            &s,
            &[fresh(&s, "estimated_high_eps", "999999")],
            NumberFormat::Point,
        );
        let v = ai_judgments(&s, Some(&frame), &overlays, NumberFormat::Point);
        let line = v.est_high.expect("drawn");
        assert_eq!(line.geometry.y, 0.0, "clamped to the top edge");
        assert!(
            line.value.starts_with("999"),
            "the label keeps the exact value"
        );
        let after = chart::eps_scale(&frame).expect("scale");
        assert_eq!((before.lmin, before.lmax), (after.lmin, after.lmax));
        // The owner chart drawn before and after the overlay was computed is the same chart.
        let owner_after = chart::growth_chart(&frame, NumberFormat::Point);
        assert_eq!(
            owner_before.judgment_commands,
            owner_after.judgment_commands
        );
        assert_eq!(owner_before.judgment_y, owner_after.judgment_y);
        assert_eq!(owner_before.axis_min, owner_after.axis_min);
        assert_eq!(owner_before.axis_max, owner_after.axis_max);
    }

    #[test]
    fn a_growth_proposal_draws_its_implied_est_high_only_without_a_direct_one() {
        let mut s = study();
        s.judgment.estimated_high_eps = None;
        let frame = engine::build_frame(&s).expect("frame");
        let overlays = pending_judgment_overlays(
            &s,
            &[fresh(&s, "projected_eps_growth_pct", "9")],
            NumberFormat::Comma,
        );
        let v = ai_judgments(&s, Some(&frame), &overlays, NumberFormat::Comma);
        assert!(v.est_high.is_some(), "implied line drawn");
        assert_eq!(v.growth_chips.len(), 1);
        assert!(v.field_chips.is_empty());

        // With a direct est-high the growth proposal would not move the line: chip under its field.
        s.judgment.estimated_high_eps = Some(Money::from(Decimal::new(40, 1)));
        let frame = engine::build_frame(&s).expect("frame");
        let v = ai_judgments(&s, Some(&frame), &overlays, NumberFormat::Comma);
        assert!(v.est_high.is_none());
        assert_eq!(v.field_chips.len(), 1);
    }

    #[test]
    fn a_direct_proposals_line_label_is_its_chips_value() {
        let s = study();
        let frame = engine::build_frame(&s).expect("frame");
        let overlays = pending_judgment_overlays(
            &s,
            &[
                fresh(&s, "judged_avg_low_pe", "11"),
                fresh(&s, "estimated_low_eps", "1.3"),
            ],
            NumberFormat::Comma,
        );
        let v = ai_judgments(&s, Some(&frame), &overlays, NumberFormat::Comma);
        let chip = |f: DraftField| {
            let o = overlays.iter().find(|o| o.field == f).expect("overlay");
            o.value_label.clone()
        };
        assert_eq!(
            v.pe_low.expect("drawn").value,
            chip(DraftField::JudgedAvgLowPe)
        );
        assert_eq!(
            v.est_low.expect("drawn").value,
            chip(DraftField::EstimatedLowEps)
        );
    }

    #[test]
    fn a_proposal_finer_than_the_display_scale_shows_its_exact_digits() {
        let s = study();
        let got = pending_judgment_overlays(
            &s,
            &[fresh(&s, "judged_avg_high_pe", "20.04")],
            NumberFormat::Comma,
        );
        assert_eq!(
            got[0].value_label, "20,04",
            "never rounded into another value"
        );
    }

    #[test]
    fn no_ai_line_without_a_pending_proposal() {
        let s = study();
        let frame = engine::build_frame(&s).expect("frame");
        let v = ai_judgments(&s, Some(&frame), &[], NumberFormat::Comma);
        assert_eq!(v.est_high, None);
        assert_eq!(v.est_low, None);
        assert_eq!(v.pe_high, None);
        assert_eq!(v.pe_low, None);
        assert!(v.growth_chips.is_empty() && v.pe_chips.is_empty() && v.field_chips.is_empty());
    }

    #[test]
    fn a_validated_judgment_carries_its_caption_date() {
        let mut s = study();
        s.judgment.ai_placed.judged_avg_high_pe = Some(AiOrigin {
            draft_id: Uuid::new_v4(),
            client: "c".into(),
            model: "m".into(),
            validated_at: Timestamp("2026-09-28T12:00:00Z".into()),
        });
        let caps = ai_placed_captions(&s);
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0].0, DraftField::JudgedAvgHighPe);
        assert_eq!(
            caps[0].1,
            date_fr(&Timestamp("2026-09-28T12:00:00Z".into()))
        );
    }
}
