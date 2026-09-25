//! Story 7.1 — one study → one comparison column (the form's thirty rows), formatted at this
//! boundary from the SAME frame the study screen shows (`build_frame`: no drift). Four rows are
//! min / max / × over engine outputs (rows 8, 9, 11, 15) — pure, tested here; the rest is a
//! straight read. The two worded rows (20 zone, 28 state) cross as keys; the report and the
//! screen word them in their own inventories.

use rust_decimal::Decimal;
use steadyinvest_contract::{Source, Study};
use steadyinvest_core::method::USABLE_YEARS_FLOOR;
use steadyinvest_core::normalize::CanonicalYear;
use steadyinvest_core::rounding::DisplayField;
use steadyinvest_core::ssg::{JudgmentInputs, SsgOutputs, YearValuation, quality_flags_assessable};
use steadyinvest_report::ComparisonColumn;

use crate::state;
use crate::viewmodel::engine::{
    fmt, fmt_pct, fmt_total_return, fmt_trend, fmt_ud, verdict_state, zone_position_key,
};
use crate::viewmodel::form::EMPTY_SLOT;
use crate::viewmodel::format::{NumberFormat, format_scaled};

/// The column of a study that could not be read (a read failure): every row « indisponible »
/// (the report's word). `label` is the pick's label, shown as is (already upper-cased).
pub fn unavailable_column(label: &str) -> ComparisonColumn {
    ComparisonColumn {
        ticker: label.to_string(),
        unavailable: true,
        rows: vec![String::new(); 30],
        ..ComparisonColumn::default()
    }
}

/// The column of a picked study that no longer exists (`Ok(None)` — deleted meanwhile): every
/// row « introuvable ». An absence, never worded as a read failure (misattribution is a lie).
pub fn missing_column(label: &str) -> ComparisonColumn {
    ComparisonColumn {
        missing: true,
        ..unavailable_column(label)
    }
}

/// The column of a study that reads but whose frame does not compute (`build_frame` refused its
/// inputs): the study's own header facts, every row « non calculable » — neither a read failure
/// nor an absence; the study exists and can be opened to see why.
pub fn uncomputable_column(study: &Study) -> ComparisonColumn {
    ComparisonColumn {
        ticker: study.security_ticker.to_uppercase(),
        name: study.company_name.clone().unwrap_or_default(),
        currency: study.native_currency.to_uppercase(),
        date: study.created_at.0.chars().take(10).collect(),
        unavailable: true,
        uncomputable: true,
        rows: vec![String::new(); 30],
        ..ComparisonColumn::default()
    }
}

/// The listing venue of a canonical ticker (`NESN.SW` → `SW`) — only a KNOWN venue, so a
/// share-class suffix (`BRK.B`) is never passed off as an exchange; `""` otherwise (« — »).
fn exchange_of(ticker: &str) -> String {
    steadyinvest_ingestion::ticker::known_venue(ticker).unwrap_or_default()
}

/// The latest PROVIDER timestamp among the study's filled cells (the form's « date of source
/// material »), `YYYY-MM-DD`; `None` when no provider figure is held — then row 29 is « — »,
/// never the creation date passed off as the data's date (G1, #237).
fn latest_provider_date(study: &Study) -> Option<String> {
    study
        .years
        .iter()
        .flat_map(|y| {
            let mut cells = vec![&y.sales, &y.eps, &y.high_price, &y.low_price];
            cells.extend(y.dividend_per_share.iter());
            cells.extend(y.pre_tax_profit.iter());
            cells.extend(y.book_value_per_share.iter());
            cells
        })
        .filter(|c| c.value.is_some() && c.provenance.source == Source::Provider)
        .map(|c| c.provenance.timestamp.0.clone())
        .max()
        .map(|ts| ts.chars().take(10).collect())
}

/// Rows 9 / 11 / 15 are labelled « sur 5 ans »: a shorter window (a low-confidence study) is
/// not five years, so its extremes are absent rather than passed off as the five-year ones.
fn full_window(len: usize) -> bool {
    len >= USABLE_YEARS_FLOOR as usize
}

/// Row 9: the lowest low / highest high over the §3 window's years — `(None, None)` when ANY
/// window year lacks the price or the window is shorter than five years (absent, never a
/// partial extreme passed off as the 5-year one).
fn window_price_range(
    window: &[i32],
    series: &[CanonicalYear],
) -> (Option<Decimal>, Option<Decimal>) {
    if !full_window(window.len()) {
        return (None, None);
    }
    let year = |w: &i32| series.iter().find(|y| y.year == *w);
    let lows: Option<Vec<Decimal>> = window.iter().map(|w| year(w)?.low_price).collect();
    let highs: Option<Vec<Decimal>> = window.iter().map(|w| year(w)?.high_price).collect();
    (
        lows.and_then(|v| v.into_iter().min()),
        highs.and_then(|v| v.into_iter().max()),
    )
}

/// Rows 11 / 15: the highest high P/E and the lowest low P/E over the window's years — each
/// `None` when any window year's ratio is unknown or the window is short (same rule as row 9).
fn window_pe_extremes(per_year: &[YearValuation]) -> (Option<Decimal>, Option<Decimal>) {
    if !full_window(per_year.len()) {
        return (None, None);
    }
    let highs: Option<Vec<Decimal>> = per_year.iter().map(|y| y.high_pe).collect();
    let lows: Option<Vec<Decimal>> = per_year.iter().map(|y| y.low_pe).collect();
    (
        highs.and_then(|v| v.into_iter().max()),
        lows.and_then(|v| v.into_iter().min()),
    )
}

/// Row 27: « N : flag · flag » when flags are raised; « 0 » only when every rule was checked
/// (`core::ssg::quality_flags_assessable`, next to the rules themselves); not assessable →
/// `""` (« — »), never a zero standing for an absence.
fn flags_row(outputs: &SsgOutputs, judgment: &JudgmentInputs) -> String {
    if outputs.quality_flags.is_empty() {
        if quality_flags_assessable(outputs, judgment) {
            "0".to_string()
        } else {
            String::new()
        }
    } else {
        format!(
            "{} : {}",
            outputs.quality_flags.len(),
            outputs
                .quality_flags
                .iter()
                .map(|k| state::quality_flag_label(*k))
                .collect::<Vec<_>>()
                .join(" · ")
        )
    }
}

/// `lo – hi` when both are known, else `""`.
fn range(
    lo: Option<Decimal>,
    hi: Option<Decimal>,
    field: DisplayField,
    format: NumberFormat,
) -> String {
    match (lo, hi) {
        (Some(l), Some(h)) => format!(
            "{} – {}",
            format_scaled(l, field, format),
            format_scaled(h, field, format)
        ),
        _ => String::new(),
    }
}

/// Rows 5 / 6 when the columns average over different spans (G1, #237): the cell names its own
/// years — « 47,6 % sur 3 ans · ↑ hausse ». An empty cell stays empty; `years` 0 leaves it as is.
pub fn with_avg_years(cell: &str, years: usize) -> String {
    if cell.is_empty() || years == 0 {
        return cell.to_string();
    }
    let span = if years == 1 {
        crate::viewmodel::engine::AVG_OVER_ONE_YEAR.to_string()
    } else {
        crate::viewmodel::engine::AVG_OVER_YEARS.replace("{n}", &years.to_string())
    };
    match cell.split_once(" · ") {
        Some((avg, trend)) => format!("{avg} {span} · {trend}"),
        None => format!("{cell} {span}"),
    }
}

/// One study → its thirty rows, from the frame the study screen shows.
pub fn comparison_column(
    study: &Study,
    frame: &crate::viewmodel::engine::StudyFrame,
    format: NumberFormat,
) -> ComparisonColumn {
    let outputs = frame.snapshot.outputs();
    let j = &study.judgment;
    let g = &outputs.growth;
    let m = &outputs.management;
    let v = &outputs.valuation;
    let r = &outputs.risk_reward;
    let ret = &outputs.returns;
    let money = |d: Option<steadyinvest_contract::Money>| d.map(|x| x.as_decimal());
    let current = money(j.current_price);
    // The §3 window's years: the valuation's per_year rows; the price range spans those years.
    let window: Vec<i32> = v.per_year.iter().map(|y| y.year).collect();
    let (range_lo, range_hi) = window_price_range(&window, &frame.series);
    let (pe_max, pe_min) = window_pe_extremes(&v.per_year);
    let eps_total_5y = ret
        .avg_annual_eps
        .and_then(|a| a.checked_mul(Decimal::from(5)));
    let with_trend = |avg: Option<Decimal>, trend: Option<steadyinvest_core::ssg::Trend>| match avg
    {
        Some(a) => format!(
            "{} % · {}",
            format_scaled(a, DisplayField::Percent, format),
            fmt_trend(trend)
        ),
        None => String::new(),
    };
    let zone =
        |lo: Option<Decimal>, hi: Option<Decimal>| range(lo, hi, DisplayField::Price, format);
    let (z_low, z_buy_top, z_neutral_top, z_high) = match &r.zones {
        Some(z) => (
            Some(z.forecast_low),
            Some(z.buy_top),
            Some(z.neutral_top),
            Some(z.forecast_high),
        ),
        None => (None, None, None, None),
    };
    let flags = flags_row(outputs, &steadyinvest_report::form::to_judgment_inputs(j));
    let empty_if_dash = |s: String| if s == EMPTY_SLOT { String::new() } else { s };
    let rows = vec![
        empty_if_dash(fmt_pct(g.sales_cagr_pct, format)), // 1
        empty_if_dash(fmt_pct(money(j.projected_sales_growth_pct), format)), // 2
        empty_if_dash(fmt_pct(g.eps_cagr_pct, format)),   // 3
        empty_if_dash(fmt_pct(money(j.projected_eps_growth_pct), format)), // 4
        with_trend(m.avg_ptp_pct, m.ptp_trend),           // 5
        with_trend(m.avg_roe_pct, m.roe_trend),           // 6
        String::new(),                                    // 7 not carried
        empty_if_dash(fmt(eps_total_5y, DisplayField::PerShare, format)), // 8
        range(range_lo, range_hi, DisplayField::Price, format), // 9
        empty_if_dash(fmt(current, DisplayField::Price, format)), // 10
        empty_if_dash(fmt(pe_max, DisplayField::PeRatio, format)), // 11
        empty_if_dash(fmt(v.avg_high_pe, DisplayField::PeRatio, format)), // 12
        empty_if_dash(fmt(v.avg_pe, DisplayField::PeRatio, format)), // 13
        empty_if_dash(fmt(v.avg_low_pe, DisplayField::PeRatio, format)), // 14
        empty_if_dash(fmt(pe_min, DisplayField::PeRatio, format)), // 15
        empty_if_dash(fmt(v.current_pe, DisplayField::PeRatio, format)), // 16
        zone(z_low, z_buy_top),                           // 17
        zone(z_buy_top, z_neutral_top),                   // 18
        zone(z_neutral_top, z_high),                      // 19
        String::new(),                                    // 20 (key)
        empty_if_dash(fmt_ud(&r.upside_downside, format)), // 21
        empty_if_dash(fmt_pct(ret.present_yield_pct, format)), // 22
        empty_if_dash(fmt_total_return(ret, format)),     // 23
        String::new(),                                    // 24 not carried
        String::new(),                                    // 25 not carried
        empty_if_dash(fmt_pct(v.avg_payout_pct, format)), // 26
        flags,                                            // 27
        String::new(),                                    // 28 (key)
        latest_provider_date(study).unwrap_or_default(),  // 29
        exchange_of(&study.security_ticker),              // 30
    ];
    ComparisonColumn {
        ticker: study.security_ticker.to_uppercase(),
        name: study.company_name.clone().unwrap_or_default(),
        currency: study.native_currency.to_uppercase(),
        date: study.created_at.0.chars().take(10).collect(),
        rows,
        zone: zone_position_key(r, current).to_string(),
        state: verdict_state(frame.snapshot.verdict()).to_string(),
        low_confidence: outputs.low_confidence,
        // The years each shown average runs over (0 when no average is shown).
        ptp_avg_years: m.avg_ptp_pct.map_or(0, |_| m.ptp_avg_years),
        roe_avg_years: m.avg_roe_pct.map_or(0, |_| m.roe_avg_years),
        ..ComparisonColumn::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exchange_suffix_and_ranges_are_data_only() {
        assert_eq!(exchange_of("NESN.SW"), "SW");
        assert_eq!(exchange_of("BRK"), "");
        // A share class is not an exchange (G1, #237).
        assert_eq!(exchange_of("BRK.B"), "");
        assert_eq!(exchange_of("BRK.B.US"), "US");
        let d = |s: &str| Decimal::from_str_exact(s).unwrap();
        assert_eq!(
            range(
                Some(d("50")),
                Some(d("100")),
                DisplayField::Price,
                NumberFormat::Comma
            ),
            "50 – 100"
        );
        assert_eq!(
            range(
                None,
                Some(d("100")),
                DisplayField::Price,
                NumberFormat::Comma
            ),
            ""
        );
    }

    #[test]
    fn an_unavailable_column_has_thirty_empty_rows_and_the_flag() {
        let c = unavailable_column("NESN.SW · 2026-01-01");
        assert!(c.unavailable);
        assert!(!c.missing && !c.uncomputable);
        assert_eq!(
            c.ticker, "NESN.SW · 2026-01-01",
            "the pick's label, as shown"
        );
        assert_eq!(c.rows.len(), 30);
        // A study gone meanwhile is its own state, not a read failure.
        let m = missing_column("NESN.SW");
        assert!(m.missing);
        assert_eq!(m.ticker, "NESN.SW");
        assert_eq!(m.rows.len(), 30);
        // A study that reads but does not compute keeps its facts, and its own state.
        let study = demo_study().unwrap();
        let u = uncomputable_column(&study);
        assert!(u.uncomputable && u.unavailable && !u.missing);
        assert_eq!(u.ticker, "DÉMO");
        assert_eq!(u.date, "2026-01-01");
        assert_eq!(u.currency, study.native_currency.to_uppercase());
        assert_eq!(u.rows.len(), 30);
    }

    use crate::viewmodel::engine::{
        build_frame, growth_computed, mgmt_computed, pe_computed, pe_year_cells, return_computed,
        risk_computed,
    };
    use crate::viewmodel::verify::demo_study;

    const F: NumberFormat = NumberFormat::Comma;

    /// The study screen's em-dash is the comparison's `""` (worded « — » by the screen/report).
    fn dash_free(s: impl Into<String>) -> String {
        let s = s.into();
        if s == EMPTY_SLOT { String::new() } else { s }
    }

    #[test]
    fn every_straight_read_row_equals_the_study_screens_figure() {
        // Spec §5 / AC1: one read path — the comparison cell IS the study screen's string.
        let study = demo_study().unwrap();
        let frame = build_frame(&study).unwrap();
        let o = frame.snapshot.outputs();
        let col = comparison_column(&study, &frame, F);
        assert_eq!(col.rows.len(), 30);
        let growth = growth_computed(o, F);
        assert_eq!(col.rows[0], dash_free(growth.sales_cagr.to_string()));
        assert_eq!(col.rows[2], dash_free(growth.eps_cagr.to_string()));
        let years: Vec<i32> = study.years.iter().map(|y| y.year).collect();
        let mgmt = mgmt_computed(o, &years, F);
        {
            assert!(o.management.avg_ptp_pct.is_some() && o.management.avg_roe_pct.is_some());
            assert_eq!(
                col.rows[4],
                format!("{} · {}", mgmt.avg_ptp, mgmt.ptp_trend)
            );
        }
        {
            assert_eq!(
                col.rows[5],
                format!("{} · {}", mgmt.avg_roe, mgmt.roe_trend)
            );
        }
        let pe = pe_computed(o, F);
        assert_eq!(col.rows[11], dash_free(pe.avg_high_pe.to_string()));
        assert_eq!(col.rows[12], dash_free(pe.avg_pe.to_string()));
        assert_eq!(col.rows[13], dash_free(pe.avg_low_pe.to_string()));
        assert_eq!(col.rows[15], dash_free(pe.current_pe.to_string()));
        let risk = risk_computed(o, &study.judgment, F);
        assert_eq!(col.rows[20], dash_free(risk.ud_ratio.to_string()));
        let ret = return_computed(o, F);
        assert_eq!(col.rows[21], dash_free(ret.present_yield.to_string()));
        assert_eq!(col.rows[22], dash_free(ret.total_return.to_string()));
        // The demo is a complete worked example: the figures are there, not blanks.
        for i in [0, 2, 11, 12, 13, 20] {
            assert!(!col.rows[i].is_empty(), "row {} is filled", i + 1);
        }
        // Rows 7, 24, 25 are not carried; 20 and 28 are keys.
        for i in [6, 19, 23, 24, 27] {
            assert!(col.rows[i].is_empty(), "row {} carries no string", i + 1);
        }
    }

    #[test]
    fn the_derived_rows_are_the_window_extremes_of_the_study_screens_cells() {
        let study = demo_study().unwrap();
        let frame = build_frame(&study).unwrap();
        let o = frame.snapshot.outputs();
        let v = &o.valuation;
        let col = comparison_column(&study, &frame, F);
        // Row 8 = the average annual EPS × 5.
        let eps5 = o.returns.avg_annual_eps.unwrap() * Decimal::from(5);
        assert_eq!(col.rows[7], fmt(Some(eps5), DisplayField::PerShare, F));
        // Rows 11 / 15: the highest D and the lowest E cell the study screen shows for the
        // window's years (§3 per-year grid).
        let hi = v.per_year.iter().map(|y| y.high_pe.unwrap()).max().unwrap();
        let lo = v.per_year.iter().map(|y| y.low_pe.unwrap()).min().unwrap();
        assert_eq!(col.rows[10], fmt(Some(hi), DisplayField::PeRatio, F));
        assert_eq!(col.rows[14], fmt(Some(lo), DisplayField::PeRatio, F));
        let d_cells: Vec<String> = v
            .per_year
            .iter()
            .map(|y| pe_year_cells(o, y.year, F)[0].clone())
            .collect();
        let e_cells: Vec<String> = v
            .per_year
            .iter()
            .map(|y| pe_year_cells(o, y.year, F)[1].clone())
            .collect();
        assert!(d_cells.contains(&col.rows[10]));
        assert!(e_cells.contains(&col.rows[14]));
        // Row 9: lowest low – highest high over the same years of the canonical series.
        let window: Vec<i32> = v.per_year.iter().map(|y| y.year).collect();
        let in_w: Vec<&CanonicalYear> = frame
            .series
            .iter()
            .filter(|y| window.contains(&y.year))
            .collect();
        assert_eq!(in_w.len(), window.len());
        let plo = in_w.iter().map(|y| y.low_price.unwrap()).min().unwrap();
        let phi = in_w.iter().map(|y| y.high_price.unwrap()).max().unwrap();
        assert_eq!(
            col.rows[8],
            range(Some(plo), Some(phi), DisplayField::Price, F)
        );
    }

    #[test]
    fn a_window_year_without_the_figure_makes_the_extreme_absent_never_partial() {
        let study = demo_study().unwrap();
        let frame = build_frame(&study).unwrap();
        let v = &frame.snapshot.outputs().valuation;
        let window: Vec<i32> = v.per_year.iter().map(|y| y.year).collect();
        assert!(window.len() >= 2);
        // One window year loses its high price: the high side is absent, the low side stands.
        let mut series = frame.series.clone();
        let gap = series.iter_mut().find(|y| y.year == window[0]).unwrap();
        gap.high_price = None;
        let (lo, hi) = window_price_range(&window, &series);
        assert!(lo.is_some());
        assert_eq!(hi, None);
        // A window year missing from the series altogether: both sides absent.
        let short: Vec<CanonicalYear> = frame
            .series
            .iter()
            .filter(|y| y.year != window[1])
            .cloned()
            .collect();
        assert_eq!(window_price_range(&window, &short), (None, None));
        // Rows 11 / 15: one unknown ratio → that extreme is absent.
        let mut per_year = v.per_year.clone();
        per_year[0].high_pe = None;
        let (max, min) = window_pe_extremes(&per_year);
        assert_eq!(max, None);
        assert!(min.is_some());
        per_year[1].low_pe = None;
        assert_eq!(window_pe_extremes(&per_year), (None, None));
        // An empty window: absent, never a fabricated extreme.
        assert_eq!(window_pe_extremes(&[]), (None, None));
        assert_eq!(window_price_range(&[], &frame.series), (None, None));
    }

    #[test]
    fn row_27_states_zero_only_when_every_rule_was_checked() {
        use steadyinvest_core::ssg::{QualityFlagKey, quality_flag_input_known};
        use steadyinvest_report::form::to_judgment_inputs;
        let mut study = demo_study().unwrap();
        // The worked example carries no TTM EPS → no current P/E → no relative value: one rule
        // unchecked. Give it one so every rule's input is known.
        let frame = build_frame(&study).unwrap();
        let judged = to_judgment_inputs(&study.judgment);
        assert!(!quality_flags_assessable(frame.snapshot.outputs(), &judged));
        study.judgment.ttm_eps = Some(steadyinvest_contract::Money::from(Decimal::from(5)));
        let frame = build_frame(&study).unwrap();
        let mut o = frame.snapshot.outputs().clone();
        assert!(quality_flags_assessable(&o, &judged), "every rule checked");
        // Every flag the engine raised had its input known (the predicate mirrors the rules).
        for k in &o.quality_flags {
            assert!(quality_flag_input_known(*k, &o, &judged), "{k:?}");
        }
        assert_eq!(
            QualityFlagKey::ALL.len(),
            9,
            "a new rule: state its input in core"
        );
        o.quality_flags.clear();
        assert_eq!(flags_row(&o, &judged), "0");
        // Nothing to judge the future high P/E against: not assessable → « — », never « 0 ».
        let mut unjudged = judged.clone();
        unjudged.judged_avg_high_pe = None;
        assert_eq!(flags_row(&o, &unjudged), "");
        // An unknown trend input, same.
        o.management.ptp_trend = None;
        assert_eq!(flags_row(&o, &judged), "");
        // A raised flag is a fact whatever else is unknown.
        o.quality_flags.push(QualityFlagKey::RoeLow);
        assert!(flags_row(&o, &unjudged).starts_with("1 : "));
    }

    #[test]
    fn a_window_shorter_than_five_years_is_not_the_five_year_extreme() {
        let study = demo_study().unwrap();
        let frame = build_frame(&study).unwrap();
        let v = &frame.snapshot.outputs().valuation;
        assert_eq!(v.per_year.len(), 5, "the worked example has a full window");
        let window: Vec<i32> = v.per_year.iter().map(|y| y.year).collect();
        let (lo, hi) = window_price_range(&window, &frame.series);
        assert!(lo.is_some() && hi.is_some());
        // Four years (a low-confidence study): rows 9 / 11 / 15 read « — ».
        assert_eq!(
            window_price_range(&window[1..], &frame.series),
            (None, None)
        );
        assert_eq!(window_pe_extremes(&v.per_year[1..]), (None, None));
    }

    #[test]
    fn rows_5_and_6_carry_the_years_actually_averaged() {
        let study = demo_study().unwrap();
        let frame = build_frame(&study).unwrap();
        let col = comparison_column(&study, &frame, F);
        let m = &frame.snapshot.outputs().management;
        assert_eq!(col.ptp_avg_years, m.ptp_avg_years);
        assert_eq!(col.roe_avg_years, m.roe_avg_years);
        assert!(col.ptp_avg_years > 0);
        // A cell names its own span only when asked (the columns differ).
        assert_eq!(
            with_avg_years("47,6 % · ↑ hausse", 3),
            "47,6 % sur 3 ans · ↑ hausse"
        );
        assert_eq!(with_avg_years("47,6 % · —", 1), "47,6 % sur 1 an · —");
        assert_eq!(with_avg_years("", 3), "", "an absent average stays absent");
        assert_eq!(with_avg_years("47,6 % · ↑ hausse", 0), "47,6 % · ↑ hausse");
    }

    #[test]
    fn row_29_is_the_provider_date_never_the_creation_date() {
        let mut study = demo_study().unwrap();
        let frame = build_frame(&study).unwrap();
        let col = comparison_column(&study, &frame, F);
        assert_eq!(col.rows[28], "2026-01-01");
        // Only a FILLED provider cell dates the data: an empty one stamped later does not.
        let later = steadyinvest_contract::Timestamp("2027-03-01T00:00:00Z".into());
        let mut probe = study.clone();
        let cell = &mut probe.years[0].sales;
        cell.provenance.timestamp = later.clone();
        cell.value = None;
        assert_eq!(latest_provider_date(&probe).as_deref(), Some("2026-01-01"));
        probe.years[0].sales.value = study.years[0].sales.value;
        assert_eq!(latest_provider_date(&probe).as_deref(), Some("2027-03-01"));
        // A study with no provider figure: « — », not the creation date passed off as data's.
        for y in &mut study.years {
            for c in [
                &mut y.sales,
                &mut y.eps,
                &mut y.high_price,
                &mut y.low_price,
            ] {
                c.provenance.source = Source::Manual;
            }
            for c in [
                &mut y.dividend_per_share,
                &mut y.pre_tax_profit,
                &mut y.book_value_per_share,
            ]
            .into_iter()
            .flatten()
            {
                c.provenance.source = Source::Manual;
            }
        }
        let frame = build_frame(&study).unwrap();
        let col = comparison_column(&study, &frame, F);
        assert_eq!(col.rows[28], "");
        assert_eq!(
            col.date, "2026-01-01",
            "the decision date stays in the header"
        );
    }
}
