//! Raw → canonical financial normalization — "Murat lever #1" (Story 1.7).
//!
//! [`normalize`] is the **one** place raw financial inputs become canonical: declared splits are
//! rebased onto the per-share series, IFRS ↔ US-GAAP representational differences collapse into
//! the same canonical year, and the three input-shape plausibility findings of spec §3
//! (`split_series_break`, `currency_mismatch`, `fiscal_period_misalignment`) are raised — flag
//! only, never auto-corrected. Both manual entry (Epic 2) and providers (Epic 3) reuse this
//! single function; the raw ↔ `contract` mapping is the caller's job (see [`types`] docs).
//!
//! Pure calculation (Cardinal Rule): no I/O / UI / SQL / network, all math in exact
//! [`rust_decimal::Decimal`] (never floats), **no rounding** (display-only,
//! [`crate::rounding`]), and **no panic on any input** — every division is checked, and
//! degenerate inputs become typed `unknown/insufficient` states, never a silent 0.

mod checks;
mod gaap;
mod splits;
mod types;

pub use types::{
    CanonicalFinancials, CanonicalYear, Finding, NormalizeError, PlausibilityKey, RawAmount,
    RawFinancials, RawYear, SplitEvent, YearUsability,
};

/// Normalize raw financial inputs into [`CanonicalFinancials`].
///
/// Deterministic: identical input ⇒ identical output, regardless of the order of `years` and
/// `splits` (years are sorted ascending; the cumulative split factor is order-independent;
/// findings are emitted in a fixed pass order). Structural input errors (duplicate years, a
/// zero split ratio) return a typed [`NormalizeError`] — they are malformed inputs, not
/// plausibility findings (the pinned `PLAUSIBILITY_RULES` catalog must not grow here).
pub fn normalize(raw: RawFinancials) -> Result<CanonicalFinancials, NormalizeError> {
    for split in &raw.splits {
        if split.numerator == 0 || split.denominator == 0 {
            return Err(NormalizeError::InvalidSplitRatio {
                effective_year: split.effective_year,
            });
        }
    }

    let mut years = raw.years;
    years.sort_by_key(|y| y.year);
    for pair in years.windows(2) {
        if pair[0].year == pair[1].year {
            return Err(NormalizeError::DuplicateYear { year: pair[0].year });
        }
    }

    let mut findings = Vec::new();
    checks::currency_findings(&years, &raw.native_currency, &mut findings);
    checks::fiscal_findings(&years, &mut findings);

    let canonical_years: Vec<CanonicalYear> = years
        .iter()
        .map(|year| canonical_year(year, &raw.splits))
        .collect();

    splits::detect_breaks(&canonical_years, &mut findings);

    let usable = canonical_years
        .iter()
        .filter(|y| matches!(y.usability, YearUsability::Usable))
        .count();
    let usable_years = u32::try_from(usable).unwrap_or(u32::MAX);

    Ok(CanonicalFinancials {
        years: canonical_years,
        findings,
        usable_years,
    })
}

/// Build one canonical year: rebase the per-share series by the declared splits (aggregates —
/// `sales`, PTP — are never split-adjusted), canonicalize PTP (§2 gross-up when needed), then
/// derive the usability state from the **canonical** presence of the load-bearing fields.
fn canonical_year(raw: &RawYear, splits: &[SplitEvent]) -> CanonicalYear {
    let ratio = splits::cumulative_ratio(raw.year, splits);
    let value = |a: &Option<RawAmount>| a.as_ref().map(|x| x.value);
    let rebased = |a: &Option<RawAmount>| splits::rebase_per_share(value(a), ratio);

    let mut year = CanonicalYear {
        year: raw.year,
        sales: value(&raw.sales),
        eps: rebased(&raw.eps),
        high_price: rebased(&raw.high_price),
        low_price: rebased(&raw.low_price),
        dividend_per_share: rebased(&raw.dividend_per_share),
        pre_tax_profit: gaap::canonical_pre_tax_profit(
            value(&raw.pre_tax_profit),
            value(&raw.net_profit),
            raw.tax_rate,
        ),
        book_value_per_share: rebased(&raw.book_value_per_share),
        usability: YearUsability::Usable, // provisional — fixed from canonical presence below
    };
    let missing = types::missing_load_bearing(&year);
    if !missing.is_empty() {
        year.usability = YearUsability::Insufficient { missing };
    }
    year
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn d(mantissa: i64, scale: u32) -> Decimal {
        Decimal::new(mantissa, scale)
    }

    fn amt(value: Decimal) -> Option<RawAmount> {
        Some(RawAmount {
            value,
            currency: "USD".to_string(),
        })
    }

    fn usable_year(y: i32) -> RawYear {
        let mut year = RawYear::empty(y);
        year.sales = amt(d(1000, 0));
        year.eps = amt(d(100, 2));
        year.high_price = amt(d(2000, 2));
        year.low_price = amt(d(1000, 2));
        year
    }

    #[test]
    fn years_are_sorted_ascending_regardless_of_input_order() {
        let raw = RawFinancials {
            native_currency: "USD".to_string(),
            years: vec![usable_year(2023), usable_year(2021), usable_year(2022)],
            splits: vec![],
        };
        let out = normalize(raw).expect("valid input");
        let order: Vec<i32> = out.years.iter().map(|y| y.year).collect();
        assert_eq!(order, vec![2021, 2022, 2023]);
    }

    #[test]
    fn duplicate_years_are_a_structural_error() {
        let raw = RawFinancials {
            native_currency: "USD".to_string(),
            years: vec![usable_year(2022), usable_year(2022)],
            splits: vec![],
        };
        assert_eq!(
            normalize(raw),
            Err(NormalizeError::DuplicateYear { year: 2022 })
        );
    }

    #[test]
    fn zero_split_ratio_is_a_structural_error() {
        let raw = RawFinancials {
            native_currency: "USD".to_string(),
            years: vec![usable_year(2022)],
            splits: vec![SplitEvent {
                effective_year: 2021,
                numerator: 0,
                denominator: 1,
            }],
        };
        assert_eq!(
            normalize(raw),
            Err(NormalizeError::InvalidSplitRatio {
                effective_year: 2021
            })
        );
    }

    #[test]
    fn missing_load_bearing_field_marks_year_insufficient_with_names() {
        let mut year = usable_year(2024);
        year.eps = None;
        year.low_price = None;
        let raw = RawFinancials {
            native_currency: "USD".to_string(),
            years: vec![year, usable_year(2023)],
            splits: vec![],
        };
        let out = normalize(raw).expect("valid input");
        assert_eq!(out.usable_years, 1, "only 2023 is usable");
        assert_eq!(
            out.years[1].usability,
            YearUsability::Insufficient {
                missing: vec!["eps", "low_price"],
            }
        );
        assert_eq!(out.years[1].eps, None, "missing eps stays None, never 0");
        assert_eq!(out.years[0].usability, YearUsability::Usable);
    }

    #[test]
    fn sales_are_never_split_adjusted_and_per_share_fields_are() {
        let mut y2020 = usable_year(2020);
        y2020.eps = amt(d(900, 2)); // 9.00 pre-split
        y2020.high_price = amt(d(9000, 2)); // 90.00
        y2020.low_price = amt(d(3000, 2)); // 30.00
        y2020.dividend_per_share = amt(d(300, 2)); // 3.00
        y2020.book_value_per_share = amt(d(600, 2)); // 6.00
        let mut y2021 = usable_year(2021);
        y2021.eps = amt(d(310, 2)); // post-split scale
        let raw = RawFinancials {
            native_currency: "USD".to_string(),
            years: vec![y2020, y2021],
            splits: vec![SplitEvent {
                effective_year: 2021,
                numerator: 3,
                denominator: 1,
            }],
        };
        let out = normalize(raw).expect("valid input");
        let y = &out.years[0];
        assert_eq!(y.sales, Some(d(1000, 0)), "sales must NOT be rebased");
        assert_eq!(y.eps, Some(d(300, 2)), "9.00 / 3 = 3.00 exactly");
        assert_eq!(y.high_price, Some(d(3000, 2)));
        assert_eq!(y.low_price, Some(d(1000, 2)));
        assert_eq!(y.dividend_per_share, Some(d(100, 2)));
        assert_eq!(y.book_value_per_share, Some(d(200, 2)));
        // Post-split year untouched.
        assert_eq!(out.years[1].eps, Some(d(310, 2)));
    }

    #[test]
    fn declared_split_is_not_flagged_but_undeclared_jump_is() {
        // 2020 is reported in PRE-split shares (×3 of 2021's post-split scale across the whole
        // per-share series); 2021 in post-split shares.
        let mut pre = usable_year(2020);
        pre.eps = amt(d(900, 2)); // 9.00
        pre.high_price = amt(d(6000, 2)); // 60.00
        pre.low_price = amt(d(3000, 2)); // 30.00
        let mut post = usable_year(2021);
        post.eps = amt(d(300, 2)); // 3.00 — a 1/3 jump if unexplained

        let declared = RawFinancials {
            native_currency: "USD".to_string(),
            years: vec![pre.clone(), post.clone()],
            splits: vec![SplitEvent {
                effective_year: 2021,
                numerator: 3,
                denominator: 1,
            }],
        };
        let out = normalize(declared).expect("valid input");
        assert!(
            out.findings.is_empty(),
            "a correctly declared split must not trip the detector: {:?}",
            out.findings
        );

        let undeclared = RawFinancials {
            native_currency: "USD".to_string(),
            years: vec![pre, post],
            splits: vec![],
        };
        let out = normalize(undeclared).expect("valid input");
        let break_at = |context: &'static str| Finding {
            key: PlausibilityKey::SplitSeriesBreak,
            year: 2021,
            context,
        };
        assert_eq!(
            out.findings,
            vec![
                break_at("eps"),
                break_at("high_price"),
                break_at("low_price"),
            ],
            "the same jump undeclared must be flagged on every jumping per-share field"
        );
    }

    #[test]
    fn currency_mismatch_value_passes_through_unconverted() {
        let mut year = usable_year(2024);
        year.eps = Some(RawAmount {
            value: d(500, 2),
            currency: "EUR".to_string(),
        });
        let raw = RawFinancials {
            native_currency: "USD".to_string(),
            years: vec![year],
            splits: vec![],
        };
        let out = normalize(raw).expect("valid input");
        assert_eq!(
            out.findings,
            vec![Finding {
                key: PlausibilityKey::CurrencyMismatch,
                year: 2024,
                context: "eps",
            }]
        );
        assert_eq!(
            out.years[0].eps,
            Some(d(500, 2)),
            "the EUR value must pass through UNCONVERTED"
        );
        assert_eq!(out.usable_years, 1, "a flagged year still passes through");
    }

    #[test]
    fn gaap_representations_collapse_to_the_same_canonical_year() {
        let mut direct = usable_year(2024);
        direct.pre_tax_profit = amt(d(100, 0));
        let mut grossed = usable_year(2024);
        grossed.net_profit = amt(d(70, 0));
        grossed.tax_rate = Some(d(30, 2));

        let mk = |year: RawYear| RawFinancials {
            native_currency: "USD".to_string(),
            years: vec![year],
            splits: vec![],
        };
        let a = normalize(mk(direct)).expect("valid input");
        let b = normalize(mk(grossed)).expect("valid input");
        assert_eq!(
            a, b,
            "direct PTP and net+tax_rate represent the same economics"
        );
        assert_eq!(a.years[0].pre_tax_profit, Some(d(100, 0)));
    }
}
