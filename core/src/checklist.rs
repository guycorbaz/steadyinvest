//! Story 7.3 — the « Stock Check List for Beginning Investors » (1996) arithmetic: two six-year
//! ladders (sales, EPS) reduced to a compound annual rate each, a five-year price / P/E record
//! reduced to an average P/E, and the three price facts the form asks the reader to write in.
//! PURE: `Decimal` in, `Option<Decimal>` out — an absent figure is `None`, never 0; the roots go
//! through [`crate::ssg::growth::endpoints_cagr_pct`] (the same exact-root helper as §1).
//!
//! A ladder reads a six-year window: (1)–(2) the recent year and the one before, (5)–(6) the
//! fifth and sixth years back counting the recent one as the first (`recent − 4`, `recent − 5`).
//! The form's conversion table (« 27 % increase ↔ 5 % compounded », « 271 % ↔ 30 % ») is
//! `(1 + r)^5 = 1 + increase` — five years for the six-year window, even though the averages'
//! midpoints sit four years apart. Fidelity to the form wins (NFR-U3, spec Q2); the span is
//! reported so the screen can state it.

use rust_decimal::Decimal;

use crate::normalize::CanonicalYear;
use crate::ssg::endpoints_cagr_pct;

/// The form's exponent for its six-year window (its conversion table).
pub const FORM_SPAN_YEARS: u32 = 5;
/// Fewer usable years than this and a ladder is « indisponible »: two consecutive pairs that do
/// not overlap (G1 review — three years made the middle one serve both pairs).
pub const MIN_LADDER_YEARS: usize = 4;
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
    /// (5) the figure of the `span`-th year counting (1) as the first, and its year.
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
    /// The exponent: the window's length minus one — the form's five for its six years, fewer when
    /// the series only allows a shorter window.
    pub span_years: u32,
    /// `true` when the series holds no two non-overlapping consecutive pairs inside the six-year
    /// window: every line is `None`.
    pub unavailable: bool,
}

/// One row of the §3 price record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PriceRow {
    pub year: i32,
    pub high: Option<Decimal>,
    pub low: Option<Decimal>,
    pub eps: Option<Decimal>,
    /// (A ÷ C), (B ÷ C) — `None` on a missing or non-positive price or EPS.
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
///
/// Absence honesty (G1 review): the P/E totals and averages cover the SAME rows — those with both
/// a high and a low P/E — and `pe_years` states how many; a « cinq ans » wording is the layout's
/// only when `five_year_record` holds and the figure covers all five rows. A count over no row is
/// `None`, never `0`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PriceRecord {
    /// The last five years with a price, oldest first.
    pub rows: Vec<PriceRow>,
    /// `true` when `rows` are the form's five consecutive years.
    pub five_year_record: bool,
    /// The rows the P/E totals and averages cover (both P/Es present).
    pub pe_years: u32,
    pub pe_high_total: Option<Decimal>,
    pub pe_low_total: Option<Decimal>,
    pub pe_high_avg: Option<Decimal>,
    pub pe_low_avg: Option<Decimal>,
    /// « Average of the high and low P/E averages ».
    pub pe_avg_of_avgs: Option<Decimal>,
    pub present_price: Option<Decimal>,
    pub present_eps: Option<Decimal>,
    pub present_pe: Option<Decimal>,
    /// The high of the oldest row (« the high price five years ago » on a full record), its
    /// year, and the present price's distance from it, in percent (positive = higher).
    pub high_five_years_ago: Option<Decimal>,
    pub high_year: Option<i32>,
    pub price_vs_high_pct: Option<Decimal>,
    /// « This stock has sold as high as the current price in N of the last 5 years »: N over the
    /// `high_years` rows whose high is known; `None` without a present price or any known high.
    pub years_sold_as_high: Option<u32>,
    pub high_years: u32,
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

/// The two pairs of a ladder, by YEAR (never by position in the series — a gap must not pair
/// two years that are not consecutive): the most recent consecutive pair `(r, r − 1)`, and the
/// old pair `(o, o − 1)` — the form's `o = r − 4` when present, else the oldest consecutive pair
/// still inside the six-year window and disjoint from the recent pair (`r − 4 < o ≤ r − 2`).
/// `None` when either pair is missing. Returns `(r, o)`.
fn ladder_pairs(points: &[(i32, Decimal)]) -> Option<(i32, i32)> {
    let has = |y: i32| points.iter().any(|(py, _)| *py == y);
    let recent = points.iter().rev().map(|(y, _)| *y).find(|y| has(y - 1))?;
    let window = FORM_SPAN_YEARS as i32 - 1; // r − 4: the form's (5)
    (recent - window..=recent - 2)
        .find(|o| has(*o) && has(o - 1))
        .map(|o| (recent, o))
}

/// Build one ladder from `(year, figure)` pairs (ascending, figures present).
fn ladder(points: &[(i32, Decimal)]) -> Ladder {
    let Some((recent_year, old_year)) = ladder_pairs(points) else {
        return Ladder {
            unavailable: true,
            ..Ladder::default()
        };
    };
    let at = |y: i32| {
        points
            .iter()
            .find(|(py, _)| *py == y)
            .map(|(_, v)| *v)
            .expect("ladder_pairs checked the year")
    };
    let (recent_prior_year, old_prior_year) = (recent_year - 1, old_year - 1);
    let (recent, recent_prior) = (at(recent_year), at(recent_prior_year));
    let (old, old_prior) = (at(old_year), at(old_prior_year));
    // The window runs from (6) to (1): its length minus one is the exponent — the form's five.
    let span = (recent_year - old_prior_year) as u32;
    let recent_total = recent.checked_add(recent_prior);
    let recent_avg = recent_total.and_then(|t| t.checked_div(two()));
    let old_total = old.checked_add(old_prior);
    let old_avg = old_total.and_then(|t| t.checked_div(two()));
    let increase = match (recent_avg, old_avg) {
        (Some(r), Some(o)) => r.checked_sub(o),
        _ => None,
    };
    // A non-positive old average has no meaningful « hausse en pour cent »: dividing by it would
    // invert the sign (a recovery from −1 to +1 read as −200 %) — absent, never wrong (G1 review).
    let increase_pct = match (increase, old_avg) {
        (Some(i), Some(o)) if o > Decimal::ZERO => i
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

/// A P/E needs a positive price AND a positive EPS: a zero or negative price is a data fault, not
/// a « PER de 0 » — absent, never wrong (G1 D review). Every P/E, and so every P/E average, is
/// then positive when present.
fn pe(price: Option<Decimal>, eps: Option<Decimal>) -> Option<Decimal> {
    match (price, eps) {
        (Some(p), Some(e)) if p > Decimal::ZERO && e > Decimal::ZERO => p.checked_div(e),
        _ => None,
    }
}

/// The sum of every value; `None` for no value or on an overflow — never the values that fit
/// passed off as the total (G1 review: an overflow used to restart the sum at the last value).
fn total(values: impl Iterator<Item = Decimal>) -> Option<Decimal> {
    let mut acc: Option<Decimal> = None;
    for v in values {
        acc = Some(match acc {
            None => v,
            Some(a) => a.checked_add(v)?,
        });
    }
    acc
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
    let five_year_record =
        rows.len() == 5 && rows.last().map(|r| r.year) == rows.first().map(|r| r.year + 4);
    // The high and low P/E columns are summed over the SAME rows (both P/Es present), so the two
    // averages — and their average — describe one set of years, whose count is stated.
    let pe_pairs: Vec<(Decimal, Decimal)> = rows
        .iter()
        .filter_map(|r| Some((r.pe_high?, r.pe_low?)))
        .collect();
    let pe_years = pe_pairs.len() as u32;
    let pe_high_total = total(pe_pairs.iter().map(|(h, _)| *h));
    let pe_low_total = total(pe_pairs.iter().map(|(_, l)| *l));
    let avg = |t: Option<Decimal>| t.and_then(|t| t.checked_div(Decimal::from(pe_years)));
    let pe_high_avg = avg(pe_high_total);
    let pe_low_avg = avg(pe_low_total);
    let pe_avg_of_avgs = match (pe_high_avg, pe_low_avg) {
        (Some(h), Some(l)) => h.checked_add(l).and_then(|s| s.checked_div(two())),
        _ => None,
    };
    let present_pe = pe(present_price, present_eps);
    let high_five_years_ago = rows.first().and_then(|r| r.high);
    let high_year = rows.first().filter(|r| r.high.is_some()).map(|r| r.year);
    let price_vs_high_pct = match (present_price, high_five_years_ago) {
        (Some(p), Some(h)) if h > Decimal::ZERO => p
            .checked_sub(h)
            .and_then(|d| d.checked_div(h))
            .and_then(|q| q.checked_mul(Decimal::ONE_HUNDRED)),
        _ => None,
    };
    let high_years = rows.iter().filter(|r| r.high.is_some()).count() as u32;
    let years_sold_as_high = present_price.filter(|_| high_years > 0).map(|p| {
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
        five_year_record,
        pe_years,
        pe_high_total,
        pe_low_total,
        pe_high_avg,
        pe_low_avg,
        pe_avg_of_avgs,
        present_price,
        present_eps,
        present_pe,
        high_five_years_ago,
        high_year,
        price_vs_high_pct,
        years_sold_as_high,
        high_years,
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
        // Sales 100, 110, …, 150 over 2021–2026 (six years, spec §7 AC1): recent pair 2026/2025,
        // old pair 2022/2021 (the fifth and sixth years, counting 2026 as the first), span 5.
        let years: Vec<CanonicalYear> = (0..6)
            .map(|i| {
                let s = 100 + 10 * i;
                year(2021 + i, &s.to_string(), "1", "10", "5")
            })
            .collect();
        let out = quick_screen(&years, Some(d("12")), Some(d("1")));
        let l = &out.sales;
        assert_eq!(
            (l.recent_year, l.recent_prior_year),
            (Some(2026), Some(2025))
        );
        assert_eq!((l.old_year, l.old_prior_year), (Some(2022), Some(2021)));
        assert_eq!(l.recent_avg, Some(d("145")));
        assert_eq!(l.old_avg, Some(d("105")));
        assert_eq!(l.increase, Some(d("40")));
        assert_eq!(l.increase_pct.map(|p| p.round_dp(2)), Some(d("38.10")));
        assert_eq!(l.span_years, 5);
        // (145/105)^(1/5) − 1 = 6.7 %
        assert_eq!(l.compound_rate_pct.map(|p| p.round_dp(1)), Some(d("6.7")));
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
        assert_eq!(l.span_years, 3, "a four-year window: 2026 − 2023");
        // Three years: the middle one would serve both pairs (G1 review) — unavailable.
        let three: Vec<CanonicalYear> = years[1..].to_vec();
        assert!(quick_screen(&three, None, None).sales.unavailable);
        assert_eq!(
            quick_screen(&three, None, None).sales.compound_rate_pct,
            None
        );
        let two: Vec<CanonicalYear> = years[..2].to_vec();
        assert!(quick_screen(&two, None, None).sales.unavailable);
    }

    #[test]
    fn the_pairs_are_keyed_by_year_inside_the_six_year_window() {
        // Twelve years with 2021 missing: the form's old pair (2022/2021) is broken, so the
        // oldest consecutive pair inside the window is taken (2023/2022, span 4) — never a pair
        // from before the window, never two years across the gap.
        let years: Vec<CanonicalYear> = (2015..=2026)
            .filter(|y| *y != 2021)
            .map(|y| year(y, "100", "1", "10", "5"))
            .collect();
        let l = quick_screen(&years, None, None).sales;
        assert_eq!((l.old_year, l.old_prior_year), (Some(2023), Some(2022)));
        assert_eq!(l.span_years, 4);
        // The latest year without its predecessor: the recent pair is the latest CONSECUTIVE one.
        let years: Vec<CanonicalYear> = [2019, 2020, 2021, 2022, 2023, 2024, 2026]
            .into_iter()
            .map(|y| year(y, "100", "1", "10", "5"))
            .collect();
        let l = quick_screen(&years, None, None).sales;
        assert_eq!(
            (l.recent_year, l.recent_prior_year),
            (Some(2024), Some(2023))
        );
        assert_eq!((l.old_year, l.old_prior_year), (Some(2020), Some(2019)));
        assert_eq!(l.span_years, 5);
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
        // A zero price yields no P/E either — never a « PER de 0 » averaged in.
        let mut zero = years.clone();
        zero[5].low_price = Some(d("0"));
        let out = quick_screen(&zero, Some(d("0")), Some(d("5")));
        assert_eq!(out.price.rows[4].pe_low, None);
        assert_eq!(out.price.present_pe, None);
        assert_eq!(out.price.pe_years, 4);
    }

    /// Six years whose two-year averages are `old` (2021–2022) and `recent` (2025–2026).
    fn six_years_with_averages(old: &str, recent: &str) -> Vec<CanonicalYear> {
        [
            (2021, old),
            (2022, old),
            (2023, "1"),
            (2024, "1"),
            (2025, recent),
            (2026, recent),
        ]
        .into_iter()
        .map(|(y, s)| year(y, s, s, "10", "5"))
        .collect()
    }

    /// Spec §6: the form's conversion table reproduced END TO END — six years through
    /// `quick_screen`, the real ladder, the real rate (not the root helper alone).
    #[test]
    fn quick_screen_reproduces_the_forms_conversion_table() {
        let rate = |recent: &str| {
            let out = quick_screen(&six_years_with_averages("100", recent), None, None);
            assert_eq!(out.sales.span_years, FORM_SPAN_YEARS);
            out.sales.compound_rate_pct.unwrap()
        };
        // The table's own precision (whole percents): 27 % → 5 %, 271 % → 30 %.
        assert_eq!(rate("127").round_dp(0), d("5"));
        assert_eq!(rate("371").round_dp(0), d("30"));
        // The screen's precision (one decimal): 27,6 % → 5,0 %, 271,3 % → 30,0 %.
        assert_eq!(rate("127.6").round_dp(1), d("5.0"));
        assert_eq!(rate("371.3").round_dp(1), d("30.0"));
        let out = quick_screen(&six_years_with_averages("100", "127"), None, None);
        assert_eq!(out.sales.increase_pct, Some(d("27")));
    }

    #[test]
    fn a_non_positive_old_average_has_no_percentage_increase() {
        // EPS −1 → +1: a recovery, never « −200 % » (G1 review).
        let l = quick_screen(&six_years_with_averages("-1", "1"), None, None).eps;
        assert_eq!(l.increase, Some(d("2")));
        assert_eq!(l.increase_pct, None);
        assert_eq!(l.compound_rate_pct, None);
        let l = quick_screen(&six_years_with_averages("0", "1"), None, None).eps;
        assert_eq!(l.increase_pct, None);
    }

    #[test]
    fn the_pe_figures_cover_the_same_rows_and_say_how_many() {
        let mut years = vec![
            year(2022, "1", "2", "50", "30"),  // 25 / 15
            year(2023, "1", "4", "80", "40"),  // 20 / 10
            year(2024, "1", "4", "100", "60"), // 25 / 15
            year(2025, "1", "5", "100", "50"), // 20 / 10
            year(2026, "1", "5", "120", "60"), // 24 / 12
        ];
        let p = quick_screen(&years, Some(d("110")), Some(d("5"))).price;
        assert!(p.five_year_record);
        assert_eq!(p.pe_years, 5);
        assert_eq!(p.high_year, Some(2022));
        assert_eq!(p.high_years, 5);
        // 2024 loses its low price: its high P/E must not enter the high total alone.
        years[2].low_price = None;
        let p = quick_screen(&years, Some(d("110")), Some(d("5"))).price;
        assert_eq!(p.pe_years, 4);
        assert_eq!(p.pe_high_total, Some(d("89")));
        assert_eq!(p.pe_low_total, Some(d("47")));
        assert_eq!(p.pe_high_avg, Some(d("22.25")));
        assert_eq!(p.pe_low_avg, Some(d("11.75")));
        // A non-positive EPS everywhere: no P/E row → every P/E figure absent, count 0.
        for y in &mut years {
            y.eps = Some(d("-1"));
        }
        let p = quick_screen(&years, Some(d("110")), None).price;
        assert_eq!(p.pe_years, 0);
        assert_eq!(
            (p.pe_high_total, p.pe_high_avg, p.pe_avg_of_avgs),
            (None, None, None)
        );
    }

    #[test]
    fn an_overflowing_pe_total_is_absent_never_the_last_value() {
        let big = Decimal::MAX.to_string();
        let years = vec![
            year(2025, "1", "1", &big, "1"),
            year(2026, "1", "1", &big, "1"),
        ];
        let p = quick_screen(&years, None, None).price;
        assert_eq!(p.pe_years, 2);
        assert_eq!(p.pe_high_total, None);
        assert_eq!(p.pe_high_avg, None);
        assert_eq!(p.pe_avg_of_avgs, None);
        assert_eq!(p.pe_low_total, Some(d("2")));
    }

    #[test]
    fn a_short_record_claims_no_five_years_and_no_high_counts_as_absent() {
        let years = vec![
            year(2024, "1", "2", "50", "30"),
            year(2025, "1", "4", "80", "40"),
            year(2026, "1", "4", "100", "60"),
        ];
        let p = quick_screen(&years, Some(d("90")), Some(d("4"))).price;
        assert!(!p.five_year_record);
        assert_eq!(p.high_year, Some(2024));
        assert_eq!((p.years_sold_as_high, p.high_years), (Some(1), 3));
        // Five rows that are not consecutive are not the form's five years either.
        let gap: Vec<CanonicalYear> = [2019, 2022, 2023, 2024, 2026]
            .into_iter()
            .map(|y| year(y, "1", "1", "10", "5"))
            .collect();
        assert!(!quick_screen(&gap, None, None).price.five_year_record);
        // Rows without a high: « sold as high in 0 of … » would be a lie — absent.
        let mut no_high = years.clone();
        for y in &mut no_high {
            y.high_price = None;
        }
        let p = quick_screen(&no_high, Some(d("90")), None).price;
        assert_eq!(p.years_sold_as_high, None);
        assert_eq!(p.high_years, 0);
        assert_eq!(p.high_year, None);
        // No price rows at all: absent, never 0.
        assert_eq!(
            quick_screen(&[], Some(d("90")), None)
                .price
                .years_sold_as_high,
            None
        );
    }
}
