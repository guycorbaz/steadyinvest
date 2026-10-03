//! The JSON the MCP tools return (Story 8.4 AC 6–8): the dossier identity, studies, history
//! snapshots, notes, the drafts record — and each study's **computed outputs** (owner decision O1),
//! built with the app's own construction `report::form::build_snapshot` and mapped field by field.
//!
//! Numbers are the **raw decimals as strings** (`normalize()`d), `null` when unknown — never a
//! presentation format, never a value re-derived here (Cardinal Rule). Zones are **neutral codes**
//! (`low` / `middle` / `high` — the app's neutral label set), never buy / hold / sell (FR13: this
//! is app-generated output).

use rust_decimal::Decimal;
use serde_json::{Value, json};
use steadyinvest_contract::{Study, Timestamp};
use steadyinvest_core::ssg::{CriterionFact, UpsideDownside, Zone};
use steadyinvest_core::verdict::{GateState, GatedInput, StudySnapshot, Verdict};
use steadyinvest_persistence::{
    DossierIdentity, DraftRecord, McpSnapshot, McpStudyRead, Paged, StudySummary,
};

/// The dossier a call read (O3): `journal_id` + resolved path.
pub fn dossier(id: &DossierIdentity) -> Value {
    json!({
        "journal_id": id.journal_id.to_string(),
        "path": id.path.display().to_string(),
    })
}

fn dec(d: Option<Decimal>) -> Value {
    match d {
        Some(d) => Value::String(d.normalize().to_string()),
        None => Value::Null,
    }
}

fn zone_code(z: Zone) -> &'static str {
    match z {
        Zone::Buy => "low",
        Zone::Neutral => "middle",
        Zone::Sell => "high",
    }
}

fn criterion(c: CriterionFact) -> &'static str {
    match c {
        CriterionFact::Met => "met",
        CriterionFact::Unmet => "unmet",
        CriterionFact::UnmetByInsufficiency => "unknown",
    }
}

fn gate_state(s: GateState) -> &'static str {
    match s {
        GateState::Missing => "missing",
        GateState::NotValidated => "not_validated",
        GateState::Stale => "stale",
        GateState::ValidatedFresh => "validated_fresh",
    }
}

fn gated_input(i: GatedInput) -> Value {
    match i {
        GatedInput::YearField { year, field } => json!({ "year": year, "field": field }),
        GatedInput::JudgmentInput { name } => json!({ "field": name }),
    }
}

/// The computed outputs of one snapshot (Story 8.4 AC 8).
pub fn computed(snapshot: &StudySnapshot, study: &Study) -> Value {
    let verdict = snapshot.verdict();
    let state = match verdict {
        Verdict::Full(_) => "full",
        Verdict::Provisional(_) => "provisional",
        Verdict::Withheld(_) => "withheld",
    };
    let facts = verdict.facts();
    let out = snapshot.outputs();
    let rr = &out.risk_reward;
    let (ud, ud_state) = match rr.upside_downside {
        UpsideDownside::Ratio(r) => (dec(Some(r)), "ratio"),
        UpsideDownside::Undefined => (Value::Null, "undefined"),
        UpsideDownside::Unknown => (Value::Null, "unknown"),
    };
    let zones = match &rr.zones {
        Some(z) => json!({
            "forecast_low": dec(Some(z.forecast_low)),
            "low_zone_top": dec(Some(z.buy_top)),
            "middle_zone_top": dec(Some(z.neutral_top)),
            "forecast_high": dec(Some(z.forecast_high)),
        }),
        None => Value::Null,
    };
    json!({
        "verdict_state": state,
        "low_confidence": verdict.low_confidence(),
        "open_gates": verdict.open_gates().iter().map(|g| json!({
            "input": gated_input(g.input),
            "state": gate_state(g.state),
        })).collect::<Vec<_>>(),
        "method_version": verdict.method_version(),
        "inputs_hash": verdict.inputs_hash(),
        "verdict_facts": {
            "present_price_zone": facts.present_price_zone.map(zone_code),
            "ud_at_or_above_target": criterion(facts.ud_at_or_above_target),
            "relative_value_below_ceiling": criterion(facts.relative_value_below_ceiling),
            "present_price_in_low_zone": criterion(facts.present_price_in_buy_zone),
            "appreciation_at_or_above_double": criterion(facts.appreciation_at_or_above_double),
            "quality_value_candidate": facts.quality_value_candidate,
        },
        "forecast_high": dec(rr.forecast_high),
        "forecast_low": dec(rr.forecast_low),
        "zones": zones,
        "upside_downside": ud,
        "upside_downside_state": ud_state,
        "relative_value_pct": dec(out.valuation.relative_value_pct),
        "projected_appreciation_pct": dec(out.returns.projected_appreciation_pct),
        "projected_annualized_appreciation_pct":
            dec(out.returns.projected_annualized_appreciation_pct),
        "projected_total_annualized_return_pct":
            dec(out.returns.projected_total_annualized_return_pct),
        "sales_cagr_pct": dec(out.growth.sales_cagr_pct),
        "eps_cagr_pct": dec(out.growth.eps_cagr_pct),
        // As the owner's screens state them (one high-P/E flag — project review 2026-10-01), with
        // whether every rule could be checked: an empty list then means « none », else « unknown ».
        "quality_flags": steadyinvest_core::ssg::shown_quality_flags(&out.quality_flags)
            .iter()
            .map(|f| f.as_str())
            .collect::<Vec<_>>(),
        "quality_flags_assessable": steadyinvest_core::ssg::quality_flags_assessable(
            out,
            &steadyinvest_report::form::to_judgment_inputs(&study.judgment),
        ),
    })
}

/// A study as the contract serializes it (cells with provenance, judgments, rationale, notes).
fn study_json(study: &Study) -> Value {
    serde_json::to_value(study).unwrap_or_else(|e| json!({ "unserializable": e.to_string() }))
}

/// `get_study`: the study, its status and its computed outputs. A study the engine cannot
/// normalize still returns its data, with `computed: null` and the reason. The current price is
/// aged on `now`'s day with the owner's horizon (FR23) before the verdict is computed — the same
/// read-time rule as the app (`report::price_age`; the mark is never serialized). Since the study
/// JSON's `current_price_origin.freshness` only carries a failed refresh's flag, `computed` states
/// the age explicitly (review of PR #293): `price_aged` (the current price is past the horizon on
/// this read — why an `open_gates` entry can say `stale` while the freshness says `current`) and
/// `price_stale_after_trading_days` (the horizon used).
pub fn study_read(read: &McpStudyRead, now: &Timestamp, price_stale_after: u32) -> Value {
    use steadyinvest_report::price_age;
    let mut study = read.study.clone();
    price_age::apply_price_age(&mut study, price_age::today(now), price_stale_after);
    let price_aged = study.judgment.current_price.is_some()
        && match study.judgment.current_price_origin.as_ref() {
            Some(origin) => origin.aged,
            None => price_age::unknown_origin_is_aged(),
        };
    let mut v = json!({
        "status": read.status,
        "study": study_json(&study),
    });
    match steadyinvest_report::form::build_snapshot(&study) {
        Ok(snapshot) => {
            let mut c = computed(&snapshot, &study);
            c["price_aged"] = Value::Bool(price_aged);
            c["price_stale_after_trading_days"] = json!(price_stale_after);
            v["computed"] = c;
        }
        Err(e) => {
            v["computed"] = Value::Null;
            v["computed_unavailable"] = Value::String(e.to_string());
        }
    }
    v
}

/// `get_notes`: the study's notes.
pub fn notes(read: &McpStudyRead) -> Value {
    json!({
        "study_id": read.study.id.to_string(),
        "security_ticker": read.study.security_ticker,
        "notes": serde_json::to_value(&read.study.notes).unwrap_or(Value::Null),
    })
}

/// One page of study summaries.
pub fn study_list(page: &Paged<StudySummary>) -> Value {
    json!({
        "offset": page.offset,
        "total": page.total,
        "studies": page.items.iter().map(|s| json!({
            "id": s.id.to_string(),
            "security_ticker": s.security_ticker,
            "created_at": s.created_at.0,
            "status": s.status,
        })).collect::<Vec<_>>(),
    })
}

/// One page of FR51 history snapshots, newest first.
pub fn history(study_id: uuid::Uuid, page: &Paged<McpSnapshot>) -> Value {
    json!({
        "study_id": study_id.to_string(),
        "offset": page.offset,
        "total": page.total,
        "snapshots": page.items.iter().map(|s| json!({
            "id": s.id.to_string(),
            "created_at": s.created_at.0,
            "study": study_json(&s.study),
        })).collect::<Vec<_>>(),
    })
}

/// One draft of the record (FR77), with its outcome.
pub fn draft(d: &DraftRecord) -> Value {
    let payload =
        serde_json::from_str::<Value>(&d.payload).unwrap_or(Value::String(d.payload.clone()));
    json!({
        "id": d.id.to_string(),
        "kind": d.kind.as_str(),
        "study_id": d.study_id.map(|s| s.to_string()),
        "security_ticker": d.security_ticker,
        "native_currency": d.native_currency,
        "status": d.status.as_str(),
        "created_at": d.created_at.0,
        "decided_at": d.decided_at.as_ref().map(|t| t.0.clone()),
        "comment": d.comment,
        "origin_client": d.origin_client,
        "origin_model": d.origin_model,
        "stale_at_decision": d.stale_at_decision,
        "edited_before_validation": d.edited_before_validation,
        "created_study_id": d.created_study_id.map(|s| s.to_string()),
        "payload": payload,
    })
}

/// One page of the drafts record.
pub fn draft_list(page: &Paged<DraftRecord>) -> Value {
    json!({
        "offset": page.offset,
        "total": page.total,
        "drafts": page.items.iter().map(draft).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::{
        Cell, Coverage, ForecastLowOption, Freshness, Judgment, Money, Provenance, Review, Source,
        Timestamp, YearData,
    };
    use steadyinvest_core::verdict::Verdict;
    use uuid::Uuid;

    fn money(s: &str) -> Money {
        Money::from(Decimal::from_str_exact(s).unwrap())
    }

    fn cell(value: &str) -> Cell {
        Cell {
            value: Some(money(value)),
            source: Source::Manual,
            freshness: Freshness::Current,
            review: Review::Validated,
            coverage: Coverage::Present,
            provenance: Provenance {
                ai_origin: None,
                source: Source::Manual,
                logical_version: 1,
                timestamp: Timestamp("2026-03-09T00:00:00Z".to_string()),
                hash_of_dependencies: "manual".to_string(),
            },
            pending: None,
        }
    }

    /// The report crate's fully-validated 5-year study (a Full verdict).
    fn full_study() -> Study {
        let judgment = Judgment {
            ai_placed: Default::default(),
            current_price_origin: None,
            estimated_high_eps: Some(money("9")),
            estimated_low_eps: Some(money("4")),
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: Some(money("18")),
            judged_avg_low_pe: Some(money("10")),
            forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
            recent_severe_low: None,
            current_price: Some(money("80")),
            present_full_year_dividend: Some(money("2")),
            ttm_eps: None,
        };
        let mut s = Study::new(
            Uuid::from_u128(0x56),
            Uuid::from_u128(0x1),
            "NESN",
            "CHF",
            judgment,
            Timestamp("2026-03-09T09:30:00Z".to_string()),
        );
        s.years = (2021..=2025)
            .map(|y| YearData {
                year: y,
                sales: cell("1000"),
                eps: cell("5"),
                high_price: cell("100"),
                low_price: cell("50"),
                dividend_per_share: Some(cell("2")),
                pre_tax_profit: Some(cell("200")),
                book_value_per_share: Some(cell("40")),
            })
            .collect();
        s
    }

    #[test]
    fn a_full_verdict_maps_zone_and_criteria_to_neutral_codes() {
        let study = full_study();
        let snapshot = steadyinvest_report::form::build_snapshot(&study).unwrap();
        assert!(matches!(snapshot.verdict(), Verdict::Full(_)));
        let v = computed(&snapshot, &study);
        assert_eq!(v["verdict_state"], json!("full"));
        assert_eq!(v["open_gates"], json!([]));
        let facts = snapshot.verdict().facts();
        let expected_zone = facts.present_price_zone.map(zone_code);
        assert_eq!(
            v["verdict_facts"]["present_price_zone"],
            json!(expected_zone)
        );
        assert_eq!(
            expected_zone,
            Some("low"),
            "80 sits in the lower third of 40..162"
        );
        for (key, fact) in [
            ("ud_at_or_above_target", facts.ud_at_or_above_target),
            (
                "relative_value_below_ceiling",
                facts.relative_value_below_ceiling,
            ),
            ("present_price_in_low_zone", facts.present_price_in_buy_zone),
            (
                "appreciation_at_or_above_double",
                facts.appreciation_at_or_above_double,
            ),
        ] {
            assert_eq!(v["verdict_facts"][key], json!(criterion(fact)), "{key}");
        }
        assert_eq!(v["forecast_high"], json!("162"));
        assert_eq!(v["forecast_low"], json!("40"));
        assert!(v["zones"]["low_zone_top"].is_string());
        for word in ["buy", "sell", "hold"] {
            assert!(!v.to_string().to_lowercase().contains(word), "{word}");
        }
        // Every zone and criterion value maps to a neutral code.
        assert_eq!(zone_code(Zone::Buy), "low");
        assert_eq!(zone_code(Zone::Neutral), "middle");
        assert_eq!(zone_code(Zone::Sell), "high");
        assert_eq!(criterion(CriterionFact::Met), "met");
        assert_eq!(criterion(CriterionFact::Unmet), "unmet");
        assert_eq!(criterion(CriterionFact::UnmetByInsufficiency), "unknown");
    }

    #[test]
    fn a_study_the_engine_cannot_normalize_still_serves_its_data() {
        let mut study = full_study();
        let first = study.years[0].clone();
        study.years.push(first); // a duplicated year: the engine refuses to normalize
        let read = McpStudyRead {
            study,
            status: "active".to_string(),
        };
        let v = study_read(&read, &Timestamp("2026-10-01T10:00:00Z".to_string()), 1);
        assert_eq!(v["computed"], Value::Null);
        assert!(v["computed_unavailable"].is_string(), "{v}");
        assert_eq!(v["study"]["security_ticker"], json!("NESN"));
    }

    // FR23 age horizon (owner decision 2026-10-01): the AI's live verdict ages the price like the
    // app's — provisional past the horizon, full within it; the mark never reaches the study JSON.
    #[test]
    fn the_live_verdict_ages_the_price_with_the_owners_horizon() {
        let mut study = full_study();
        study.judgment.current_price_origin = Some(steadyinvest_contract::PriceOrigin {
            source: Source::Provider,
            at: Timestamp("2026-10-02T20:00:00Z".to_string()),
            session_date: Some("2026-10-02".to_string()), // a Friday
            freshness: Freshness::Current,
            aged: false,
        });
        let read = McpStudyRead {
            study,
            status: "active".to_string(),
        };
        let monday = Timestamp("2026-10-05T09:00:00Z".to_string());
        let tuesday = Timestamp("2026-10-06T09:00:00Z".to_string());
        assert_eq!(
            study_read(&read, &monday, 1)["computed"]["verdict_state"],
            json!("full")
        );
        let aged = study_read(&read, &tuesday, 1);
        assert_eq!(
            aged["computed"]["verdict_state"],
            json!("provisional"),
            "{aged}"
        );
        assert!(!aged["study"].to_string().contains("aged"));
        let wider = study_read(&read, &tuesday, 2);
        assert_eq!(wider["computed"]["verdict_state"], json!("full"));
        // Review of PR #293: the study JSON's freshness still says `current` (no failed refresh),
        // so `computed` states the age and the horizon explicitly — no self-contradiction.
        assert_eq!(
            aged["study"]["judgment"]["current_price_origin"]["freshness"],
            json!("current")
        );
        assert_eq!(aged["computed"]["price_aged"], json!(true));
        assert_eq!(aged["computed"]["price_stale_after_trading_days"], json!(1));
        assert!(
            aged["computed"]["open_gates"]
                .as_array()
                .unwrap()
                .iter()
                .any(|g| g["state"] == json!("stale")),
            "{aged}"
        );
        assert_eq!(
            study_read(&read, &monday, 1)["computed"]["price_aged"],
            json!(false)
        );
        assert_eq!(wider["computed"]["price_aged"], json!(false));
        assert_eq!(
            wider["computed"]["price_stale_after_trading_days"],
            json!(2)
        );
    }

    // Project review 2026-10-01: the AI reads the flags the owner sees — one high-P/E flag — and
    // whether they could be assessed at all.
    #[test]
    fn the_flags_are_the_shown_ones_with_their_assessability() {
        let mut study = full_study();
        study.judgment.judged_avg_high_pe = Some(steadyinvest_contract::Money::from(
            rust_decimal::Decimal::new(45, 0),
        ));
        let snapshot = steadyinvest_report::form::build_snapshot(&study).unwrap();
        let v = computed(&snapshot, &study);
        let flags: Vec<&str> = v["quality_flags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_str().unwrap())
            .collect();
        assert!(
            flags.contains(&"projected_high_pe_implausible"),
            "{flags:?}"
        );
        assert!(
            !flags.contains(&"projected_high_pe_aggressive"),
            "{flags:?}"
        );
        assert!(v["quality_flags_assessable"].is_boolean());
    }
}
