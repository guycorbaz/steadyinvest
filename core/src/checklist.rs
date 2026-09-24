//! Story 7.3 — the « Stock Check List for Beginning Investors » (1996) arithmetic: two six-year
//! ladders (sales, EPS) reduced to a compound annual rate each, a five-year price / P/E record
//! reduced to an average P/E, and the three price facts the form asks the reader to write in.
//! PURE: `Decimal` in, `Option<Decimal>` out — an absent figure is `None`, never 0; the roots go
//! through [`crate::ssg::growth::endpoints_cagr_pct`] (the same exact-root helper as §1).
//!
//! The form's conversion table (« 27 % increase ↔ 5 % compounded », « 271 % ↔ 30 % ») is
//! `(1 + r)^5 = 1 + increase` — five years between the two two-year averages, even though the
//! averages' midpoints sit four years apart. Fidelity to the form wins (NFR-U3); the span is
//! reported so the screen can state it.

use rust_decimal::Decimal;

use crate::normalize::CanonicalYear;
use crate::ssg::endpoints_cagr_pct;

/// The form's span between the recent and the old two-year averages (its conversion table).
pub const FORM_SPAN_YEARS: u32 = 5;
/// Fewer usable years than this and a ladder is « indisponible » (spec §6).
pub const MIN_LADDER_YEARS: usize = 3;
/// « voisin » band around the five-year average P/E, in percent of it.
pub const PE_SIMILAR_BAND_PCT: u32 = 10;

/// One ladder (§1 sales, §2 EPS): the form's ten lines and the compound rate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ladder {
    /// (1) the most recent year's figure, and its year.
    pub recent: Option<Decimal>,
    pub recent_year: Option<i32>,
    /// (2) the year before, and its year.
    pub recent_prior: Option<Decimal>,
    pub recent_prior_year: Option<i32>,
    /// (3) (1) + (2); (4) ÷ 2.
    pub recent_total: Option<Decimal>,
    pub recent_avg: Option<Decimal>,
    /// (5) the figure `span` years before (1), and its year.
    pub old: Option<Decimal>,
    pub old_year: Option<i32>,
    /// (6) the year before (5), and its year.
    pub old_prior: Option<Decimal>,
    pub old_prior_year: Option<i32>,
    /// (7) (5) + (6); (8) ÷ 2.
    pub old_total: Option<Decimal>,
    pub old_avg: Option<Decimal>,
    /// (9) (4) − (8); (10) (9) ÷ (8) in percent.
    pub increase: Option<Decimal>,
    pub increase_pct: Option<Decimal>,
    /// The compound annual rate over `span_years`, in percent.
    pub compound_rate_pct: Option<Decimal>,
    /// The years between (1) and (5): the form's five when the series allows, else the real span.
    pub span_years: u32,
    /// `true` when the series had fewer than [`MIN_LADDER_YEARS`] figures: every line is `None`.
    pub unavailable: bool,
}

/// One row of the §3 price record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PriceRow {
    pub year: i32,
    pub high: Option<Decimal>,
    pub low: Option<Decimal>,
    pub eps: Option<Decimal>,
    /// (A ÷ C), (B ÷ C) — `None` on a missing or non-positive EPS.
    pub pe_high: Option<Decimal>,
    pub pe_low: Option<Decimal>,
}

/// Where the present P/E stands against the five-year average of averages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PePosition {
    Higher,
    Similar,
    Lower,
}

/// Which of the two rates grew faster (the form's « EPS have increased more / less than sales »).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateComparison {
    EpsFaster,
    SalesFaster,
    Same,
}

/// The §3 price record and its facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PriceRecord {
    /// The last five years with a price, oldest first.
    pub rows: Vec<PriceRow>,
    pub pe_high_total: Option<Decimal>,
    pub pe_low_total: Option<Decimal>,
    pub pe_high_avg: Option<Decimal>,
    pub pe_low_avg: Option<Decimal>,
    /// « Average of the high and low P/E averages ».
    pub pe_avg_of_avgs: Option<Decimal>,
    pub present_price: Option<Decimal>,
    pub present_eps: Option<Decimal>,
    pub present_pe: Option<Decimal>,
    /// The high of the oldest row (« the high price five years ago ») and the present price's
    /// distance from it, in percent (positive = higher).
    pub high_five_years_ago: Option<Decimal>,
    pub price_vs_high_pct: Option<Decimal>,
    /// « This stock has sold as high as the current price in N of the last 5 years ».
    pub years_sold_as_high: Option<u32>,
    pub pe_position: Option<PePosition>,
}

/// The whole examination.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuickScreenOutputs {
    pub sales: Ladder,
    pub eps: Ladder,
    pub eps_vs_sales: Option<RateComparison>,
    pub price: PriceRecord,
}

fn two() -> Decimal {
    Decimal::from(2)
}

/// Build one ladder from `(year, figure)` pairs (ascending, figures present).
fn ladder(points: &[(i32, Decimal)]) -> Ladder {
    if points.len() < MIN_LADDER_YEARS {
        return Ladder {
            unavailable: true,
            ..Ladder::default()
        };
    }
    let n = points.len();
    let (recent_year, recent) = points[n - 1];
    let (recent_prior_year, recent_prior) = points[n - 2];
    // The form's old pair: `span` and `span + 1` years before the recent year; when the series
    // lacks them, the oldest consecutive pair it has, and the real span.
    let by_year = |y: i32| points.iter().find(|(py, _)| *py == y).map(|(_, v)| *v);
    let form_old = recent_year - FORM_SPAN_YEARS as i32;
    let (old_year, old, old_prior_year, old_prior, span) =
        match (by_year(form_old), by_year(form_old - 1)) {
            (Some(o), Some(op)) => (form_old, o, form_old - 1, op, FORM_SPAN_YEARS),
            _ => {
                let (oy, o) = points[1];
                let (opy, op) = points[0];
                (oy, o, opy, op, (recent_year - oy).max(1) as u32)
            }
        };
    let recent_total = recent.checked_add(recent_prior);
    let recent_avg = recent_total.and_then(|t| t.checked_div(two()));
    let old_total = old.checked_add(old_prior);
    let old_avg = old_total.and_then(|t| t.checked_div(two()));
    let increase = match (recent_avg, old_avg) {
        (Some(r), Some(o)) => r.checked_sub(o),
        _ => None,
    };
    let increase_pct = match (increase, old_avg) {
        (Some(i), Some(o)) if o != Decimal::ZERO => i
            .checked_div(o)
            .and_then(|q| q.checked_mul(Decimal::ONE_HUNDRED)),
        _ => None,
    };
    let compound_rate_pct = match (old_avg, recent_avg) {
        (Some(o), Some(r)) => endpoints_cagr_pct(o, r, span),
        _ => None,
    };
    Ladder {
        recent: Some(recent),
        recent_year: Some(recent_year),
        recent_prior: Some(recent_prior),
        recent_prior_year: Some(recent_prior_year),
        recent_total,
        recent_avg,
        old: Some(old),
        old_year: Some(old_year),
        old_prior: Some(old_prior),
        old_prior_year: Some(old_prior_year),
        old_total,
        old_avg,
        increase,
        increase_pct,
        compound_rate_pct,
        span_years: span,
        unavailable: false,
    }
}

fn pe(price: Option<Decimal>, eps: Option<Decimal>) -> Option<Decimal> {
    match (price, eps) {
        (Some(p), Some(e)) if e > Decimal::ZERO => p.checked_div(e),
        _ => None,
    }
}

fn sum(values: impl Iterator<Item = Option<Decimal>>) -> (Option<Decimal>, u32) {
    let mut total: Option<Decimal> = None;
    let mut count = 0u32;
    for v in values.flatten() {
        total = Some(total.unwrap_or(Decimal::ZERO).checked_add(v).unwrap_or(v));
        count += 1;
    }
    (total, count)
}

fn price_record(
    years: &[CanonicalYear],
    present_price: Option<Decimal>,
    present_eps: Option<Decimal>,
) -> PriceRecord {
    let with_price: Vec<&CanonicalYear> = years
        .iter()
        .filter(|y| y.high_price.is_some() || y.low_price.is_some())
        .collect();
    let start = with_price.len().saturating_sub(5);
    let rows: Vec<PriceRow> = with_price[start..]
        .iter()
        .map(|y| PriceRow {
            year: y.year,
            high: y.high_price,
            low: y.low_price,
            eps: y.eps,
            pe_high: pe(y.high_price, y.eps),
            pe_low: pe(y.low_price, y.eps),
        })
        .collect();
    let (pe_high_total, n_high) = sum(rows.iter().map(|r| r.pe_high));
    let (pe_low_total, n_low) = sum(rows.iter().map(|r| r.pe_low));
    let avg = |t: Option<Decimal>, n: u32| t.and_then(|t| t.checked_div(Decimal::from(n)));
    let pe_high_avg = avg(pe_high_total, n_high);
    let pe_low_avg = avg(pe_low_total, n_low);
    let pe_avg_of_avgs = match (pe_high_avg, pe_low_avg) {
        (Some(h), Some(l)) => h.checked_add(l).and_then(|s| s.checked_div(two())),
        _ => None,
    };
    let present_pe = pe(present_price, present_eps);
    let high_five_years_ago = rows.first().and_then(|r| r.high);
    let price_vs_high_pct = match (present_price, high_five_years_ago) {
        (Some(p), Some(h)) if h > Decimal::ZERO => p
            .checked_sub(h)
            .and_then(|d| d.checked_div(h))
            .and_then(|q| q.checked_mul(Decimal::ONE_HUNDRED)),
        _ => None,
    };
    let years_sold_as_high = present_price.map(|p| {
        rows.iter()
            .filter(|r| r.high.is_some_and(|h| h >= p))
            .count() as u32
    });
    let pe_position = match (present_pe, pe_avg_of_avgs) {
        (Some(now), Some(avg)) if avg > Decimal::ZERO => {
            let band = avg * Decimal::from(PE_SIMILAR_BAND_PCT) / Decimal::ONE_HUNDRED;
            Some(if now > avg + band {
                PePosition::Higher
            } else if now < avg - band {
                PePosition::Lower
            } else {
                PePosition::Similar
            })
        }
        _ => None,
    };
    PriceRecord {
        rows,
        pe_high_total,
        pe_low_total,
        pe_high_avg,
        pe_low_avg,
        pe_avg_of_avgs,
        present_price,
        present_eps,
        present_pe,
        high_five_years_ago,
        price_vs_high_pct,
        years_sold_as_high,
        pe_position,
    }
}

/// The examination of a canonical series (ascending years). `present_eps` is the TTM figure when
/// known; the caller falls back to the latest annual EPS.
pub fn quick_screen(
    years: &[CanonicalYear],
    present_price: Option<Decimal>,
    present_eps: Option<Decimal>,
) -> QuickScreenOutputs {
    let sales_points: Vec<(i32, Decimal)> = years
        .iter()
        .filter_map(|y| y.sales.map(|s| (y.year, s)))
        .collect();
    let eps_points: Vec<(i32, Decimal)> = years
        .iter()
        .filter_map(|y| y.eps.map(|e| (y.year, e)))
        .collect();
    let sales = ladder(&sales_points);
    let eps = ladder(&eps_points);
    let eps_vs_sales = match (eps.compound_rate_pct, sales.compound_rate_pct) {
        (Some(e), Some(s)) if e > s => Some(RateComparison::EpsFaster),
        (Some(e), Some(s)) if e < s => Some(RateComparison::SalesFaster),
        (Some(_), Some(_)) => Some(RateComparison::Same),
        _ => None,
    };
    QuickScreenOutputs {
        sales,
        eps,
        eps_vs_sales,
        price: price_record(years, present_price, present_eps),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::YearUsability;

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    fn year(y: i32, sales: &str, eps: &str, high: &str, low: &str) -> CanonicalYear {
        CanonicalYear {
            year: y,
            sales: Some(d(sales)),
            eps: Some(d(eps)),
            high_price: Some(d(high)),
            low_price: Some(d(low)),
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
            usability: YearUsability::Usable,
        }
    }

    /// The form's conversion table: 27 % ↔ 5 %, 271 % ↔ 30 % (rounded to one decimal).
    #[test]
    fn the_conversion_table_is_the_fifth_root() {
        let r = |increase_pct: &str| {
            let old = d("100");
            let recent = old + old * d(increase_pct) / Decimal::ONE_HUNDRED;
            endpoints_cagr_pct(old, recent, FORM_SPAN_YEARS)
                .unwrap()
                .round_dp(1)
        };
        assert_eq!(r("27.6"), d("5.0"));
        assert_eq!(r("271.3"), d("30.0"));
        assert_eq!(r("61"), d("10.0"));
    }

    #[test]
    fn a_six_year_ladder_follows_the_form() {
        // Sales 100, 110, …, 200 over 2020–2026 (seven years): recent pair 2026/2025, old pair
        // 2021/2020 (five and six years before 2026), span 5.
        let years: Vec<CanonicalYear> = (0..7)
            .map(|i| {
                let s = 100 + 10 * i;
                year(2020 + i, &s.to_string(), "1", "10", "5")
            })
            .collect();
        let out = quick_screen(&years, Some(d("12")), Some(d("1")));
        let l = &out.sales;
        assert_eq!(
            (l.recent_year, l.recent_prior_year),
            (Some(2026), Some(2025))
        );
        assert_eq!((l.old_year, l.old_prior_year), (Some(2021), Some(2020)));
        assert_eq!(l.recent_avg, Some(d("155")));
        assert_eq!(l.old_avg, Some(d("105")));
        assert_eq!(l.increase, Some(d("50")));
        assert_eq!(l.increase_pct.map(|p| p.round_dp(2)), Some(d("47.62")));
        assert_eq!(l.span_years, 5);
        // (155/105)^(1/5) − 1 = 8.1 %
        assert_eq!(l.compound_rate_pct.map(|p| p.round_dp(1)), Some(d("8.1")));
        // EPS flat → 0 % → sales grew faster.
        assert_eq!(
            out.eps.compound_rate_pct.map(|p| p.round_dp(1)),
            Some(d("0.0"))
        );
        assert_eq!(out.eps_vs_sales, Some(RateComparison::SalesFaster));
    }

    #[test]
    fn a_short_series_uses_its_real_span_and_a_very_short_one_is_unavailable() {
        let years: Vec<CanonicalYear> = (0..4)
            .map(|i| year(2023 + i, &(100 + 10 * i).to_string(), "1", "10", "5"))
            .collect();
        let l = quick_screen(&years, None, None).sales;
        assert!(!l.unavailable);
        assert_eq!((l.old_year, l.old_prior_year), (Some(2024), Some(2023)));
        assert_eq!(l.span_years, 2, "2026 − 2024");
        let two: Vec<CanonicalYear> = years[..2].to_vec();
        assert!(quick_screen(&two, None, None).sales.unavailable);
        assert_eq!(quick_screen(&two, None, None).sales.compound_rate_pct, None);
    }

    #[test]
    fn the_price_record_averages_pes_and_states_the_three_facts() {
        let years = vec![
            year(2021, "1", "2", "40", "20"),  // P/E 20 / 10
            year(2022, "1", "2", "50", "30"),  // 25 / 15
            year(2023, "1", "4", "80", "40"),  // 20 / 10
            year(2024, "1", "4", "100", "60"), // 25 / 15
            year(2025, "1", "5", "100", "50"), // 20 / 10
            year(2026, "1", "5", "120", "60"), // 24 / 12
        ];
        let out = quick_screen(&years, Some(d("110")), Some(d("5")));
        let p = &out.price;
        assert_eq!(p.rows.len(), 5, "the last five years");
        assert_eq!(p.rows[0].year, 2022);
        assert_eq!(p.pe_high_total, Some(d("114")));
        assert_eq!(p.pe_low_total, Some(d("62")));
        assert_eq!(p.pe_high_avg, Some(d("22.8")));
        assert_eq!(p.pe_low_avg, Some(d("12.4")));
        assert_eq!(p.pe_avg_of_avgs, Some(d("17.6")));
        assert_eq!(p.present_pe, Some(d("22")));
        assert_eq!(p.high_five_years_ago, Some(d("50")));
        assert_eq!(p.price_vs_high_pct, Some(d("120")));
        // Highs ≥ 110: 2026 (120) only.
        assert_eq!(p.years_sold_as_high, Some(1));
        // 22 > 17.6 × 1.1 = 19.36 → higher.
        assert_eq!(p.pe_position, Some(PePosition::Higher));
        // A non-positive EPS yields no P/E, never a negative multiple.
        let mut neg = years.clone();
        neg[5].eps = Some(d("-1"));
        let out = quick_screen(&neg, Some(d("110")), Some(d("-1")));
        assert_eq!(out.price.rows[4].pe_high, None);
        assert_eq!(out.price.present_pe, None);
        assert_eq!(out.price.pe_position, None);
    }
}
