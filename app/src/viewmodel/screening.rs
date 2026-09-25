//! Story 7.3 (PR 2) — the watchlist « criblage »: one row per watched ticker, the examination's
//! four facts side by side, in the watchlist's order. No ranking, no sort, no pass / fail: the
//! reader's objective is deliberately NOT applied here (a pass / fail column is a ranking in
//! disguise). Pure formatting + the row states' transitions, tested.

use steadyinvest_core::checklist::{Ladder, PePosition, QuickScreenOutputs, RateComparison};
use steadyinvest_core::rounding::DisplayField;
use steadyinvest_ingestion::IngestionError;

use crate::viewmodel::format::{NumberFormat, format_scaled};

/// The form reads six years (two two-year averages, five years apart).
pub const FORM_YEARS: u32 = 6;

/// Where a row stands in the run. `Examined` carries the examination (kept whole so « Ouvrir
/// l'examen » opens it without a second fetch).
#[derive(Clone)]
pub enum RowState<T> {
    /// Queued behind the paced fetches.
    Pending,
    Examined(T),
    /// The fetch failed for another cause than the usage limit (unknown symbol, no data, network…).
    Unavailable,
    /// The usage limit stopped the run on or before this row — « non examiné (limite d'usage) ».
    NotExamined,
}

/// A fetch's failure → the row's state: the usage limit reads « non examiné », anything else
/// « indisponible » (on that row only — the run goes on).
pub fn failed_state<T>(error: &IngestionError) -> RowState<T> {
    if crate::fetch::is_quota(error) {
        RowState::NotExamined
    } else {
        RowState::Unavailable
    }
}

/// How many of the form's six years a ladder spans (the recent pair back to the old pair).
fn ladder_years(l: &Ladder) -> Option<u32> {
    if l.unavailable {
        return None;
    }
    let (recent, oldest) = (l.recent_year?, l.old_prior_year?);
    Some(((recent - oldest + 1).max(0) as u32).min(FORM_YEARS))
}

/// The years the examination rests on (the shorter of the two ladders); `None` = a ladder is
/// unavailable (fewer than two non-overlapping consecutive pairs).
pub fn years_used(out: &QuickScreenOutputs) -> Option<u32> {
    match (ladder_years(&out.sales), ladder_years(&out.eps)) {
        (Some(s), Some(e)) => Some(s.min(e)),
        _ => None,
    }
}

/// One formatted criblage row (keys cross as keys; the screen words them under `@tr`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScreeningRowView {
    pub ticker: String,
    /// pending | done | unavailable | quota
    pub state: String,
    /// « 6 / 6 », « 4 / 6 »; "" before the examination or when a ladder is unavailable.
    pub years: String,
    /// A ladder is unavailable: its six-year window holds no two disjoint consecutive pairs — the
    /// screen words the cause (the window), never « < 4 » (G1 final review: a long series with a
    /// gap in the window is not « fewer than four years »).
    pub window_short: bool,
    pub sales_rate: String,
    pub eps_rate: String,
    /// eps | sales | same | ""
    pub eps_vs_sales: String,
    /// higher | similar | lower | ""
    pub pe_position: String,
    /// The present price against the high five years ago, signed; "" = unknown.
    pub price_vs_high: String,
    pub has_study: bool,
}

fn pct(v: Option<rust_decimal::Decimal>, format: NumberFormat) -> String {
    v.map(|d| format!("{} %", format_scaled(d, DisplayField::Percent, format)))
        .unwrap_or_default()
}

/// Format one row from its state (the examination's outputs when it has one).
pub fn screening_row_view(
    ticker: &str,
    has_study: bool,
    state: &RowState<&QuickScreenOutputs>,
    format: NumberFormat,
) -> ScreeningRowView {
    let base = ScreeningRowView {
        ticker: ticker.to_uppercase(),
        has_study,
        ..ScreeningRowView::default()
    };
    let out = match state {
        RowState::Pending => {
            return ScreeningRowView {
                state: "pending".into(),
                ..base
            };
        }
        RowState::Unavailable => {
            return ScreeningRowView {
                state: "unavailable".into(),
                ..base
            };
        }
        RowState::NotExamined => {
            return ScreeningRowView {
                state: "quota".into(),
                ..base
            };
        }
        RowState::Examined(out) => *out,
    };
    ScreeningRowView {
        state: "done".into(),
        years: years_used(out)
            .map(|n| format!("{n} / {FORM_YEARS}"))
            .unwrap_or_default(),
        window_short: years_used(out).is_none(),
        sales_rate: pct(out.sales.compound_rate_pct, format),
        eps_rate: pct(out.eps.compound_rate_pct, format),
        eps_vs_sales: match out.eps_vs_sales {
            Some(RateComparison::EpsFaster) => "eps",
            Some(RateComparison::SalesFaster) => "sales",
            Some(RateComparison::Same) => "same",
            Some(RateComparison::DifferentYears) => "years",
            None => "",
        }
        .into(),
        pe_position: match out.price.pe_position {
            Some(PePosition::Higher) => "higher",
            Some(PePosition::Similar) => "similar",
            Some(PePosition::Lower) => "lower",
            None => "",
        }
        .into(),
        price_vs_high: pct(out.price.price_vs_high_pct, format),
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use steadyinvest_ingestion::ProviderError;

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    fn ladder(recent: i32, oldest: i32, rate: &str) -> Ladder {
        Ladder {
            recent_year: Some(recent),
            old_prior_year: Some(oldest),
            compound_rate_pct: Some(d(rate)),
            ..Ladder::default()
        }
    }

    #[test]
    fn a_quota_reads_not_examined_and_any_other_failure_unavailable() {
        let quota = IngestionError::Provider(ProviderError::Quota {
            retry_after_secs: None,
        });
        assert!(matches!(failed_state::<()>(&quota), RowState::NotExamined));
        let unknown = IngestionError::Provider(ProviderError::TickerNotFound {
            ticker: "XXX".into(),
        });
        assert!(matches!(
            failed_state::<()>(&unknown),
            RowState::Unavailable
        ));
    }

    #[test]
    fn the_years_used_are_the_shorter_ladder_capped_at_the_forms_six() {
        let mut out = QuickScreenOutputs {
            sales: ladder(2025, 2020, "8.2"),
            eps: ladder(2025, 2022, "5.1"),
            ..QuickScreenOutputs::default()
        };
        assert_eq!(years_used(&out), Some(4));
        out.eps = ladder(2025, 2015, "5.1");
        assert_eq!(years_used(&out), Some(6), "a longer series still reads six");
        out.eps.unavailable = true;
        assert_eq!(years_used(&out), None);
    }

    #[test]
    fn an_examined_row_states_the_four_facts_and_no_verdict() {
        let mut out = QuickScreenOutputs {
            sales: ladder(2025, 2020, "8.2"),
            eps: ladder(2025, 2020, "5.1"),
            eps_vs_sales: Some(RateComparison::SalesFaster),
            ..QuickScreenOutputs::default()
        };
        out.price.pe_position = Some(PePosition::Higher);
        out.price.price_vs_high_pct = Some(d("-40.2"));
        let v = screening_row_view(
            "nesn.sw",
            true,
            &RowState::Examined(&out),
            NumberFormat::Comma,
        );
        assert_eq!(v.ticker, "NESN.SW");
        assert_eq!(v.state, "done");
        assert_eq!(v.years, "6 / 6");
        assert_eq!(v.sales_rate, "8,2 %");
        assert_eq!(v.eps_rate, "5,1 %");
        assert_eq!(v.eps_vs_sales, "sales");
        assert_eq!(v.pe_position, "higher");
        assert_eq!(v.price_vs_high, "−40,2 %");
        assert!(v.has_study);
    }

    #[test]
    fn a_row_without_an_examination_carries_its_state_and_no_figure() {
        for (state, key) in [
            (RowState::Pending, "pending"),
            (RowState::Unavailable, "unavailable"),
            (RowState::NotExamined, "quota"),
        ] {
            let v = screening_row_view("rog.sw", false, &state, NumberFormat::Comma);
            assert_eq!(v.state, key);
            assert_eq!(v.years, "");
            assert_eq!(v.sales_rate, "");
            assert!(!v.has_study);
        }
    }

    #[test]
    fn a_short_series_names_its_shortfall() {
        let mut out = QuickScreenOutputs::default();
        out.sales.unavailable = true;
        let v = screening_row_view("x", false, &RowState::Examined(&out), NumberFormat::Comma);
        assert_eq!(v.years, "", "never « < 4 / 6 »: the cause is the window");
        assert!(v.window_short);
        assert_eq!(v.sales_rate, "");
        // Two ladders over different years: said, never compared.
        let mut out = QuickScreenOutputs {
            sales: ladder(2025, 2020, "8.2"),
            eps: ladder(2024, 2019, "5.1"),
            eps_vs_sales: Some(RateComparison::DifferentYears),
            ..QuickScreenOutputs::default()
        };
        let v = screening_row_view("x", false, &RowState::Examined(&out), NumberFormat::Comma);
        assert_eq!(v.eps_vs_sales, "years");
        assert!(!v.window_short);
        out.eps_vs_sales = None;
    }
}
