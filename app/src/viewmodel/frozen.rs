//! Story 8.8 — the frozen verdict against the current one (UX spec §4.6, §5.9; FR68, A13).
//!
//! Pure: the stored `frozen_verdict` and the current verdict in the same shape
//! (`report::form::verdict_record`, computed live, never persisted) → the strip the study shows:
//! nothing, the « identique » caption, or the • band with its comparison table and cause line.
//! Every string is app text (posture-scanned below); zones travel as keys the Slint side writes
//! with its runtime labels, in ink.

use rust_decimal::Decimal;
use steadyinvest_contract::{
    ForecastLowOption, FrozenCriterion, FrozenUpsideDownside, FrozenVerdict, FrozenZone, Money,
    Study, Timestamp,
};
use steadyinvest_core::rounding::DisplayField;

use crate::viewmodel::drafts::option_label;
use crate::viewmodel::engine::{
    LBL_CURRENT_PRICE, LBL_EPS, LBL_EST_HIGH_EPS, LBL_EST_LOW_EPS, LBL_HIGH_PE, LBL_HIGH_PRICE,
    LBL_LOW_PE, LBL_LOW_PRICE, LBL_SALES, LBL_UNKNOWN_FIELD, TOTAL_RETURN_NO_DIV,
};
use crate::viewmodel::format::{NumberFormat, format_amount, format_scaled};
use crate::viewmodel::history::{
    HIST_EMPTY_SLOT, LBL_BOOK_VALUE, LBL_DIVIDEND_PS, LBL_DIVIDEND_YEAR, LBL_EPS_GROWTH,
    LBL_FORECAST_LOW_OPTION, LBL_PRETAX_PROFIT, LBL_SALES_GROWTH, LBL_SEVERE_LOW, LBL_TTM_EPS,
};
use steadyinvest_report::frozen::{causes, changed_inputs};

// ── Vocabulary (spec §3.3 where it names the words; Decisions 12–13 otherwise) ──

pub const FROZEN_QV_MET: &str = "critères réunis";
pub const FROZEN_QV_UNMET: &str = "critères non réunis";
pub const FROZEN_CRIT_MET: &str = "réuni";
pub const FROZEN_CRIT_UNMET: &str = "non réuni";
pub const FROZEN_CRIT_UNKNOWN: &str = "inconnu";
pub const FROZEN_UD_TARGET: &str = "{value} · ≥ 3 : {crit}";
pub const FROZEN_RV_CEILING: &str = "{value} · < 100 % : {crit}";
pub const FROZEN_APPRECIATION_DOUBLE: &str = "{value} · ≥ doublement : {crit}";
pub const FROZEN_ENTRIES_SAME: &str = "identiques";
pub const FROZEN_ENTRIES_CHANGED: &str = "{n} modifiée(s) : {list}";
pub const FROZEN_ENTRY_CHANGE: &str = "{champ} {fige} → {actuel}";
pub const FROZEN_WITHHELD: &str = "retenu — entrées ouvertes : {list}";
pub const FROZEN_Q_QUARTER: &str = "Trimestre";

#[cfg(test)]
use steadyinvest_report::frozen::{
    CAUSE_AI, CAUSE_METHOD, CAUSE_OWNER, CAUSE_REFRESH, CAUSE_UNKNOWN,
};

/// Every Rust-side string of the strip, scanned by the posture gate (FR13).
#[cfg(test)]
pub const FROZEN_USER_FACING_LABELS: &[&str] = &[
    FROZEN_QV_MET,
    FROZEN_QV_UNMET,
    FROZEN_CRIT_MET,
    FROZEN_CRIT_UNMET,
    FROZEN_CRIT_UNKNOWN,
    FROZEN_UD_TARGET,
    FROZEN_RV_CEILING,
    FROZEN_APPRECIATION_DOUBLE,
    FROZEN_ENTRIES_SAME,
    FROZEN_ENTRIES_CHANGED,
    FROZEN_ENTRY_CHANGE,
    FROZEN_WITHHELD,
    CAUSE_REFRESH,
    CAUSE_OWNER,
    CAUSE_AI,
    CAUSE_METHOD,
    CAUSE_UNKNOWN,
    FROZEN_Q_QUARTER,
];

/// The current verdict's state, for the « actuel (…) » column header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentState {
    Full,
    Provisional,
    Withheld,
}

/// One row of the comparison: both sides, app-formatted, and whether it changed (• + semibold).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompareRow {
    pub frozen: String,
    pub current: String,
    pub changed: bool,
}

/// The eight rows of spec §5.9, in order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Comparison {
    pub verdict: CompareRow,
    /// The zone row's values are KEYS (`buy` / `neutral` / `sell` / ``), written by Slint with
    /// its runtime labels.
    pub zone: CompareRow,
    pub ud: CompareRow,
    pub relative_value: CompareRow,
    pub appreciation: CompareRow,
    pub potential: CompareRow,
    pub entries: CompareRow,
    pub method: CompareRow,
    /// « Cause : » what follows (causes joined by « , »).
    pub cause: String,
}

/// What the strip shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StripView {
    /// No frozen verdict: nothing rendered.
    None,
    /// Frozen = current: the one-line caption.
    Same { date: String },
    /// They differ: the • band and the comparison.
    Differs {
        date: String,
        frozen_method: String,
        current_method: String,
        current_state: CurrentState,
        comparison: Box<Comparison>,
    },
}

/// JJ/MM of an RFC 3339 stamp (its UTC date).
pub fn day_month(stamp: &Timestamp) -> String {
    // G3: the stamp's UTC date, the PDF's and the history's rule — one date on every surface.
    steadyinvest_report::frozen::day_month(stamp)
}

/// Build the strip (Story 8.8). `current`: the live verdict in the frozen shape; `open_inputs`:
/// what keeps it from being full (`None` when full).
pub fn strip(
    study: &Study,
    current: &FrozenVerdict,
    current_state: CurrentState,
    open_inputs: Option<&str>,
    format: NumberFormat,
) -> StripView {
    let Some(frozen) = study.frozen_verdict.as_ref() else {
        return StripView::None;
    };
    let date = day_month(&frozen.frozen_at);
    let full = current_state == CurrentState::Full;
    if !steadyinvest_report::frozen::differs(frozen, current, full) {
        return StripView::Same { date };
    }
    let withheld = |text: String| match (current_state, open_inputs) {
        (CurrentState::Withheld, Some(list)) => FROZEN_WITHHELD.replace("{list}", list),
        _ => text,
    };
    // A row is marked when what it SHOWS differs (G3: never a « • » over two identical texts —
    // the PDF's rule too).
    let row = |f: String, c: String| {
        let current = withheld(c);
        CompareRow {
            changed: f != current,
            current,
            frozen: f,
        }
    };
    let qv = |b: bool| if b { FROZEN_QV_MET } else { FROZEN_QV_UNMET }.to_string();
    let zone_key = |z: Option<FrozenZone>| match z {
        Some(FrozenZone::Low) => "buy",
        Some(FrozenZone::Middle) => "neutral",
        Some(FrozenZone::High) => "sell",
        None => "",
    };
    let ud = |v: &FrozenVerdict| {
        FROZEN_UD_TARGET
            .replace("{value}", &fmt_ud(&v.upside_downside, format))
            .replace("{crit}", crit(v.ud_at_or_above_target))
    };
    let rv = |v: &FrozenVerdict| {
        FROZEN_RV_CEILING
            .replace("{value}", &fmt_pct(v.relative_value_pct.as_ref(), format))
            .replace("{crit}", crit(v.relative_value_below_ceiling))
    };
    let app = |v: &FrozenVerdict| {
        FROZEN_APPRECIATION_DOUBLE
            .replace(
                "{value}",
                &fmt_pct(v.projected_appreciation_pct.as_ref(), format),
            )
            .replace("{crit}", crit(v.appreciation_at_or_above_double))
    };
    let potential = |v: &FrozenVerdict| match (&v.total_return_pct, &v.appreciation_only_pct) {
        (Some(t), _) => fmt_pct(Some(t), format),
        (None, Some(a)) => format!("{} ({TOTAL_RETURN_NO_DIV})", fmt_pct(Some(a), format)),
        (None, None) => HIST_EMPTY_SLOT.to_string(),
    };
    let changed_inputs = changed_inputs(frozen, current);
    let entries = if changed_inputs.is_empty() {
        CompareRow {
            frozen: FROZEN_ENTRIES_SAME.to_string(),
            current: FROZEN_ENTRIES_SAME.to_string(),
            changed: false,
        }
    } else {
        let list: Vec<String> = changed_inputs
            .iter()
            .map(|(k, f, c)| {
                FROZEN_ENTRY_CHANGE
                    .replace("{champ}", &input_label(k))
                    .replace("{fige}", &input_value(k, f, format))
                    .replace("{actuel}", &input_value(k, c, format))
            })
            .collect();
        let text = FROZEN_ENTRIES_CHANGED
            .replace("{n}", &list.len().to_string())
            .replace("{list}", &list.join(", "));
        CompareRow {
            frozen: String::new(),
            current: text,
            changed: true,
        }
    };
    let comparison = Comparison {
        verdict: {
            let mut r = row(
                qv(frozen.quality_value_candidate),
                qv(current.quality_value_candidate),
            );
            // A current verdict no longer full is itself the difference (G3).
            r.changed |= !full;
            r
        },
        zone: {
            let (f, c) = (
                zone_key(frozen.present_zone).to_string(),
                zone_key(current.present_zone).to_string(),
            );
            // Keys the strip writes with its labels; a withheld cell passes its text through.
            let c = withheld(c);
            CompareRow {
                changed: f != c,
                frozen: f,
                current: c,
            }
        },
        ud: row(ud(frozen), ud(current)),
        relative_value: row(rv(frozen), rv(current)),
        appreciation: row(app(frozen), app(current)),
        potential: row(potential(frozen), potential(current)),
        entries,
        method: CompareRow {
            frozen: frozen.method_version.clone(),
            current: current.method_version.clone(),
            changed: frozen.method_version != current.method_version,
        },
        cause: causes(
            study,
            frozen,
            current,
            &changed_inputs,
            if full { None } else { open_inputs },
            &day_month,
        ),
    };
    StripView::Differs {
        date,
        frozen_method: frozen.method_version.clone(),
        current_method: current.method_version.clone(),
        current_state,
        comparison: Box::new(comparison),
    }
}

fn crit(c: FrozenCriterion) -> &'static str {
    match c {
        FrozenCriterion::Met => FROZEN_CRIT_MET,
        FrozenCriterion::Unmet => FROZEN_CRIT_UNMET,
        FrozenCriterion::UnmetByInsufficiency => FROZEN_CRIT_UNKNOWN,
    }
}

fn fmt_ud(ud: &FrozenUpsideDownside, format: NumberFormat) -> String {
    match ud {
        FrozenUpsideDownside::Ratio(r) => format!(
            "{}:1",
            format_scaled(r.as_decimal(), DisplayField::Ratio, format)
        ),
        _ => HIST_EMPTY_SLOT.to_string(),
    }
}

fn fmt_pct(v: Option<&Money>, format: NumberFormat) -> String {
    match v {
        Some(m) => format!(
            "{} %",
            format_scaled(m.as_decimal(), DisplayField::Percent, format)
        ),
        None => HIST_EMPTY_SLOT.to_string(),
    }
}

/// « {champ} » of an input key (`y2024.eps` → « BPA 2024 », `j.current_price` → « Prix actuel »).
fn input_label(key: &str) -> String {
    if let Some(rest) = key.strip_prefix('y')
        && let Some((year, field)) = rest.split_once('.')
    {
        let label = match field {
            "sales" => LBL_SALES,
            "eps" => LBL_EPS,
            "high_price" => LBL_HIGH_PRICE,
            "low_price" => LBL_LOW_PRICE,
            "dividend_per_share" => LBL_DIVIDEND_PS,
            "pre_tax_profit" => LBL_PRETAX_PROFIT,
            "book_value_per_share" => LBL_BOOK_VALUE,
            _ => LBL_UNKNOWN_FIELD,
        };
        return format!("{label} {year}");
    }
    match key {
        "j.estimated_high_eps" => LBL_EST_HIGH_EPS,
        "j.estimated_low_eps" => LBL_EST_LOW_EPS,
        "j.projected_sales_growth_pct" => LBL_SALES_GROWTH,
        "j.projected_eps_growth_pct" => LBL_EPS_GROWTH,
        "j.judged_avg_high_pe" => LBL_HIGH_PE,
        "j.judged_avg_low_pe" => LBL_LOW_PE,
        "j.recent_severe_low" => LBL_SEVERE_LOW,
        "j.current_price" => LBL_CURRENT_PRICE,
        "j.present_full_year_dividend" => LBL_DIVIDEND_YEAR,
        "j.forecast_low_option" => LBL_FORECAST_LOW_OPTION,
        "q.ttm_quarterly_eps" => LBL_TTM_EPS,
        k if k.starts_with("q.") => FROZEN_Q_QUARTER,
        _ => LBL_UNKNOWN_FIELD,
    }
    .to_string()
}

/// An input's value as the owner reads it: the locale's number, the option's label, « — ».
fn input_value(key: &str, value: &str, format: NumberFormat) -> String {
    if value == steadyinvest_report::form::ABSENT {
        return HIST_EMPTY_SLOT.to_string();
    }
    if key == "j.forecast_low_option" {
        for o in [
            ForecastLowOption::AvgLowPeTimesEps,
            ForecastLowOption::AvgLowPriceLast5y,
            ForecastLowOption::RecentSevereLow,
            ForecastLowOption::DividendSupported,
        ] {
            if steadyinvest_contract::option_name(o) == value {
                return option_label(o).to_string();
            }
        }
        return value.to_string();
    }
    // The trailing-twelve-month EPS travels as four quarters, the app filling the first (its TTM
    // figure) and zeros: show the figure only (G3).
    if key == "q.ttm_quarterly_eps"
        && let Some(first) = value.split('|').next()
    {
        return format_amount(first, format);
    }
    if value.contains('|') {
        return value
            .split('|')
            .map(|v| format_amount(v, format))
            .collect::<Vec<_>>()
            .join(" · ");
    }
    match value.parse::<Decimal>() {
        Ok(_) => format_amount(value, format),
        Err(_) => value.to_string(),
    }
}

/// The strip of a study, from ONE frame (the current verdict live, never persisted): the entry
/// point of the wiring and the tests. A study that does not normalize has no current verdict to
/// compare: no strip (the study screen already names the normalize failure).
#[cfg(test)]
pub fn strip_of(study: &Study, now: &Timestamp, format: NumberFormat) -> StripView {
    if study.frozen_verdict.is_none() {
        return StripView::None;
    }
    let Ok(frame) = steadyinvest_report::form::build_frame(study) else {
        return StripView::None;
    };
    let current = steadyinvest_report::form::verdict_record(study, &frame, now);
    let state = match frame.snapshot.verdict() {
        steadyinvest_core::verdict::Verdict::Full(_) => CurrentState::Full,
        steadyinvest_core::verdict::Verdict::Provisional(_) => CurrentState::Provisional,
        steadyinvest_core::verdict::Verdict::Withheld(_) => CurrentState::Withheld,
    };
    let open = crate::viewmodel::engine::open_inputs(&frame.snapshot);
    strip(study, &current, state, open.as_deref(), format)
}
