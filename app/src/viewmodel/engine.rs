//! The engine-wiring adapter (Story 2.6) — the FIRST place the `app` crate calls `core`'s engine.
//!
//! This module owns the `contract` → `core` mapping and the inverse `core` → Slint formatting:
//!
//! - **map** `contract::Study` → [`core::RawFinancials`] → [`core::normalize`] →
//!   [`core::CanonicalFinancials`]; `contract::Judgment` → [`core::ssg::JudgmentInputs`];
//!   build [`core::verdict::InputGates`] from each usable year's cell `review × freshness` plus the
//!   judgment inputs; then [`core::verdict::StudySnapshot::new`] **once** — the single construction
//!   path, so the outputs and the verdict are born in one coherent frame;
//! - **format** the engine's `Option<Decimal>` outputs into already-grouped, locale-aware **strings**
//!   (`None` → the faithful em-dash, NEVER `0`), the §4 [`ZoneBar`] geometry, the verdict badge state
//!   and the traceability surface — the only numbers that cross into `.slint` are pre-formatted
//!   strings + layout floats. **No calculation here** (Cardinal Rule): every value comes from the
//!   snapshot; this module maps types and presents them.
//!
//! The interpretations this module records (filed as a GitHub issue, the 1.11/2.1–2.5 pattern):
//! - **`judgment_to_gate_state`**: a present judgment value is `ValidatedFresh` — a deliberately
//!   typed personal judgment is the user's own validated number, not provider data awaiting sign-off
//!   (`None` → `Missing`). The verdict's all-validated-and-fresh gate is therefore, in practice,
//!   gated by the §2/§3 *data* cells (2.5's review markers) once the judgment values are entered.
//! - **`to_observations`**: v1 carries no quarterly data → [`QuarterlyObservations::empty`] → current
//!   P/E / relative value are honestly `unknown` (quarterly capture is a later story / Epic 3).
//! - **splits**: v1 manual entry records no split events → `splits: vec![]`.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use steadyinvest_contract::{
    Cell, ForecastLowOption as CForecastLowOption, Judgment, Money, Study,
};
use steadyinvest_core::normalize::{
    self, CanonicalFinancials, CanonicalYear, Finding, NormalizeError, PlausibilityKey, RawAmount,
    RawFinancials, RawYear, YearUsability,
};
use steadyinvest_core::rounding::DisplayField;
use steadyinvest_core::ssg::{
    CalcFinding, ForecastLowOption, JudgmentInputs, QuarterlyObservations, SsgOutputs, Trend,
    UpsideDownside, Zone,
};
use steadyinvest_core::verdict::{
    GateState, GatedInput, InputGates, OpenGate, StudySnapshot, Verdict, YearGates,
};

use crate::viewmodel::entry;
use crate::viewmodel::form::EMPTY_SLOT;
use crate::viewmodel::format::{format_scaled, NumberFormat};
use crate::{
    GrowthComputed, JudgmentFields, MgmtComputed, PeComputed, ReturnComputed, RiskComputed,
    TraceState, VerdictState, ZoneBarState,
};

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

/// `Option<Money>` → `Option<Decimal>` (the judgment-input rail). `None` stays `None`.
fn money_dec(value: Option<Money>) -> Option<Decimal> {
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

/// v1 manual study carries no quarterly data → [`QuarterlyObservations::empty`] (current P/E /
/// relative value are honestly `unknown`; quarterly capture is a later story / Epic 3).
pub fn to_observations(_study: &Study) -> QuarterlyObservations {
    QuarterlyObservations::empty()
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

/// A judgment input value → [`GateState`] (recorded interpretation, AC 6): `None` → `Missing`;
/// `Some` → `ValidatedFresh` (a deliberately-typed personal judgment is validated-fresh by the act
/// of entry — it is the user's own number, not provider data awaiting sign-off).
pub fn judgment_to_gate_state(value: Option<Money>) -> GateState {
    match value {
        None => GateState::Missing,
        Some(_) => GateState::ValidatedFresh,
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
        judgment_to_gate_state(j.current_price),
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
/// the caller (a neutral notice in `state.rs`) — never `unwrap`/`.ok()`. **Normalizes exactly once**
/// (no second pass that could drift from the frame that produced the verdict).
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
/// that need only the snapshot (`state::snapshot_for`, the adapter tests). Normalizes once.
pub fn build_snapshot(study: &Study) -> Result<StudySnapshot, NormalizeError> {
    build_frame(study).map(|frame| frame.snapshot)
}

// ── Plausibility surfacing (Story 2.7) — map the engine's two finding sets to UI cell addresses ──

/// Where a plausibility finding attaches on the faithful form. A `Cell` finding draws the inline §2/§3
/// warning glyph; a `Year` finding marks the whole year suspect (fiscal metadata has no value cell);
/// a `Study` finding (the study-level `low_price_above_current`, `year == None`) anchors near §4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningAnchor {
    Cell {
        year_index: usize,
        field: &'static str,
    },
    Year {
        year_index: usize,
    },
    Study,
}

/// One resolved plausibility warning: its key (for the glyph/microcopy) and where it anchors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlausibilityWarning {
    pub key: PlausibilityKey,
    pub anchor: WarningAnchor,
}

/// The per-study set of resolved warnings, the thin-UI view the form adapter reads (Story 2.7, AC2).
/// Detection stays in `core` (Cardinal Rule); this only maps already-computed findings to addresses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlausibilityWarnings {
    pub items: Vec<PlausibilityWarning>,
}

impl PlausibilityWarnings {
    /// The first warning key anchored at the `(year_index, field)` §2/§3 cell, if any (the form
    /// adapter's per-cell lookup). Deterministic: findings are stored in the engine's pass order.
    pub fn cell_key(&self, year_index: usize, field: &str) -> Option<PlausibilityKey> {
        self.items.iter().find_map(|w| match w.anchor {
            WarningAnchor::Cell {
                year_index: y,
                field: f,
            } if y == year_index && f == field => Some(w.key),
            _ => None,
        })
    }

    /// The first study-level (§4 / judgment-area) warning key, if any.
    pub fn study_key(&self) -> Option<PlausibilityKey> {
        self.items
            .iter()
            .find_map(|w| matches!(w.anchor, WarningAnchor::Study).then_some(w.key))
    }
}

/// A finding `context` field name → its §2/§3 editable cell field, when one exists. Covers the raw
/// input contexts AND the derived-ratio fallback (a `*_pe`/`*_pct`/`ttm_eps` finding anchors at the
/// input cell that contributes it) — the Story-2.7 interpretation table. `None` for fiscal metadata
/// (`period_months` / `fiscal_year_end_month`) and the study-level `current_pe` / `forecast_low`.
fn context_to_field(context: &str) -> Option<&'static str> {
    match context {
        // §3 P/E inputs (raw + the derived ratios that descend from them).
        "high_price" | "high_pe" => Some(entry::FIELD_HIGH), // "a"
        "low_price" | "low_pe" => Some(entry::FIELD_LOW),    // "b"
        "eps" | "ttm_eps" => Some(entry::FIELD_EPS),         // "c"
        "dividend" | "dividend_per_share" => Some(entry::FIELD_DIVIDEND), // "f"
        // §2 management inputs (raw + the derived ratios).
        "sales" => Some(entry::FIELD_SALES),
        "net_profit" | "pre_tax_profit" | "pretax" | "ptp_pct" => Some(entry::FIELD_PRETAX),
        "book_value_per_share" | "roe_pct" => Some(entry::FIELD_BOOK),
        _ => None,
    }
}

/// Resolve one finding `(year, context)` to a [`WarningAnchor`] against the materialized-year window.
/// A `None` year (study-level: `forecast_low`, `current_pe`) → [`WarningAnchor::Study`]; a year outside
/// the window → `Study` too (anchored, never dropped, never mis-attached to the wrong value cell); a
/// known input/derived context → the [`WarningAnchor::Cell`]; otherwise the whole [`WarningAnchor::Year`].
fn resolve_anchor(year: Option<i32>, context: &str, year_numbers: &[i32]) -> WarningAnchor {
    let Some(y) = year else {
        return WarningAnchor::Study;
    };
    let Some(year_index) = year_numbers.iter().position(|n| *n == y) else {
        return WarningAnchor::Study;
    };
    match context_to_field(context) {
        Some(field) => WarningAnchor::Cell { year_index, field },
        None => WarningAnchor::Year { year_index },
    }
}

/// Map the two engine finding sets (input-shape `Finding`s off the frame + calc-time `CalcFinding`s
/// off the outputs) to per-cell / per-year / study-level warnings against the materialized-year
/// window (Story 2.7, AC2/AC3/AC7). Pure mapping — no detection, no thresholds.
pub fn plausibility(
    input_findings: &[Finding],
    calc_findings: &[CalcFinding],
    year_numbers: &[i32],
) -> PlausibilityWarnings {
    let mut items = Vec::with_capacity(input_findings.len() + calc_findings.len());
    for f in input_findings {
        items.push(PlausibilityWarning {
            key: f.key,
            anchor: resolve_anchor(Some(f.year), f.context, year_numbers),
        });
    }
    for f in calc_findings {
        items.push(PlausibilityWarning {
            key: f.key,
            anchor: resolve_anchor(f.year, f.context, year_numbers),
        });
    }
    PlausibilityWarnings { items }
}

// ── core → Slint formatting (already-grouped strings + layout floats; no `Decimal` into `.slint`) ──

/// `Option<Decimal>` → a grouped display string under `field`'s scale, or the faithful em-dash for
/// `None` (NEVER `0`).
fn fmt(value: Option<Decimal>, field: DisplayField, format: NumberFormat) -> String {
    match value {
        Some(v) => format_scaled(v, field, format),
        None => EMPTY_SLOT.to_string(),
    }
}

/// A percentage `Option<Decimal>` → "12,5 %", or the em-dash for `None`.
fn fmt_pct(value: Option<Decimal>, format: NumberFormat) -> String {
    match value {
        Some(v) => format!("{} %", format_scaled(v, DisplayField::Percent, format)),
        None => EMPTY_SLOT.to_string(),
    }
}

/// A [`Trend`] → a fact-stating glyph + noun (never colour): "↑ hausse" / "→ stable" / "↓ baisse".
/// `None` → em-dash. The arrows are language-neutral glyphs; the nouns are scanned (see
/// [`USER_FACING_LABELS`]).
fn fmt_trend(trend: Option<Trend>) -> String {
    match trend {
        Some(Trend::Up) => format!("↑ {TREND_UP}"),
        Some(Trend::Even) => format!("→ {TREND_EVEN}"),
        Some(Trend::Down) => format!("↓ {TREND_DOWN}"),
        None => EMPTY_SLOT.to_string(),
    }
}

/// The U/D ratio as a fact-stating string: `Ratio(d)` → "3,4:1"; `Undefined`/`Unknown` → a stating
/// em-dash (never a fabricated ratio).
fn fmt_ud(ud: &UpsideDownside, format: NumberFormat) -> String {
    match ud {
        UpsideDownside::Ratio(d) => {
            format!("{}:1", format_scaled(*d, DisplayField::Ratio, format))
        }
        UpsideDownside::Undefined | UpsideDownside::Unknown => EMPTY_SLOT.to_string(),
    }
}

/// The present-price zone key crossed to `.slint` (`Labels` resolves the localized noun): "buy" |
/// "neutral" | "sell" | "" (outside range / unknown).
fn zone_key(zone: Option<Zone>) -> &'static str {
    match zone {
        Some(Zone::Buy) => "buy",
        Some(Zone::Neutral) => "neutral",
        Some(Zone::Sell) => "sell",
        None => "",
    }
}

/// The verdict integrity state crossed to `.slint` as an enum-derived string (never the domain enum).
fn verdict_state(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::Full(_) => "full",
        Verdict::Provisional(_) => "provisional",
        Verdict::Withheld(_) => "withheld",
    }
}

/// The §1 growth computed results (historical sales/EPS CAGR; the projected growths are inputs).
pub fn growth_computed(outputs: &SsgOutputs, format: NumberFormat) -> GrowthComputed {
    GrowthComputed {
        sales_cagr: fmt_pct(outputs.growth.sales_cagr_pct, format).into(),
        eps_cagr: fmt_pct(outputs.growth.eps_cagr_pct, format).into(),
    }
}

/// The §2 management computed results (per-year PTP%/ROE%, the 5-yr averages + trends). The per-year
/// cells are aligned to the form's materialized `years` (looked up by year, em-dash when the engine
/// has no row for that year) so the grid columns never misalign with the input rows.
pub fn mgmt_computed(outputs: &SsgOutputs, years: &[i32], format: NumberFormat) -> MgmtComputed {
    let m = &outputs.management;
    let lookup = |year: i32| m.per_year.iter().find(|r| r.year == year);
    let ptp: Vec<slint::SharedString> = years
        .iter()
        .map(|y| fmt_pct(lookup(*y).and_then(|r| r.ptp_pct), format).into())
        .collect();
    let roe: Vec<slint::SharedString> = years
        .iter()
        .map(|y| fmt_pct(lookup(*y).and_then(|r| r.roe_pct), format).into())
        .collect();
    MgmtComputed {
        ptp: slint::ModelRc::new(slint::VecModel::from(ptp)),
        roe: slint::ModelRc::new(slint::VecModel::from(roe)),
        avg_ptp: fmt_pct(m.avg_ptp_pct, format).into(),
        avg_roe: fmt_pct(m.avg_roe_pct, format).into(),
        ptp_trend: fmt_trend(m.ptp_trend).into(),
        roe_trend: fmt_trend(m.roe_trend).into(),
    }
}

/// The §3 P/E summary results (avg high/low/mean P/E, current P/E, relative value).
pub fn pe_computed(outputs: &SsgOutputs, format: NumberFormat) -> PeComputed {
    let v = &outputs.valuation;
    PeComputed {
        avg_high_pe: fmt(v.avg_high_pe, DisplayField::PeRatio, format).into(),
        avg_low_pe: fmt(v.avg_low_pe, DisplayField::PeRatio, format).into(),
        avg_pe: fmt(v.avg_pe, DisplayField::PeRatio, format).into(),
        current_pe: fmt(v.current_pe, DisplayField::PeRatio, format).into(),
        relative_value: fmt_pct(v.relative_value_pct, format).into(),
    }
}

/// The §4 risk/reward computed results (forecast high/low, U/D, appreciation).
pub fn risk_computed(outputs: &SsgOutputs, format: NumberFormat) -> RiskComputed {
    let r = &outputs.risk_reward;
    RiskComputed {
        forecast_high: fmt(r.forecast_high, DisplayField::Price, format).into(),
        forecast_low: fmt(r.forecast_low, DisplayField::Price, format).into(),
        ud_ratio: fmt_ud(&r.upside_downside, format).into(),
        appreciation: fmt_pct(outputs.returns.projected_appreciation_pct, format).into(),
    }
}

/// The §5 five-year-potential computed results (present/avg yield, total annualized return).
pub fn return_computed(outputs: &SsgOutputs, format: NumberFormat) -> ReturnComputed {
    let r = &outputs.returns;
    ReturnComputed {
        present_yield: fmt_pct(r.present_yield_pct, format).into(),
        avg_yield: fmt_pct(r.avg_yield_pct, format).into(),
        total_return: fmt_pct(r.projected_total_annualized_return_pct, format).into(),
    }
}

/// The per-year §3 D/E/G/H strings for one year, looked up in the engine's last-5-usable window;
/// a year not in the window (or an unknown ratio) renders the em-dash.
pub fn pe_year_cells(outputs: &SsgOutputs, year: i32, format: NumberFormat) -> [String; 4] {
    match outputs.valuation.per_year.iter().find(|v| v.year == year) {
        Some(v) => [
            fmt(v.high_pe, DisplayField::PeRatio, format), // D = A÷C
            fmt(v.low_pe, DisplayField::PeRatio, format),  // E = B÷C
            fmt_pct(v.payout_pct, format),                 // G = F÷C×100
            fmt_pct(v.high_yield_pct, format),             // H = F÷B×100
        ],
        None => [
            EMPTY_SLOT.to_string(),
            EMPTY_SLOT.to_string(),
            EMPTY_SLOT.to_string(),
            EMPTY_SLOT.to_string(),
        ],
    }
}

/// The §4 zone-bar geometry + saturation gate. `available = false` → the calm empty state (no fake
/// band). The marker position is a layout float (0 = bottom = forecast low, 1 = top = forecast high);
/// the displayed prices are formatted strings. The `confidence` is the verdict state — full saturated
/// colour ONLY when the verdict is `Full` (AC 6); the bar renders Provisional hatched and Withheld
/// uncoloured.
pub fn zone_bar(study: &Study, snapshot: &StudySnapshot, format: NumberFormat) -> ZoneBarState {
    let outputs = snapshot.outputs();
    let r = &outputs.risk_reward;
    let confidence = verdict_state(snapshot.verdict());
    // The current price is a judgment input (not carried by the immutable snapshot); read it from the
    // same study the snapshot was built from — one frame, one source.
    let current_price = money_dec(study.judgment.current_price);
    let Some(zones) = &r.zones else {
        return ZoneBarState {
            available: false,
            confidence: confidence.into(),
            forecast_low: EMPTY_SLOT.into(),
            buy_top: EMPTY_SLOT.into(),
            neutral_top: EMPTY_SLOT.into(),
            forecast_high: EMPTY_SLOT.into(),
            present_price: fmt(current_price, DisplayField::Price, format).into(),
            present_zone: "".into(),
            marker_pos: -1.0,
        };
    };
    // Layout-only normalized marker position (geometry, not a decision number): where the current
    // price sits within [forecast_low, forecast_high]. Out of range / unknown → no marker (-1).
    let span = zones.forecast_high - zones.forecast_low;
    let marker_pos = match (r.present_price_zone, current_price) {
        (Some(_), Some(price)) if span > Decimal::ZERO => {
            let frac = (price - zones.forecast_low) / span;
            frac.to_f32().unwrap_or(-1.0).clamp(0.0, 1.0)
        }
        _ => -1.0,
    };
    ZoneBarState {
        available: true,
        confidence: confidence.into(),
        forecast_low: format_scaled(zones.forecast_low, DisplayField::Price, format).into(),
        buy_top: format_scaled(zones.buy_top, DisplayField::Price, format).into(),
        neutral_top: format_scaled(zones.neutral_top, DisplayField::Price, format).into(),
        forecast_high: format_scaled(zones.forecast_high, DisplayField::Price, format).into(),
        present_price: fmt(current_price, DisplayField::Price, format).into(),
        present_zone: zone_key(r.present_price_zone).into(),
        marker_pos,
    }
}

/// The verdict badge + sticky-bar facts, all from the SAME snapshot (one coherence frame).
pub fn verdict_badge(
    study: &Study,
    snapshot: &StudySnapshot,
    format: NumberFormat,
) -> VerdictState {
    let outputs = snapshot.outputs();
    let verdict = snapshot.verdict();
    let r = &outputs.risk_reward;
    // The temporal-provenance caption ("computed from data of DD/MM") is a PROVISIONAL-only fact
    // (AC 6): a `Full` verdict needs no qualifier, and a `Withheld` verdict computed nothing to
    // date-stamp — its honest surface is the named open gates, not a provenance date. Only
    // `Provisional` carries the DD/MM.
    let provenance_date = if matches!(verdict, Verdict::Provisional(_)) {
        provenance_dd_mm(study)
    } else {
        String::new()
    };
    let open_gates: Vec<slint::SharedString> = verdict
        .open_gates()
        .iter()
        .map(|g| open_gate_label(g).into())
        .collect();
    // The FR8 low-confidence reason, carried as explicit text ON the verdict surface (AC1) — it shows
    // whenever fewer than five usable years exist, independent of the Provisional/Withheld split, and
    // is empty (silent) otherwise (AC6). `Verdict::low_confidence()` is `false` for a `Full` verdict
    // by construction, so a Full verdict never carries the label.
    let low_confidence = verdict.low_confidence();
    let confidence_label = if low_confidence { CONFIDENCE_LOW } else { "" };
    VerdictState {
        state: verdict_state(verdict).into(),
        low_confidence,
        confidence_label: confidence_label.into(),
        present_price: fmt(
            money_dec(study.judgment.current_price),
            DisplayField::Price,
            format,
        )
        .into(),
        projected_return: fmt_pct(
            outputs.returns.projected_total_annualized_return_pct,
            format,
        )
        .into(),
        appreciation: fmt_pct(outputs.returns.projected_appreciation_pct, format).into(),
        ud_ratio: fmt_ud(&r.upside_downside, format).into(),
        present_zone: zone_key(r.present_price_zone).into(),
        provenance_date: provenance_date.into(),
        open_gates: slint::ModelRc::new(slint::VecModel::from(open_gates)),
    }
}

/// The traceability surface for the verdict (AC 7): the judgment inputs it descends from with their
/// provenance, the method identity + a formula caption, and (for a degraded verdict) the open gates.
pub fn verdict_trace(study: &Study, snapshot: &StudySnapshot, format: NumberFormat) -> TraceState {
    let verdict = snapshot.verdict();
    let j = &study.judgment;
    let inputs: Vec<slint::SharedString> = vec![
        trace_input(
            LBL_EST_HIGH_EPS,
            j.estimated_high_eps,
            DisplayField::PerShare,
            format,
        )
        .into(),
        trace_input(
            LBL_EST_LOW_EPS,
            j.estimated_low_eps,
            DisplayField::PerShare,
            format,
        )
        .into(),
        trace_input(
            LBL_HIGH_PE,
            j.judged_avg_high_pe,
            DisplayField::PeRatio,
            format,
        )
        .into(),
        trace_input(
            LBL_LOW_PE,
            j.judged_avg_low_pe,
            DisplayField::PeRatio,
            format,
        )
        .into(),
        trace_input(
            LBL_CURRENT_PRICE,
            j.current_price,
            DisplayField::Price,
            format,
        )
        .into(),
    ];
    let open_gates: Vec<slint::SharedString> = verdict
        .open_gates()
        .iter()
        .map(|g| open_gate_label(g).into())
        .collect();
    TraceState {
        visible: true,
        title: TRACE_TITLE_VERDICT.into(),
        inputs: slint::ModelRc::new(slint::VecModel::from(inputs)),
        rule: format!(
            "{TRACE_RULE_PREFIX} {} · {TRACE_VERDICT_FORMULA}",
            snapshot.method_version()
        )
        .into(),
        open_gates: slint::ModelRc::new(slint::VecModel::from(open_gates)),
    }
}

/// One traceability input line: "label : value (provenance)". `None` value → the em-dash.
fn trace_input(
    label: &str,
    value: Option<Money>,
    field: DisplayField,
    format: NumberFormat,
) -> String {
    let shown = match value {
        Some(m) => format_scaled(m.as_decimal(), field, format),
        None => EMPTY_SLOT.to_string(),
    };
    // A typed judgment is the user's own input (provenance: manual) — the recorded gate-state reading.
    format!("{label} : {shown} ({PROVENANCE_MANUAL})")
}

/// The current judgment-input values surfaced as locale-formatted strings for the entry fields
/// (so reopening restores them) + the selected forecast-low option key.
pub fn judgment_fields(study: &Study, format: NumberFormat) -> JudgmentFields {
    let j = &study.judgment;
    let fmt_money = |v: Option<Money>, field: DisplayField| -> slint::SharedString {
        match v {
            Some(m) => format_scaled(m.as_decimal(), field, format).into(),
            None => slint::SharedString::new(),
        }
    };
    JudgmentFields {
        sales_growth: fmt_money(j.projected_sales_growth_pct, DisplayField::Percent),
        eps_growth: fmt_money(j.projected_eps_growth_pct, DisplayField::Percent),
        est_high_eps: fmt_money(j.estimated_high_eps, DisplayField::PerShare),
        est_low_eps: fmt_money(j.estimated_low_eps, DisplayField::PerShare),
        high_pe: fmt_money(j.judged_avg_high_pe, DisplayField::PeRatio),
        low_pe: fmt_money(j.judged_avg_low_pe, DisplayField::PeRatio),
        forecast_low_option: forecast_low_option_key(j.forecast_low_option).into(),
        recent_severe_low: fmt_money(j.recent_severe_low, DisplayField::Price),
        current_price: fmt_money(j.current_price, DisplayField::Price),
        dividend: fmt_money(j.present_full_year_dividend, DisplayField::PerShare),
    }
}

/// The stable selector key of a forecast-low option (the wire value the Slint selector passes back).
pub fn forecast_low_option_key(option: CForecastLowOption) -> &'static str {
    match option {
        CForecastLowOption::AvgLowPeTimesEps => "avg_low_pe_times_eps",
        CForecastLowOption::AvgLowPriceLast5y => "avg_low_price_last_5y",
        CForecastLowOption::RecentSevereLow => "recent_severe_low",
        CForecastLowOption::DividendSupported => "dividend_supported",
    }
}

/// Parse a forecast-low selector key back into the contract enum; `None` for an unknown key.
pub fn forecast_low_option_from_key(key: &str) -> Option<CForecastLowOption> {
    match key {
        "avg_low_pe_times_eps" => Some(CForecastLowOption::AvgLowPeTimesEps),
        "avg_low_price_last_5y" => Some(CForecastLowOption::AvgLowPriceLast5y),
        "recent_severe_low" => Some(CForecastLowOption::RecentSevereLow),
        "dividend_supported" => Some(CForecastLowOption::DividendSupported),
        _ => None,
    }
}

/// One open-gate → a fact-stating French line: "<input label> — <state noun>" (e.g. "BPA 2023 —
/// non validé"). Neutral nouns only (scanned in [`USER_FACING_LABELS`]).
fn open_gate_label(gate: &OpenGate) -> String {
    let input_label = match &gate.input {
        GatedInput::YearField { year, field } => format!("{} {year}", gate_field_label(field)),
        GatedInput::JudgmentInput { name } => gate_judgment_label(name).to_string(),
    };
    format!("{input_label} — {}", gate_state_noun(gate.state))
}

/// The fact-stating French noun for a non-green gate state (the `OpenGate.state` is never
/// `ValidatedFresh`). Scanned in [`USER_FACING_LABELS`].
fn gate_state_noun(state: GateState) -> &'static str {
    match state {
        GateState::Missing => GATE_MISSING,
        GateState::NotValidated => GATE_NOT_VALIDATED,
        GateState::Stale => GATE_STALE,
        GateState::ValidatedFresh => GATE_NON_GREEN, // never reached for an open gate
    }
}

/// French noun for a load-bearing year field.
fn gate_field_label(field: &str) -> &'static str {
    match field {
        "sales" => LBL_SALES,
        "eps" => LBL_EPS,
        "high_price" => LBL_HIGH_PRICE,
        "low_price" => LBL_LOW_PRICE,
        _ => LBL_UNKNOWN_FIELD,
    }
}

/// French noun for a load-bearing judgment input.
fn gate_judgment_label(name: &str) -> &'static str {
    match name {
        "estimated_high_eps" => LBL_EST_HIGH_EPS,
        "estimated_low_eps" => LBL_EST_LOW_EPS,
        "judged_avg_high_pe" => LBL_HIGH_PE,
        "judged_avg_low_pe" => LBL_LOW_PE,
        "current_price" => LBL_CURRENT_PRICE,
        _ => LBL_UNKNOWN_FIELD,
    }
}

/// The provenance date (DD/MM) of the most recent load-bearing cell edit, or the study's creation
/// date when no cell has been entered — the temporal-provenance caption's source (FR11).
fn provenance_dd_mm(study: &Study) -> String {
    let latest = study
        .years
        .iter()
        .flat_map(|y| {
            [
                Some(&y.sales),
                Some(&y.eps),
                Some(&y.high_price),
                Some(&y.low_price),
            ]
            .into_iter()
            .flatten()
            .map(|c| c.provenance.timestamp.0.clone())
        })
        .filter(|ts| !ts.is_empty())
        .max();
    let ts = latest.unwrap_or_else(|| study.created_at.0.clone());
    dd_mm(&ts)
}

/// "YYYY-MM-DDT…" → "DD/MM"; a non-RFC3339 string passes through unchanged (a display transform).
fn dd_mm(ts: &str) -> String {
    let date = ts.split('T').next().unwrap_or(ts);
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() >= 3 {
        format!("{}/{}", parts[2], parts[1])
    } else {
        ts.to_string()
    }
}

// ── User-facing label inventory (Story 2.6) — scanned by the posture gate (FR13). Fact-stating
//    nouns only; never an imperative. Registered in `posture.rs`. ──

pub const LBL_SALES: &str = "Ventes";
pub const LBL_EPS: &str = "BPA";
pub const LBL_HIGH_PRICE: &str = "Prix haut";
pub const LBL_LOW_PRICE: &str = "Prix bas";
pub const LBL_EST_HIGH_EPS: &str = "BPA estimé haut";
pub const LBL_EST_LOW_EPS: &str = "BPA estimé bas";
pub const LBL_HIGH_PE: &str = "PER haut moyen";
pub const LBL_LOW_PE: &str = "PER bas moyen";
pub const LBL_CURRENT_PRICE: &str = "Prix actuel";
pub const LBL_UNKNOWN_FIELD: &str = "Entrée";
pub const GATE_NON_GREEN: &str = "à reprendre";
pub const GATE_MISSING: &str = "manquant";
pub const GATE_NOT_VALIDATED: &str = "non validé";
pub const GATE_STALE: &str = "périmé";
pub const TREND_UP: &str = "hausse";
pub const TREND_EVEN: &str = "stable";
pub const TREND_DOWN: &str = "baisse";
pub const PROVENANCE_MANUAL: &str = "manuel";
pub const TRACE_TITLE_VERDICT: &str = "Conclusion — entrées, provenance & règle";
pub const TRACE_RULE_PREFIX: &str = "Méthode";
pub const TRACE_VERDICT_FORMULA: &str = "zones §4 + ratio H/B + appréciation §5";
/// The FR8 low-confidence reason carried onto the verdict surface (Story 2.7, AC1). Fact-stating,
/// no imperative — scanned by the posture gate alongside the other engine labels.
pub const CONFIDENCE_LOW: &str = "Historique insuffisant — confiance réduite";

/// Every Story-2.6 Rust-side user-facing label, exposed so the crate-local posture gate (FR13)
/// scans them for banned verbs alongside the `@tr()` literals and `state::USER_FACING_MESSAGES`.
#[cfg(test)]
pub const USER_FACING_LABELS: &[&str] = &[
    LBL_SALES,
    LBL_EPS,
    LBL_HIGH_PRICE,
    LBL_LOW_PRICE,
    LBL_EST_HIGH_EPS,
    LBL_EST_LOW_EPS,
    LBL_HIGH_PE,
    LBL_LOW_PE,
    LBL_CURRENT_PRICE,
    LBL_UNKNOWN_FIELD,
    GATE_NON_GREEN,
    GATE_MISSING,
    GATE_NOT_VALIDATED,
    GATE_STALE,
    TREND_UP,
    TREND_EVEN,
    TREND_DOWN,
    PROVENANCE_MANUAL,
    TRACE_TITLE_VERDICT,
    TRACE_RULE_PREFIX,
    TRACE_VERDICT_FORMULA,
    CONFIDENCE_LOW,
];

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::{
        Cell, Coverage, ForecastLowOption as CFlo, Freshness, Judgment, Provenance, Review, Source,
        Timestamp, YearData,
    };
    use uuid::Uuid;

    fn money(s: &str) -> Money {
        Money::from(Decimal::from_str_exact(s).unwrap())
    }

    fn prov() -> Provenance {
        Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: Timestamp("2026-03-09T00:00:00Z".to_string()),
            hash_of_dependencies: "manual".to_string(),
        }
    }

    fn validated_cell(value: &str) -> Cell {
        Cell {
            value: Some(money(value)),
            source: Source::Manual,
            freshness: Freshness::Current,
            review: Review::Validated,
            coverage: Coverage::Present,
            provenance: prov(),
        }
    }

    fn unreviewed_cell(value: &str) -> Cell {
        Cell {
            review: Review::None,
            ..validated_cell(value)
        }
    }

    fn year(y: i32, cell: impl Fn(&str) -> Cell) -> YearData {
        YearData {
            year: y,
            sales: cell("1000"),
            eps: cell("5"),
            high_price: cell("100"),
            low_price: cell("50"),
            dividend_per_share: Some(cell("2")),
            pre_tax_profit: Some(cell("200")),
            book_value_per_share: Some(cell("40")),
        }
    }

    fn full_judgment() -> Judgment {
        Judgment {
            estimated_high_eps: Some(money("8")),
            estimated_low_eps: Some(money("3")),
            projected_sales_growth_pct: Some(money("10")),
            projected_eps_growth_pct: Some(money("10")),
            judged_avg_high_pe: Some(money("20")),
            judged_avg_low_pe: Some(money("10")),
            forecast_low_option: CFlo::AvgLowPeTimesEps,
            recent_severe_low: None,
            current_price: Some(money("60")),
            present_full_year_dividend: Some(money("2")),
        }
    }

    fn study_with(years: Vec<YearData>, judgment: Judgment) -> Study {
        let mut s = Study::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "NESN",
            "CHF",
            judgment,
            Timestamp("2026-06-13T09:30:00Z".to_string()),
        );
        s.years = years;
        s
    }

    /// 5 fully-validated-fresh usable years + complete judgment → Verdict::Full, and the adapter's
    /// snapshot outputs match a direct `compute()` on the same mapped inputs (no drift).
    #[test]
    fn snapshot_matches_direct_compute_and_derives_full() {
        let years: Vec<YearData> = (2021..=2025).map(|y| year(y, validated_cell)).collect();
        let study = study_with(years, full_judgment());

        let snap = build_snapshot(&study).expect("normalizes");

        // No adapter drift: the snapshot outputs equal a direct compute on the same mapping.
        let raw = to_raw_financials(&study);
        let canonical = normalize::normalize(raw).unwrap();
        let direct = steadyinvest_core::compute(
            &canonical,
            &to_judgment_inputs(&study.judgment),
            &to_observations(&study),
        );
        assert_eq!(snap.outputs(), &direct, "adapter must introduce no drift");

        assert!(
            matches!(snap.verdict(), Verdict::Full(_)),
            "all-validated-fresh + full confidence must derive Full, got {:?}",
            snap.verdict()
        );
        assert!(snap.verdict().open_gates().is_empty());
    }

    /// A missing load-bearing judgment input (current_price) → Verdict::Withheld, names it.
    #[test]
    fn missing_load_bearing_judgment_input_withholds() {
        let years: Vec<YearData> = (2021..=2025).map(|y| year(y, validated_cell)).collect();
        let judgment = Judgment {
            current_price: None,
            ..full_judgment()
        };
        let study = study_with(years, judgment);
        let snap = build_snapshot(&study).unwrap();
        assert!(
            matches!(snap.verdict(), Verdict::Withheld(_)),
            "a missing load-bearing input must withhold, got {:?}",
            snap.verdict()
        );
        assert!(snap.verdict().open_gates().iter().any(|g| matches!(
            g.input,
            GatedInput::JudgmentInput {
                name: "current_price"
            }
        )));
    }

    /// One un-validated load-bearing data cell (review ≠ ✓), nothing missing → Verdict::Provisional.
    #[test]
    fn one_unvalidated_cell_is_provisional() {
        let mut years: Vec<YearData> = (2021..=2025).map(|y| year(y, validated_cell)).collect();
        years[2].eps = unreviewed_cell("5"); // present, but review = None → NotValidated
        let study = study_with(years, full_judgment());
        let snap = build_snapshot(&study).unwrap();
        assert!(
            matches!(snap.verdict(), Verdict::Provisional(_)),
            "an un-validated load-bearing cell (nothing missing) must be Provisional, got {:?}",
            snap.verdict()
        );
        assert!(!snap.verdict().low_confidence());
    }

    /// Fewer than 5 usable years → low_confidence → Verdict::Provisional (even when every present
    /// gate is validated-fresh).
    #[test]
    fn low_confidence_under_five_usable_years_is_provisional() {
        let years: Vec<YearData> = (2023..=2025).map(|y| year(y, validated_cell)).collect();
        let study = study_with(years, full_judgment());
        let snap = build_snapshot(&study).unwrap();
        match snap.verdict() {
            Verdict::Provisional(d) => assert!(d.low_confidence()),
            other => panic!("3 usable years must be low-confidence Provisional, got {other:?}"),
        }
    }

    /// The gate-state mapping table (AC 6).
    #[test]
    fn cell_and_judgment_gate_state_mapping() {
        use steadyinvest_contract::{Freshness, Review};
        assert_eq!(cell_to_gate_state(None), GateState::Missing);
        let mut c = validated_cell("1");
        assert_eq!(cell_to_gate_state(Some(&c)), GateState::ValidatedFresh);
        c.freshness = Freshness::Stale;
        assert_eq!(cell_to_gate_state(Some(&c)), GateState::Stale);
        c.review = Review::ToReview;
        c.freshness = Freshness::Current;
        assert_eq!(cell_to_gate_state(Some(&c)), GateState::NotValidated);

        assert_eq!(judgment_to_gate_state(None), GateState::Missing);
        assert_eq!(
            judgment_to_gate_state(Some(money("1"))),
            GateState::ValidatedFresh
        );
    }

    /// The forecast-low option glue is total and round-trips by key.
    #[test]
    fn forecast_low_option_glue_round_trips() {
        for option in [
            CFlo::AvgLowPeTimesEps,
            CFlo::AvgLowPriceLast5y,
            CFlo::RecentSevereLow,
            CFlo::DividendSupported,
        ] {
            let key = forecast_low_option_key(option);
            assert_eq!(forecast_low_option_from_key(key), Some(option));
            // by-name into core mirrors the contract variant.
            let _ = to_forecast_low_option(option);
        }
        assert_eq!(forecast_low_option_from_key("garbage"), None);
    }

    /// Adapter formatting: a known output → a grouped string; an unknown metric → the em-dash,
    /// never `0`.
    #[test]
    fn adapter_formats_known_and_unknown_outputs() {
        let years: Vec<YearData> = (2021..=2025).map(|y| year(y, validated_cell)).collect();
        let study = study_with(years, full_judgment());
        let snap = build_snapshot(&study).unwrap();

        let pe = pe_computed(snap.outputs(), NumberFormat::Comma);
        // avg P/E = (20 + 10) / 2 = 15 with judged values; the §3 window P/Es derive from prices/eps.
        assert_ne!(
            pe.avg_high_pe.as_str(),
            EMPTY_SLOT,
            "a known avg P/E is shown"
        );

        // current P/E is unknown in v1 (empty quarterly observations) → em-dash, never 0.
        assert_eq!(
            pe.current_pe.as_str(),
            EMPTY_SLOT,
            "current P/E is honestly unknown (no quarterly data), never 0"
        );

        let risk = risk_computed(snap.outputs(), NumberFormat::Comma);
        assert_ne!(
            risk.forecast_high.as_str(),
            EMPTY_SLOT,
            "forecast high is known"
        );
    }

    /// An empty-year study still normalizes and yields a Withheld verdict (everything missing) —
    /// never a panic, never a fabricated number.
    #[test]
    fn empty_study_withholds_without_panic() {
        let study = study_with(vec![], Judgment { ..full_judgment() });
        // Clear judgment too so a load-bearing input is missing.
        let mut study = study;
        study.judgment.current_price = None;
        let snap = build_snapshot(&study).unwrap();
        assert!(matches!(snap.verdict(), Verdict::Withheld(_)));
    }

    #[test]
    fn dd_mm_extracts_day_and_month() {
        assert_eq!(dd_mm("2026-06-13T09:00:00Z"), "13/06");
        assert_eq!(dd_mm("weird"), "weird");
    }

    /// Task 5 adapter test: `verdict_badge` maps each `Verdict` state → the right state string, and
    /// the temporal-provenance caption is a PROVISIONAL-only fact (AC 6) — absent for Full (no
    /// qualifier needed) AND for Withheld (nothing computed to date-stamp); a withheld verdict names
    /// its open gate(s).
    #[test]
    fn verdict_badge_maps_state_and_restricts_provenance_to_provisional() {
        use slint::Model;
        let fmt = NumberFormat::Comma;

        // Full → "full", no provenance caption, no open gates.
        let full = study_with(
            (2021..=2025).map(|y| year(y, validated_cell)).collect(),
            full_judgment(),
        );
        let snap = build_snapshot(&full).unwrap();
        let badge = verdict_badge(&full, &snap, fmt);
        assert_eq!(badge.state.as_str(), "full");
        assert_eq!(
            badge.provenance_date.as_str(),
            "",
            "a Full verdict needs no temporal-provenance qualifier"
        );
        assert_eq!(badge.open_gates.row_count(), 0);

        // Provisional (one un-validated load-bearing cell) → "provisional" + the data DD/MM
        // (the prov() timestamp is 2026-03-09 → "09/03").
        let mut prov_years: Vec<YearData> =
            (2021..=2025).map(|y| year(y, validated_cell)).collect();
        prov_years[2].eps = unreviewed_cell("5");
        let prov = study_with(prov_years, full_judgment());
        let snap = build_snapshot(&prov).unwrap();
        let badge = verdict_badge(&prov, &snap, fmt);
        assert_eq!(badge.state.as_str(), "provisional");
        assert_eq!(
            badge.provenance_date.as_str(),
            "09/03",
            "a Provisional verdict carries the data DD/MM"
        );

        // Withheld (a missing load-bearing input) → "withheld", NO provenance caption, gate named.
        let withheld = study_with(
            (2021..=2025).map(|y| year(y, validated_cell)).collect(),
            Judgment {
                current_price: None,
                ..full_judgment()
            },
        );
        let snap = build_snapshot(&withheld).unwrap();
        let badge = verdict_badge(&withheld, &snap, fmt);
        assert_eq!(badge.state.as_str(), "withheld");
        assert_eq!(
            badge.provenance_date.as_str(),
            "",
            "a Withheld verdict computed nothing to date-stamp (AC 6)"
        );
        assert!(
            badge.open_gates.row_count() >= 1,
            "a withheld verdict names its open gate(s)"
        );
    }

    /// Task 6 adapter test: `verdict_trace` names the load-bearing judgment inputs it descends from,
    /// the method-identity rule line, and — for a degraded verdict — the open gates (the honest
    /// "why not full").
    #[test]
    fn verdict_trace_names_inputs_rule_and_open_gates_for_a_degraded_verdict() {
        use slint::Model;
        let withheld = study_with(
            (2021..=2025).map(|y| year(y, validated_cell)).collect(),
            Judgment {
                current_price: None,
                ..full_judgment()
            },
        );
        let snap = build_snapshot(&withheld).unwrap();
        let trace = verdict_trace(&withheld, &snap, NumberFormat::Comma);
        assert!(trace.visible);
        assert_eq!(
            trace.inputs.row_count(),
            5,
            "the five load-bearing judgment inputs are listed with provenance"
        );
        assert!(
            trace.rule.as_str().contains(snap.method_version()),
            "the rule line carries the method identity"
        );
        assert!(
            trace.open_gates.row_count() >= 1,
            "a degraded verdict surfaces its open gate(s)"
        );
    }

    // ── Story 2.7 — plausibility surfacing + the low-confidence label ──

    /// AC2/AC3: each `(year, context)` maps to its expected §2/§3 cell address against the
    /// materialized-year window; the derived-ratio fallback lands on the contributing input cell.
    #[test]
    fn plausibility_maps_findings_to_their_cell_addresses() {
        let years = vec![2021, 2022, 2023, 2024, 2025];
        let input = vec![
            Finding {
                key: PlausibilityKey::SplitSeriesBreak,
                year: 2023,
                context: "eps",
            },
            Finding {
                key: PlausibilityKey::CurrencyMismatch,
                year: 2021,
                context: "high_price",
            },
        ];
        let calc = vec![
            CalcFinding {
                key: PlausibilityKey::NegativeOrZeroDenominator,
                year: Some(2022),
                context: "sales",
            },
            // a derived ratio (ptp_pct) anchors at the §2 pre-tax-profit input that contributes it.
            CalcFinding {
                key: PlausibilityKey::OutOfBoundsRatio,
                year: Some(2025),
                context: "ptp_pct",
            },
            // a §3 low-P/E bound anchors at the low-price input cell.
            CalcFinding {
                key: PlausibilityKey::OutOfBoundsRatio,
                year: Some(2024),
                context: "low_pe",
            },
        ];
        let w = plausibility(&input, &calc, &years);
        assert_eq!(
            w.cell_key(2, entry::FIELD_EPS),
            Some(PlausibilityKey::SplitSeriesBreak)
        );
        assert_eq!(
            w.cell_key(0, entry::FIELD_HIGH),
            Some(PlausibilityKey::CurrencyMismatch)
        );
        assert_eq!(
            w.cell_key(1, entry::FIELD_SALES),
            Some(PlausibilityKey::NegativeOrZeroDenominator)
        );
        assert_eq!(
            w.cell_key(4, entry::FIELD_PRETAX),
            Some(PlausibilityKey::OutOfBoundsRatio)
        );
        assert_eq!(
            w.cell_key(3, entry::FIELD_LOW),
            Some(PlausibilityKey::OutOfBoundsRatio)
        );
        // A cell with no finding is silent (AC6).
        assert_eq!(w.cell_key(3, entry::FIELD_HIGH), None);
    }

    /// Anchor policy: fiscal-metadata and study-level / out-of-window findings are RETAINED (never
    /// dropped) and resolve to their year-level / §4 anchors — never mis-attached to a value cell.
    #[test]
    fn non_cell_findings_anchor_at_year_or_study_never_dropped_or_misattached() {
        let years = vec![2023, 2024, 2025];
        let input = vec![
            // fiscal metadata → the whole year is suspect, not one value cell.
            Finding {
                key: PlausibilityKey::FiscalPeriodMisalignment,
                year: 2024,
                context: "fiscal_year_end_month",
            },
            // a finding on a year OUTSIDE the window → §4/study fallback (anchored, not dropped).
            Finding {
                key: PlausibilityKey::CurrencyMismatch,
                year: 1999,
                context: "sales",
            },
        ];
        let calc = vec![
            CalcFinding {
                key: PlausibilityKey::LowPriceAboveCurrent,
                year: None,
                context: "forecast_low",
            },
            CalcFinding {
                key: PlausibilityKey::OutOfBoundsRatio,
                year: None,
                context: "current_pe",
            },
        ];
        let w = plausibility(&input, &calc, &years);
        assert_eq!(w.items.len(), 4, "every finding is retained, none dropped");
        assert!(
            w.study_key().is_some(),
            "a study-level warning surfaces near §4, not at a cell"
        );
        assert!(
            w.items
                .iter()
                .any(|it| it.key == PlausibilityKey::LowPriceAboveCurrent
                    && it.anchor == WarningAnchor::Study),
            "the year-less forecast_low finding anchors at the §4/study surface"
        );
        // The fiscal metadata anchors at the year (index 1), never at a value cell.
        assert!(w
            .items
            .iter()
            .any(|it| it.anchor == WarningAnchor::Year { year_index: 1 }));
        // forecast_low + current_pe (year None) + the out-of-window currency finding → §4/study.
        assert_eq!(
            w.items
                .iter()
                .filter(|it| it.anchor == WarningAnchor::Study)
                .count(),
            3
        );
        // No fiscal/study finding leaks into a §2/§3 cell address.
        assert!(w.cell_key(1, entry::FIELD_SALES).is_none());
    }

    /// AC1/AC6: a study with fewer than five usable years carries the explicit low-confidence label on
    /// the verdict; a clean ≥5-year study does not (and Full never does, by construction).
    #[test]
    fn low_confidence_label_only_under_five_usable_years() {
        let fmt = NumberFormat::Comma;

        let thin = study_with(
            (2023..=2025).map(|y| year(y, validated_cell)).collect(),
            full_judgment(),
        );
        let snap = build_snapshot(&thin).unwrap();
        let badge = verdict_badge(&thin, &snap, fmt);
        assert!(badge.low_confidence, "3 usable years is low-confidence");
        assert_eq!(
            badge.confidence_label.as_str(),
            CONFIDENCE_LOW,
            "the FR8 reason is carried as explicit text"
        );

        let full = study_with(
            (2021..=2025).map(|y| year(y, validated_cell)).collect(),
            full_judgment(),
        );
        let snap = build_snapshot(&full).unwrap();
        let badge = verdict_badge(&full, &snap, fmt);
        assert!(
            !badge.low_confidence,
            "five usable years is full confidence"
        );
        assert_eq!(
            badge.confidence_label.as_str(),
            "",
            "no low-confidence reason is shown when history suffices (AC6)"
        );
    }

    /// AC6/AC7: a clean ≥5-year validated study raises no plausibility findings (the channels are
    /// silent) and is not low-confidence — both finding sets read off ONE coherent frame.
    #[test]
    fn a_clean_five_year_study_is_silent_and_full_confidence() {
        let study = study_with(
            (2021..=2025).map(|y| year(y, validated_cell)).collect(),
            full_judgment(),
        );
        let frame = build_frame(&study).expect("normalizes");
        let years: Vec<i32> = (2021..=2025).collect();
        let w = plausibility(
            &frame.plausibility,
            &frame.snapshot.outputs().findings,
            &years,
        );
        assert!(
            w.items.is_empty(),
            "a clean study raises no plausibility findings (AC6), got {:?}",
            w.items
        );
        assert!(
            !frame.snapshot.verdict().low_confidence(),
            "five usable years is not low-confidence"
        );
    }
}
