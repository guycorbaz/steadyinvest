//! The single `Study → core` construction path (relocated here in Story 5.6 from `app`).
//!
//! This module owns the `contract` → `core` mapping and the ONE construction of a coherent engine
//! frame: `contract::Study` → [`RawFinancials`] → [`normalize`] → [`CanonicalFinancials`];
//! `contract::Judgment` → [`JudgmentInputs`]; [`InputGates`] from each usable year's `review ×
//! freshness` plus the judgment inputs; then [`StudySnapshot::new`] **once** — so the outputs and the
//! verdict are born in one coherent frame (the Story-2.6/2.7 invariant).
//!
//! It lives in `report` (not `core`, which deliberately does NOT depend on `contract`; not `app`,
//! which is the UI) because both the **live form** (`app`) and the **PDF** (`report`) need exactly
//! this construction — and `app` already depends on `report`. `app::viewmodel::engine` re-exports
//! these so every existing call-site keeps resolving unchanged, and there remains a SINGLE
//! `build_frame`: the PDF's computed figures cannot drift from the on-screen form's.
//!
//! Recorded interpretations (unchanged from the original `app` home):
//! - **`judgment_to_gate_state`**: a present judgment value is `ValidatedFresh` (the user's own typed
//!   number, not provider data awaiting sign-off); `None` → `Missing`.
//!   Exception (2026-10-01): the current price goes through **`price_to_gate_state`** — a fetched
//!   price flagged stale by a failed refresh is `Stale` (FR12 / FR23, method spec §5).
//! - **`to_observations`**: v1 carries no quarterly data → [`QuarterlyObservations::empty`].
//! - **splits**: v1 manual entry records no split events → `splits: vec![]`.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use steadyinvest_contract::{
    Cell, ForecastLowOption as CForecastLowOption, FrozenCriterion, FrozenUpsideDownside,
    FrozenVerdict, FrozenZone, FrozenZoneBounds, Judgment, Money, Study, Timestamp,
};
use steadyinvest_core::normalize::{
    self, CanonicalFinancials, CanonicalYear, Finding, NormalizeError, RawAmount, RawFinancials,
    RawYear, YearUsability,
};
use steadyinvest_core::ssg::{ForecastLowOption, JudgmentInputs, QuarterlyObservations};
use steadyinvest_core::verdict::{GateState, InputGates, StudySnapshot, YearGates};

// ── contract → core mapping (pure, unit-tested, no I/O) ──

/// One `Cell.value` → an optional [`RawAmount`] in the study's native currency. An absent cell value
/// stays `None` — never coerced to `0` (the project's most-repeated rail).
fn raw_amount(cell: &Cell, currency: &str) -> Option<RawAmount> {
    cell.value.map(|m| RawAmount {
        value: m.as_decimal(),
        currency: currency.to_string(),
    })
}

/// `contract::Study` → [`RawFinancials`]: one [`RawYear`] per study year. The four load-bearing
/// cells (`sales/eps/high_price/low_price`) and the three optional cells (`dividend_per_share/
/// pre_tax_profit/book_value_per_share`) map by name; the period/fiscal/`net_profit`/`tax_rate`
/// inputs the manual contract does not carry stay `None` (v1: PTP is taken directly from its cell,
/// never grossed up). `splits: vec![]` — v1 manual entry records no split events.
pub fn to_raw_financials(study: &Study) -> RawFinancials {
    let currency = study.native_currency.as_str();
    let years = study
        .years
        .iter()
        .map(|y| RawYear {
            sales: raw_amount(&y.sales, currency),
            eps: raw_amount(&y.eps, currency),
            high_price: raw_amount(&y.high_price, currency),
            low_price: raw_amount(&y.low_price, currency),
            dividend_per_share: y
                .dividend_per_share
                .as_ref()
                .and_then(|c| raw_amount(c, currency)),
            pre_tax_profit: y
                .pre_tax_profit
                .as_ref()
                .and_then(|c| raw_amount(c, currency)),
            book_value_per_share: y
                .book_value_per_share
                .as_ref()
                .and_then(|c| raw_amount(c, currency)),
            ..RawYear::empty(y.year)
        })
        .collect();
    RawFinancials {
        native_currency: study.native_currency.clone(),
        years,
        splits: Vec::new(),
    }
}

/// `Option<Money>` → `Option<Decimal>` (the judgment-input rail). `None` stays `None`. Public so the
/// `app` formatting layer (which still presents judgment values) can reuse the same coercion.
pub fn money_dec(value: Option<Money>) -> Option<Decimal> {
    value.map(Money::as_decimal)
}

/// `contract::ForecastLowOption` → [`ForecastLowOption`] **by name** (a `match`, never an `as`-cast,
/// so a future variant cannot silently mis-map — recorded glue).
pub fn to_forecast_low_option(option: CForecastLowOption) -> ForecastLowOption {
    match option {
        CForecastLowOption::AvgLowPeTimesEps => ForecastLowOption::AvgLowPeTimesEps,
        CForecastLowOption::AvgLowPriceLast5y => ForecastLowOption::AvgLowPriceLast5y,
        CForecastLowOption::RecentSevereLow => ForecastLowOption::RecentSevereLow,
        CForecastLowOption::DividendSupported => ForecastLowOption::DividendSupported,
    }
}

/// `contract::Judgment` → [`JudgmentInputs`]: each `Option<Money>` → `Option<Decimal>`; the option
/// enum glued by name.
pub fn to_judgment_inputs(judgment: &Judgment) -> JudgmentInputs {
    JudgmentInputs {
        estimated_high_eps: money_dec(judgment.estimated_high_eps),
        estimated_low_eps: money_dec(judgment.estimated_low_eps),
        projected_sales_growth_pct: money_dec(judgment.projected_sales_growth_pct),
        projected_eps_growth_pct: money_dec(judgment.projected_eps_growth_pct),
        judged_avg_high_pe: money_dec(judgment.judged_avg_high_pe),
        judged_avg_low_pe: money_dec(judgment.judged_avg_low_pe),
        forecast_low_option: to_forecast_low_option(judgment.forecast_low_option),
        recent_severe_low: money_dec(judgment.recent_severe_low),
        current_price: money_dec(judgment.current_price),
        present_full_year_dividend: money_dec(judgment.present_full_year_dividend),
    }
}

/// Build the quarterly observations the engine's current-P/E needs (Issue #113). A provider fetch
/// fills `judgment.ttm_eps` (EODHD's TTM `EarningsShare`); the engine sums a 4-quarter array for the
/// TTM denominator, so we feed it as `[ttm, 0, 0, 0]` (Σ = ttm). `None` → all-absent (current P/E /
/// relative value stay honestly `unknown`, e.g. a manual study or a provider with no TTM). The other
/// quarterly fields (the quarter-over-quarter §3 trend) are not fetched yet.
pub fn to_observations(study: &Study) -> QuarterlyObservations {
    use rust_decimal::Decimal;
    QuarterlyObservations {
        ttm_quarterly_eps: study
            .judgment
            .ttm_eps
            .map(|t| [t.as_decimal(), Decimal::ZERO, Decimal::ZERO, Decimal::ZERO]),
        ..QuarterlyObservations::empty()
    }
}

/// One data `Cell` → [`GateState`]: `None` (absent cell) → `Missing`; `(Validated, Current)` →
/// `ValidatedFresh`; `(Validated, Stale)` → `Stale`; anything else (present but `review ≠ ✓`) →
/// `NotValidated`.
pub fn cell_to_gate_state(cell: Option<&Cell>) -> GateState {
    use steadyinvest_contract::{Freshness, Review};
    match cell {
        None => GateState::Missing,
        Some(c) => match (c.review, c.freshness) {
            (Review::Validated, Freshness::Current) => GateState::ValidatedFresh,
            (Review::Validated, Freshness::Stale) => GateState::Stale,
            _ => GateState::NotValidated,
        },
    }
}

/// A judgment input value → [`GateState`] (recorded interpretation): `None` → `Missing`; `Some` →
/// `ValidatedFresh` (a deliberately-typed personal judgment is validated-fresh by the act of entry —
/// it is the user's own number, not provider data awaiting sign-off).
pub fn judgment_to_gate_state(value: Option<Money>) -> GateState {
    match value {
        None => GateState::Missing,
        Some(_) => GateState::ValidatedFresh,
    }
}

/// The current price → [`GateState`] (Guy's on-screen test 2026-10-01, FR12 / FR23): `None` →
/// `Missing`; a price whose provider origin was flagged stale by a failed refresh → `Stale` (the
/// verdict degrades, as for a stale provider cell); otherwise `ValidatedFresh` — a fetched price
/// is the owner's own gesture (he asked for the fetch), a typed one his own number.
pub fn price_to_gate_state(
    value: Option<Money>,
    origin: Option<&steadyinvest_contract::PriceOrigin>,
) -> GateState {
    match (value, origin) {
        (None, _) => GateState::Missing,
        (Some(_), Some(o)) if o.freshness == steadyinvest_contract::Freshness::Stale => {
            GateState::Stale
        }
        (Some(_), _) => GateState::ValidatedFresh,
    }
}

/// Build [`InputGates`]: one [`YearGates`] per **usable** year (filter `canonical.years` on
/// [`YearUsability::Usable`], read the matching study year's four load-bearing cells), plus the five
/// load-bearing judgment gates — exactly the pinned catalogs, in catalog order.
pub fn to_input_gates(study: &Study, canonical: &CanonicalFinancials) -> InputGates {
    let year_gates = canonical
        .years
        .iter()
        .filter(|cy| cy.usability == YearUsability::Usable)
        .filter_map(|cy| {
            let year = study.years.iter().find(|y| y.year == cy.year)?;
            // LOAD_BEARING_YEAR_FIELDS order: sales, eps, high_price, low_price.
            let states = [
                cell_to_gate_state(Some(&year.sales)),
                cell_to_gate_state(Some(&year.eps)),
                cell_to_gate_state(Some(&year.high_price)),
                cell_to_gate_state(Some(&year.low_price)),
            ];
            Some(YearGates::new(cy.year, states))
        })
        .collect();
    let j = &study.judgment;
    // LOAD_BEARING_JUDGMENT_INPUTS order: estimated_high_eps, estimated_low_eps, judged_avg_high_pe,
    // judged_avg_low_pe, current_price.
    let judgment_gates = [
        judgment_to_gate_state(j.estimated_high_eps),
        judgment_to_gate_state(j.estimated_low_eps),
        judgment_to_gate_state(j.judged_avg_high_pe),
        judgment_to_gate_state(j.judged_avg_low_pe),
        price_to_gate_state(j.current_price, j.current_price_origin.as_ref()),
    ];
    InputGates::new(year_gates, judgment_gates)
}

/// One coherent engine frame (Story 2.7): the immutable [`StudySnapshot`] **and** the input-shape
/// plausibility findings that `normalize` raised for the SAME mapped inputs. [`StudySnapshot::new`]
/// consumes `&CanonicalFinancials` without re-exposing its `.findings`, so they are cloned off the
/// canonical BEFORE the move — the verdict and the cell warnings then descend from one normalize, no
/// drift (the coherence invariant Story 2.6 established for outputs/verdict, extended to findings).
pub struct StudyFrame {
    pub snapshot: StudySnapshot,
    /// The input-shape findings (`split_series_break` / `currency_mismatch` /
    /// `fiscal_period_misalignment`) on `CanonicalFinancials`; the calc-time findings live on
    /// `snapshot.outputs().findings`.
    pub plausibility: Vec<Finding>,
    /// The canonical per-year series (sorted ascending), cloned off the canonical BEFORE the
    /// `StudySnapshot::new` move — the §1 growth chart (Story 2.8) plots these (`sales` / `eps` /
    /// `high_price`), and they descend from the SAME single `normalize` as the verdict and the
    /// warnings (no second pass, no frame drift).
    pub series: Vec<CanonicalYear>,
}

/// The single construction path: `Study` → raw → `normalize` → `StudySnapshot::new` once, returning
/// the snapshot together with the input-shape findings (Story 2.7). A [`NormalizeError`] surfaces to
/// the caller (a neutral notice) — never `unwrap`/`.ok()`. **Normalizes exactly once** (no second
/// pass that could drift from the frame that produced the verdict).
pub fn build_frame(study: &Study) -> Result<StudyFrame, NormalizeError> {
    let raw = to_raw_financials(study);
    let canonical = normalize::normalize(raw)?;
    let judgment = to_judgment_inputs(&study.judgment);
    let observations = to_observations(study);
    let gates = to_input_gates(study, &canonical);
    // Clone the input-shape findings AND the per-year series off the canonical before the `new(...)`
    // move consumes it — both descend from this one `normalize`, so the chart, the warnings and the
    // verdict share a single coherent frame (no second normalize, the Story-2.7 invariant).
    let plausibility = canonical.findings.clone();
    let series = canonical.years.clone();
    Ok(StudyFrame {
        snapshot: StudySnapshot::new(&canonical, &judgment, &observations, gates),
        plausibility,
        series,
    })
}

/// The snapshot-only view of [`build_frame`] — the Story-2.6 call shape, preserved for the callers
/// that need only the snapshot. Normalizes once.
pub fn build_snapshot(study: &Study) -> Result<StudySnapshot, NormalizeError> {
    build_frame(study).map(|frame| frame.snapshot)
}

/// Why a verdict cannot be frozen (Story 8.8): it is not full, or the study does not normalize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotFrozen {
    /// The verdict is provisional or withheld — nothing to freeze.
    NotFull,
    Normalize(NormalizeError),
}

/// Freeze the study's verdict (Story 8.8, FR68, A13) — ONLY a [`Verdict::Full`] of ONE coherent
/// frame, with that frame's outputs and the inputs its digest covers. Nothing is recomputed later:
/// the result is stored as is.
///
/// [`Verdict::Full`]: steadyinvest_core::verdict::Verdict::Full
pub fn freeze(study: &Study, now: &Timestamp) -> Result<FrozenVerdict, NotFrozen> {
    let frame = build_frame(study).map_err(NotFrozen::Normalize)?;
    if !matches!(
        frame.snapshot.verdict(),
        steadyinvest_core::verdict::Verdict::Full(_)
    ) {
        return Err(NotFrozen::NotFull);
    }
    Ok(verdict_record(study, &frame, now))
}

/// The verdict of one frame in the frozen shape (Story 8.8) — what [`freeze`] stores for a Full
/// verdict, and the CURRENT side of the frozen-vs-current comparison (any verdict state: facts are
/// carried by every state). Never persisted for a current verdict.
pub fn verdict_record(study: &Study, frame: &StudyFrame, now: &Timestamp) -> FrozenVerdict {
    use steadyinvest_core::ssg::{CriterionFact, UpsideDownside, Zone};
    let verdict = frame.snapshot.verdict();
    let facts = verdict.facts();
    let out = frame.snapshot.outputs();
    let rr = &out.risk_reward;
    let money = |d: Decimal| Money::from(d);
    let criterion = |c: CriterionFact| match c {
        CriterionFact::Met => FrozenCriterion::Met,
        CriterionFact::Unmet => FrozenCriterion::Unmet,
        CriterionFact::UnmetByInsufficiency => FrozenCriterion::UnmetByInsufficiency,
    };
    FrozenVerdict {
        frozen_at: now.clone(),
        method_version: verdict.method_version().to_string(),
        inputs_hash: verdict.inputs_hash().to_string(),
        quality_value_candidate: facts.quality_value_candidate,
        present_zone: facts.present_price_zone.map(|z| match z {
            Zone::Buy => FrozenZone::Low,
            Zone::Neutral => FrozenZone::Middle,
            Zone::Sell => FrozenZone::High,
        }),
        ud_at_or_above_target: criterion(facts.ud_at_or_above_target),
        relative_value_below_ceiling: criterion(facts.relative_value_below_ceiling),
        present_price_in_low_zone: criterion(facts.present_price_in_buy_zone),
        appreciation_at_or_above_double: criterion(facts.appreciation_at_or_above_double),
        upside_downside: match rr.upside_downside {
            UpsideDownside::Ratio(r) => FrozenUpsideDownside::Ratio(money(r)),
            UpsideDownside::Undefined => FrozenUpsideDownside::Undefined,
            UpsideDownside::Unknown => FrozenUpsideDownside::Unknown,
        },
        relative_value_pct: out.valuation.relative_value_pct.map(money),
        projected_appreciation_pct: out.returns.projected_appreciation_pct.map(money),
        total_return_pct: out.returns.projected_total_annualized_return_pct.map(money),
        appreciation_only_pct: out.returns.appreciation_only_potential().map(money),
        zones: rr.zones.as_ref().map(|z| FrozenZoneBounds {
            forecast_low: money(z.forecast_low),
            low_zone_top: money(z.buy_top),
            middle_zone_top: money(z.neutral_top),
            forecast_high: money(z.forecast_high),
        }),
        inputs: frozen_inputs(study, &frame.series),
    }
}

/// The inputs a verdict's digest covers, keyed for the comparison's « Entrées » row (Story 8.8):
/// `y{year}.{field}` for every canonical year, `j.{field}` for every judgment input, `q.{field}`
/// for the quarterly observations — the canonical decimal (normalized) or « absent ». The same
/// values `core::verdict::digest` hashes, so a hash difference always names its inputs.
pub fn frozen_inputs(study: &Study, series: &[CanonicalYear]) -> BTreeMap<String, String> {
    let enc = |v: Option<Decimal>| match v {
        Some(d) => d.normalize().to_string(),
        None => ABSENT.to_string(),
    };
    let mut map = BTreeMap::new();
    for y in series {
        for (field, v) in [
            ("sales", y.sales),
            ("eps", y.eps),
            ("high_price", y.high_price),
            ("low_price", y.low_price),
            ("dividend_per_share", y.dividend_per_share),
            ("pre_tax_profit", y.pre_tax_profit),
            ("book_value_per_share", y.book_value_per_share),
        ] {
            map.insert(format!("y{}.{field}", y.year), enc(v));
        }
    }
    let j = to_judgment_inputs(&study.judgment);
    for (field, v) in [
        ("estimated_high_eps", j.estimated_high_eps),
        ("estimated_low_eps", j.estimated_low_eps),
        ("projected_sales_growth_pct", j.projected_sales_growth_pct),
        ("projected_eps_growth_pct", j.projected_eps_growth_pct),
        ("judged_avg_high_pe", j.judged_avg_high_pe),
        ("judged_avg_low_pe", j.judged_avg_low_pe),
        ("recent_severe_low", j.recent_severe_low),
        ("current_price", j.current_price),
        ("present_full_year_dividend", j.present_full_year_dividend),
    ] {
        map.insert(format!("j.{field}"), enc(v));
    }
    map.insert(
        "j.forecast_low_option".to_string(),
        steadyinvest_contract::option_name(study.judgment.forecast_low_option).to_string(),
    );
    let q = to_observations(study);
    map.insert(
        "q.ttm_quarterly_eps".to_string(),
        match q.ttm_quarterly_eps {
            Some(qs) => qs
                .iter()
                .map(|d| d.normalize().to_string())
                .collect::<Vec<_>>()
                .join("|"),
            None => ABSENT.to_string(),
        },
    );
    for (field, v) in [
        ("latest_quarter_sales", q.latest_quarter_sales),
        ("latest_quarter_eps", q.latest_quarter_eps),
        ("year_ago_quarter_sales", q.year_ago_quarter_sales),
        ("year_ago_quarter_eps", q.year_ago_quarter_eps),
    ] {
        map.insert(format!("q.{field}"), enc(v));
    }
    map
}

/// The value [`frozen_inputs`] writes for an absent input (the digest's sentinel).
pub const ABSENT: &str = "absent";

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::{
        Cell, Coverage, Freshness, Money, Provenance, Review, Source, Timestamp, YearData,
    };
    use steadyinvest_core::verdict::Verdict;
    use uuid::Uuid;

    fn money_of(s: &str) -> Money {
        Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
    }

    fn cell(value: &str) -> Cell {
        Cell {
            value: Some(money_of(value)),
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

    fn full_study() -> Study {
        let judgment = Judgment {
            ai_placed: Default::default(),
            current_price_origin: None,
            estimated_high_eps: Some(money_of("9")),
            estimated_low_eps: Some(money_of("4")),
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: Some(money_of("18")),
            judged_avg_low_pe: Some(money_of("10")),
            forecast_low_option: CForecastLowOption::AvgLowPeTimesEps,
            recent_severe_low: None,
            current_price: Some(money_of("80")),
            present_full_year_dividend: Some(money_of("2")),
            ttm_eps: None,
        };
        let mut s = Study::new(
            Uuid::from_u128(0x5_6),
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

    // Story 8.8 — only a Full verdict freezes; the result carries the engine's own hash and method,
    // and its inputs name exactly what the digest covers.
    #[test]
    fn only_a_full_verdict_freezes_with_the_engines_hash_and_inputs() {
        let study = full_study();
        let now = Timestamp("2026-09-30T10:00:00Z".to_string());
        let frozen = freeze(&study, &now).expect("full");
        let snap = build_snapshot(&study).unwrap();
        assert_eq!(frozen.inputs_hash, snap.inputs_hash());
        assert_eq!(frozen.method_version, steadyinvest_core::METHOD_VERSION);
        assert_eq!(frozen.frozen_at, now);
        assert_eq!(frozen.inputs["y2025.eps"], "5");
        assert_eq!(frozen.inputs["j.current_price"], "80");
        assert_eq!(frozen.inputs["j.recent_severe_low"], ABSENT);
        assert_eq!(
            frozen.inputs["j.forecast_low_option"],
            "avg_low_pe_times_eps"
        );
        assert_eq!(
            frozen.inputs.len(),
            5 * 7 + 10 + 5,
            "every year field, every judgment input, every observation"
        );
        // Deterministic: the same study freezes to the same value.
        assert_eq!(freeze(&study, &now).unwrap(), frozen);
        // A provisional verdict (one input not validated) does not freeze.
        let mut provisional = study.clone();
        provisional.years[4].eps.review = Review::ToReview;
        assert_eq!(freeze(&provisional, &now), Err(NotFrozen::NotFull));
    }

    #[test]
    fn build_frame_constructs_one_coherent_frame() {
        // The single construction path yields a snapshot, its series, and the input-shape findings —
        // all from ONE normalize. A fully-validated-fresh 5-year study derives Verdict::Full.
        let frame = build_frame(&full_study()).expect("a complete study normalizes");
        assert_eq!(frame.series.len(), 5, "five canonical years");
        assert!(
            matches!(frame.snapshot.verdict(), Verdict::Full(_)),
            "all-validated-fresh + full confidence derives Full"
        );
        // `build_snapshot` is the snapshot-only view of the SAME construction.
        let snap = build_snapshot(&full_study()).unwrap();
        assert_eq!(
            snap.outputs(),
            frame.snapshot.outputs(),
            "one construction, no drift"
        );
    }

    #[test]
    fn the_price_gate_reads_its_origin() {
        use steadyinvest_contract::{Freshness, PriceOrigin, Source, Timestamp};
        let price = Some(Money::from(rust_decimal::Decimal::new(60, 0)));
        let o = |source, freshness| PriceOrigin {
            source,
            at: Timestamp("2026-10-01T09:00:00Z".to_string()),
            session_date: None,
            freshness,
        };
        assert_eq!(price_to_gate_state(None, None), GateState::Missing);
        assert_eq!(
            price_to_gate_state(price, None),
            GateState::ValidatedFresh,
            "unknown origin"
        );
        assert_eq!(
            price_to_gate_state(price, Some(&o(Source::Provider, Freshness::Current))),
            GateState::ValidatedFresh
        );
        assert_eq!(
            price_to_gate_state(price, Some(&o(Source::Provider, Freshness::Stale))),
            GateState::Stale
        );
        assert_eq!(
            price_to_gate_state(price, Some(&o(Source::Manual, Freshness::Current))),
            GateState::ValidatedFresh
        );
        assert_eq!(
            price_to_gate_state(None, Some(&o(Source::Provider, Freshness::Stale))),
            GateState::Missing,
            "no price: missing whatever its stale origin says"
        );
    }
}
