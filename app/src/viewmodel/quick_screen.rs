//! Story 7.3 — the examination's float → string boundary: `core::checklist` outputs → the
//! formatted lines the screen and the PDF show (one path, no drift), plus the four conclusions
//! against the reader's objective (pure, tested).

use rust_decimal::Decimal;
use steadyinvest_core::checklist::{Ladder, PePosition, QuickScreenOutputs, RateComparison};
use steadyinvest_core::rounding::DisplayField;
use steadyinvest_report::{QuickScreen, QuickScreenLadder, QuickScreenPriceRow};

use crate::viewmodel::format::{NumberFormat, format_scaled, parse_amount};

fn f(v: Option<Decimal>, field: DisplayField, format: NumberFormat) -> String {
    v.map(|d| format_scaled(d, field, format))
        .unwrap_or_default()
}

fn pct(v: Option<Decimal>, format: NumberFormat) -> String {
    v.map(|d| format!("{} %", format_scaled(d, DisplayField::Percent, format)))
        .unwrap_or_default()
}

fn year(y: Option<i32>) -> String {
    y.map(|y| y.to_string()).unwrap_or_default()
}

fn ladder(l: &Ladder, field: DisplayField, format: NumberFormat) -> QuickScreenLadder {
    if l.unavailable {
        return QuickScreenLadder {
            unavailable: true,
            ..QuickScreenLadder::default()
        };
    }
    QuickScreenLadder {
        lines: vec![
            f(l.recent, field, format),
            f(l.recent_prior, field, format),
            f(l.recent_total, field, format),
            f(l.recent_avg, field, format),
            f(l.old, field, format),
            f(l.old_prior, field, format),
            f(l.old_total, field, format),
            f(l.old_avg, field, format),
            f(l.increase, field, format),
            pct(l.increase_pct, format),
        ],
        years: vec![
            year(l.recent_year),
            year(l.recent_prior_year),
            year(l.old_year),
            year(l.old_prior_year),
        ],
        rate: pct(l.compound_rate_pct, format),
        span_years: l.span_years.to_string(),
        unavailable: false,
    }
}

/// Header facts the view carries besides the figures.
pub struct QuickScreenHeader {
    pub ticker: String,
    pub name: String,
    pub currency: String,
    pub date: String,
    pub source: String,
}

/// The formatted examination (the reader's fields left empty — the screen owns them).
pub fn quick_screen_view(
    head: QuickScreenHeader,
    out: &QuickScreenOutputs,
    format: NumberFormat,
) -> QuickScreen {
    let p = &out.price;
    QuickScreen {
        ticker: head.ticker.to_uppercase(),
        name: head.name,
        currency: head.currency.to_uppercase(),
        date: head.date,
        source: head.source,
        sales: ladder(&out.sales, DisplayField::LargeMonetary, format),
        eps: ladder(&out.eps, DisplayField::PerShare, format),
        eps_vs_sales: match out.eps_vs_sales {
            Some(RateComparison::EpsFaster) => "eps",
            Some(RateComparison::SalesFaster) => "sales",
            Some(RateComparison::Same) => "same",
            None => "",
        }
        .into(),
        price_rows: p
            .rows
            .iter()
            .map(|r| QuickScreenPriceRow {
                year: r.year.to_string(),
                high: f(r.high, DisplayField::Price, format),
                low: f(r.low, DisplayField::Price, format),
                eps: f(r.eps, DisplayField::PerShare, format),
                pe_high: f(r.pe_high, DisplayField::PeRatio, format),
                pe_low: f(r.pe_low, DisplayField::PeRatio, format),
            })
            .collect(),
        pe_high_total: f(p.pe_high_total, DisplayField::PeRatio, format),
        pe_low_total: f(p.pe_low_total, DisplayField::PeRatio, format),
        pe_high_avg: f(p.pe_high_avg, DisplayField::PeRatio, format),
        pe_low_avg: f(p.pe_low_avg, DisplayField::PeRatio, format),
        pe_avg_of_avgs: f(p.pe_avg_of_avgs, DisplayField::PeRatio, format),
        present_price: f(p.present_price, DisplayField::Price, format),
        present_eps: f(p.present_eps, DisplayField::PerShare, format),
        present_pe: f(p.present_pe, DisplayField::PeRatio, format),
        high_five_years_ago: f(p.high_five_years_ago, DisplayField::Price, format),
        price_vs_high_pct: pct(p.price_vs_high_pct, format),
        years_sold_as_high: p
            .years_sold_as_high
            .map(|n| n.to_string())
            .unwrap_or_default(),
        pe_position: match p.pe_position {
            Some(PePosition::Higher) => "higher",
            Some(PePosition::Similar) => "similar",
            Some(PePosition::Lower) => "lower",
            None => "",
        }
        .into(),
        ..QuickScreen::default()
    }
}

/// « atteint » / « n'atteint pas » as a key (`yes` / `no`), `""` without an objective or a rate.
/// The objective is the reader's text (« 7 », « 7 % », « 7,5 »), parsed under the locale.
pub fn meets_key(rate_pct: Option<Decimal>, objective: &str, format: NumberFormat) -> String {
    let cleaned = objective.trim().trim_end_matches('%').trim();
    let Some(target) = parse_amount(cleaned, format).map(|m| m.as_decimal()) else {
        return String::new();
    };
    match rate_pct {
        Some(r) if r >= target => "yes".into(),
        Some(_) => "no".into(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    #[test]
    fn the_objective_is_the_readers_and_reads_under_the_locale() {
        assert_eq!(meets_key(Some(d("8.1")), "7 %", NumberFormat::Comma), "yes");
        assert_eq!(meets_key(Some(d("5.2")), "7", NumberFormat::Comma), "no");
        assert_eq!(meets_key(Some(d("7.5")), "7,5", NumberFormat::Comma), "yes");
        assert_eq!(meets_key(Some(d("8.1")), "", NumberFormat::Comma), "");
        assert_eq!(meets_key(None, "7", NumberFormat::Comma), "");
        assert_eq!(meets_key(Some(d("8.1")), "sept", NumberFormat::Comma), "");
    }

    #[test]
    fn an_unavailable_ladder_formats_as_such_and_keys_cross_as_keys() {
        let mut out = QuickScreenOutputs::default();
        out.sales.unavailable = true;
        out.eps_vs_sales = Some(RateComparison::Same);
        out.price.pe_position = Some(PePosition::Lower);
        let v = quick_screen_view(
            QuickScreenHeader {
                ticker: "nesn.sw".into(),
                name: String::new(),
                currency: "chf".into(),
                date: "2026-09-24".into(),
                source: String::new(),
            },
            &out,
            NumberFormat::Comma,
        );
        assert!(v.sales.unavailable);
        assert_eq!(v.ticker, "NESN.SW");
        assert_eq!(v.currency, "CHF");
        assert_eq!(v.eps_vs_sales, "same");
        assert_eq!(v.pe_position, "lower");
        assert_eq!(v.present_pe, "");
    }
}
