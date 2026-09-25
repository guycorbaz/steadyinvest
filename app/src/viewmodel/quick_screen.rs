//! Story 7.3 — the examination's float → string boundary: `core::checklist` outputs → the
//! formatted lines the screen and the PDF show (one path, no drift), plus the four conclusions
//! against the reader's objective (pure, tested).

use rust_decimal::Decimal;
use steadyinvest_core::checklist::{
    Ladder, PePosition, PriceRecord, QuickScreenOutputs, RateComparison,
};
use steadyinvest_core::rounding::{DisplayField, round_for_display};
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
        // Line (10) is absent on a non-positive old average: the layouts say why (G1 D review).
        nonpositive_base: l.old_avg.is_some_and(|o| o <= Decimal::ZERO),
        unavailable: false,
    }
}

/// The §3 « bases »: which wording each fact may carry. « cinq ans » only over the form's full
/// five-year record, a stated count otherwise, `""` when the figure is absent (absence honesty,
/// G1 review — never « cinq ans » over fewer rows, never « 0 » for no row).
struct PriceBases {
    pe_basis: &'static str,
    high_basis: &'static str,
    sold_basis: &'static str,
    price_vs_high: &'static str,
    pe_absent: &'static str,
}

fn price_bases(p: &PriceRecord) -> PriceBases {
    let rows = p.rows.len() as u32;
    let pe_basis = match p.pe_years {
        0 => "",
        5 if p.five_year_record => "five",
        n if n == rows => "all",
        _ => "partial",
    };
    let high_basis = match p.high_year {
        None => "",
        Some(_) if p.five_year_record => "five",
        Some(_) => "year",
    };
    let sold_basis = match p.years_sold_as_high {
        None => "",
        Some(_) if p.five_year_record && p.high_years == 5 => "five",
        Some(_) => "count",
    };
    // The word is decided on the DISPLAYED percentage: « −0,0 % » is « au même niveau ».
    let price_vs_high = match p
        .price_vs_high_pct
        .map(|v| round_for_display(v, DisplayField::Percent))
    {
        Some(v) if v > Decimal::ZERO => "higher",
        Some(v) if v < Decimal::ZERO => "lower",
        Some(_) => "same",
        None => "",
    };
    let pe_absent = match (p.pe_position, p.present_pe, p.pe_avg_of_avgs) {
        (Some(_), _, _) => "",
        (None, None, None) => "both",
        (None, None, Some(_)) => "pe",
        // A present P/E without a position: the record's average is absent (core yields no P/E on
        // a non-positive price or EPS, so a present average is always positive — G1 D review).
        (None, Some(_), _) => "average",
    };
    PriceBases {
        pe_basis,
        high_basis,
        sold_basis,
        price_vs_high,
        pe_absent,
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
    let bases = price_bases(p);
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
        pe_basis: bases.pe_basis.into(),
        pe_years: p.pe_years.to_string(),
        record_years: p.rows.len().to_string(),
        present_price: f(p.present_price, DisplayField::Price, format),
        present_eps: f(p.present_eps, DisplayField::PerShare, format),
        present_pe: f(p.present_pe, DisplayField::PeRatio, format),
        high_five_years_ago: f(p.high_five_years_ago, DisplayField::Price, format),
        high_basis: bases.high_basis.into(),
        high_year: year(p.high_year),
        price_vs_high_pct: pct(p.price_vs_high_pct, format),
        price_vs_high: bases.price_vs_high.into(),
        years_sold_as_high: p
            .years_sold_as_high
            .map(|n| n.to_string())
            .unwrap_or_default(),
        sold_basis: bases.sold_basis.into(),
        sold_of: p.high_years.to_string(),
        pe_absent: bases.pe_absent.into(),
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

/// Conclusions 1 / 2 as a key: `yes` / `no` (« atteint » / « n'atteint pas »), `""` when the
/// objective is blank, `unread` when it is not a number, `no-rate` when the rate is absent — each
/// its own wording, so « objectif non renseigné » is said only of a blank objective (G1 review).
/// The objective is the reader's text (« 7 », « 7 % », « 7,5 »), parsed under the locale; the
/// rate is compared as DISPLAYED (rounded), so « 7,0 % » never « n'atteint pas » an objective of 7.
pub fn meets_key(rate_pct: Option<Decimal>, objective: &str, format: NumberFormat) -> String {
    let cleaned = objective.trim().trim_end_matches('%').trim();
    if cleaned.is_empty() {
        return String::new();
    }
    // A growth objective carries no grouping: under the Point preset « 7,5 » would otherwise lose
    // its comma to the thousands rule and read 75 — unread, never a silently different target
    // (G1 D review). Inner spaces are grouping too.
    if cleaned.contains(format.thousands_separator()) || cleaned.contains(char::is_whitespace) {
        return "unread".into();
    }
    let Some(target) = parse_amount(cleaned, format).map(|m| m.as_decimal()) else {
        return "unread".into();
    };
    match rate_pct.map(|r| round_for_display(r, DisplayField::Percent)) {
        Some(r) if r >= target => "yes".into(),
        Some(_) => "no".into(),
        None => "no-rate".into(),
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
        assert_eq!(meets_key(Some(d("8.1")), " % ", NumberFormat::Comma), "");
        assert_eq!(meets_key(None, "", NumberFormat::Comma), "");
        // The rate absent is not the objective absent (G1 review).
        assert_eq!(meets_key(None, "7", NumberFormat::Comma), "no-rate");
        assert_eq!(
            meets_key(Some(d("8.1")), "sept", NumberFormat::Comma),
            "unread"
        );
    }

    #[test]
    fn a_grouped_objective_is_unread_never_a_different_number() {
        assert_eq!(
            meets_key(Some(d("8")), "7,5", NumberFormat::Point),
            "unread"
        );
        assert_eq!(meets_key(Some(d("8")), "7.5", NumberFormat::Point), "yes");
        assert_eq!(
            meets_key(Some(d("8")), "7 5", NumberFormat::Comma),
            "unread"
        );
        assert_eq!(meets_key(Some(d("8")), "7,5 %", NumberFormat::Comma), "yes");
    }

    #[test]
    fn a_non_positive_old_average_is_named_on_line_10() {
        let mut out = QuickScreenOutputs::default();
        out.eps.old_avg = Some(d("-1"));
        out.eps.increase = Some(d("2"));
        let v = quick_screen_view(head(), &out, NumberFormat::Comma);
        assert!(v.eps.nonpositive_base);
        assert_eq!(v.eps.lines[9], "");
        assert!(!v.sales.nonpositive_base);
    }

    /// The decision is taken on the rate as shown: 6,96 % reads « 7,0 % », which meets 7.
    #[test]
    fn the_objective_is_judged_on_the_displayed_rate() {
        assert_eq!(meets_key(Some(d("6.96")), "7", NumberFormat::Comma), "yes");
        assert_eq!(meets_key(Some(d("6.94")), "7", NumberFormat::Comma), "no");
    }

    fn head() -> QuickScreenHeader {
        QuickScreenHeader {
            ticker: "t".into(),
            name: String::new(),
            currency: "chf".into(),
            date: "2026-09-25".into(),
            source: String::new(),
        }
    }

    fn year_row(
        y: i32,
        eps: &str,
        high: &str,
        low: &str,
    ) -> steadyinvest_core::normalize::CanonicalYear {
        steadyinvest_core::normalize::CanonicalYear {
            year: y,
            sales: Some(d("100")),
            eps: Some(d(eps)),
            high_price: Some(d(high)),
            low_price: Some(d(low)),
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
            usability: steadyinvest_core::normalize::YearUsability::Usable,
        }
    }

    /// Spec §6 end to end: the form's conversion table through the real ladder AND the display —
    /// averages 100 → 127,6 over the six-year window read « 5,0 % », 100 → 371,3 « 30,0 % ».
    #[test]
    fn the_conversion_table_reads_on_the_screen() {
        let rate = |recent: &str| {
            let years: Vec<_> = [
                (2021, "100"),
                (2022, "100"),
                (2023, "1"),
                (2024, "1"),
                (2025, recent),
                (2026, recent),
            ]
            .into_iter()
            .map(|(y, v)| year_row(y, v, "10", "5"))
            .collect();
            let out = steadyinvest_core::checklist::quick_screen(&years, None, None);
            quick_screen_view(head(), &out, NumberFormat::Comma)
                .eps
                .rate
        };
        assert_eq!(rate("127.6"), "5,0 %");
        assert_eq!(rate("371.3"), "30,0 %");
    }

    #[test]
    fn the_price_bases_follow_the_record() {
        let full: Vec<_> = (2022..=2026)
            .map(|y| year_row(y, "5", "100", "50"))
            .collect();
        let out = steadyinvest_core::checklist::quick_screen(&full, Some(d("110")), Some(d("5")));
        let v = quick_screen_view(head(), &out, NumberFormat::Comma);
        assert_eq!(
            (
                v.pe_basis.as_str(),
                v.high_basis.as_str(),
                v.sold_basis.as_str()
            ),
            ("five", "five", "five")
        );
        assert_eq!(v.price_vs_high, "higher");
        assert_eq!(v.pe_absent, "");
        // A short record: counts, never « cinq ».
        let out =
            steadyinvest_core::checklist::quick_screen(&full[2..], Some(d("90")), Some(d("5")));
        let v = quick_screen_view(head(), &out, NumberFormat::Comma);
        assert_eq!(
            (
                v.pe_basis.as_str(),
                v.high_basis.as_str(),
                v.sold_basis.as_str()
            ),
            ("all", "year", "count")
        );
        assert_eq!((v.pe_years.as_str(), v.high_year.as_str()), ("3", "2024"));
        assert_eq!(
            (v.years_sold_as_high.as_str(), v.sold_of.as_str()),
            ("3", "3")
        );
        assert_eq!(v.price_vs_high, "lower");
        // One row without a P/E: partial.
        let mut gap = full.clone();
        gap[1].eps = Some(d("-1"));
        let out = steadyinvest_core::checklist::quick_screen(&gap, Some(d("110")), Some(d("5")));
        let v = quick_screen_view(head(), &out, NumberFormat::Comma);
        assert_eq!(
            (
                v.pe_basis.as_str(),
                v.pe_years.as_str(),
                v.record_years.as_str()
            ),
            ("partial", "4", "5")
        );
        // No price: the facts are absent (the layouts print « — »), and so is the P/E.
        let out = steadyinvest_core::checklist::quick_screen(&full, None, Some(d("5")));
        let v = quick_screen_view(head(), &out, NumberFormat::Comma);
        assert_eq!((v.sold_basis.as_str(), v.price_vs_high.as_str()), ("", ""));
        assert_eq!(v.years_sold_as_high, "");
        assert_eq!(v.pe_absent, "pe");
        // A present P/E but no P/E in the record: the AVERAGE is missing, not the present P/E.
        let mut no_pe = full.clone();
        for y in &mut no_pe {
            y.eps = Some(d("0"));
        }
        let out = steadyinvest_core::checklist::quick_screen(&no_pe, Some(d("110")), Some(d("5")));
        let v = quick_screen_view(head(), &out, NumberFormat::Comma);
        assert_eq!((v.pe_basis.as_str(), v.pe_absent.as_str()), ("", "average"));
        assert!(!v.present_pe.is_empty());
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
