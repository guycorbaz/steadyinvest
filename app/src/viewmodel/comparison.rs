//! Story 7.1 — one study → one comparison column (the form's thirty rows), formatted at this
//! boundary from the SAME frame the study screen shows (`build_frame`: no drift). Four rows are
//! min / max / × over engine outputs (rows 8, 9, 11, 15) — pure, tested here; the rest is a
//! straight read. The two worded rows (20 zone, 28 state) cross as keys; the report and the
//! screen word them in their own inventories.

use rust_decimal::Decimal;
use steadyinvest_contract::{Source, Study};
use steadyinvest_core::rounding::DisplayField;
use steadyinvest_report::ComparisonColumn;

use crate::state;
use crate::viewmodel::engine::{
    fmt, fmt_pct, fmt_total_return, fmt_trend, fmt_ud, verdict_state, zone_position_key,
};
use crate::viewmodel::form::EMPTY_SLOT;
use crate::viewmodel::format::{NumberFormat, format_scaled};

/// The column of a study that could not be read: every row « indisponible » (the report's word).
pub fn unavailable_column(ticker: &str) -> ComparisonColumn {
    ComparisonColumn {
        ticker: ticker.to_uppercase(),
        unavailable: true,
        rows: vec![String::new(); 30],
        ..ComparisonColumn::default()
    }
}

/// The exchange suffix of a provider ticker (`NESN.SW` → `SW`), as data; `""` without one.
fn exchange_of(ticker: &str) -> String {
    ticker
        .rsplit_once('.')
        .map(|(_, suffix)| suffix.to_uppercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

/// The latest PROVIDER timestamp among the study's cells (the form's « date of source
/// material »), `YYYY-MM-DD`; `None` for a manual study.
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
        .filter(|c| c.provenance.source == Source::Provider)
        .map(|c| c.provenance.timestamp.0.clone())
        .max()
        .map(|ts| ts.chars().take(10).collect())
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
    let in_window = |y: &steadyinvest_core::normalize::CanonicalYear| window.contains(&y.year);
    let range_hi = frame
        .series
        .iter()
        .filter(|y| in_window(y))
        .filter_map(|y| y.high_price)
        .max();
    let range_lo = frame
        .series
        .iter()
        .filter(|y| in_window(y))
        .filter_map(|y| y.low_price)
        .min();
    let pe_max = v.per_year.iter().filter_map(|y| y.high_pe).max();
    let pe_min = v.per_year.iter().filter_map(|y| y.low_pe).min();
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
    let flags = if outputs.quality_flags.is_empty() {
        "0".to_string()
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
    };
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
        latest_provider_date(study)
            .unwrap_or_else(|| study.created_at.0.chars().take(10).collect()), // 29
        exchange_of(&study.security_ticker),              // 30
    ];
    ComparisonColumn {
        ticker: study.security_ticker.to_uppercase(),
        name: study.company_name.clone().unwrap_or_default(),
        currency: study.native_currency.to_uppercase(),
        date: study.created_at.0.chars().take(10).collect(),
        unavailable: false,
        rows,
        zone: zone_position_key(r, current).to_string(),
        state: verdict_state(frame.snapshot.verdict()).to_string(),
        low_confidence: outputs.low_confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exchange_suffix_and_ranges_are_data_only() {
        assert_eq!(exchange_of("NESN.SW"), "SW");
        assert_eq!(exchange_of("BRK"), "");
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
        let c = unavailable_column("nesn.sw");
        assert!(c.unavailable);
        assert_eq!(c.ticker, "NESN.SW");
        assert_eq!(c.rows.len(), 30);
    }
}
