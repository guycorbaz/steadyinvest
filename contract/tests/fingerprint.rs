//! Story 8.2b AC 5 — the draft fingerprint (arch A7, encoding `fp1`): what marks a draft stale and
//! what must not.

use rust_decimal::Decimal;
use steadyinvest_contract::{
    AiPlaced, Cell, Coverage, DraftTarget, ForecastLowOption, Freshness, Judgment, Money,
    PendingProvider, Provenance, Review, Source, Study, Timestamp, YearData, draft_fingerprint,
};
use uuid::Uuid;

const MV: &str = "ssg-1.2.0";

fn m(s: &str) -> Money {
    Money::from(Decimal::from_str_exact(s).unwrap())
}

fn prov(source: Source, ts: &str, hash: &str) -> Provenance {
    Provenance {
        source,
        logical_version: 3,
        timestamp: Timestamp(ts.into()),
        hash_of_dependencies: hash.into(),
        ai_origin: None,
    }
}

fn cell(v: &str) -> Cell {
    Cell {
        value: Some(m(v)),
        source: Source::Provider,
        freshness: Freshness::Current,
        review: Review::None,
        coverage: Coverage::Present,
        provenance: prov(Source::Provider, "2026-09-01T00:00:00Z", "h1"),
        pending: None,
    }
}

fn year(y: i32, eps: &str) -> YearData {
    YearData {
        year: y,
        sales: cell("1000"),
        eps: cell(eps),
        high_price: cell("50"),
        low_price: cell("30"),
        dividend_per_share: None,
        pre_tax_profit: None,
        book_value_per_share: None,
    }
}

fn study() -> Study {
    let judgment = Judgment {
        estimated_high_eps: Some(m("5")),
        estimated_low_eps: Some(m("4")),
        projected_sales_growth_pct: None,
        projected_eps_growth_pct: Some(m("12.5")),
        judged_avg_high_pe: Some(m("20")),
        judged_avg_low_pe: Some(m("12")),
        forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
        recent_severe_low: None,
        current_price: Some(m("100")),
        ttm_eps: Some(m("3")),
        present_full_year_dividend: None,
        ai_placed: AiPlaced::default(),
    };
    let mut s = Study::new(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        "ACME",
        "USD",
        judgment,
        Timestamp("2026-09-27T09:00:00Z".into()),
    );
    s.years = vec![year(2023, "2"), year(2024, "2.5")];
    s
}

fn eps_2024() -> DraftTarget {
    DraftTarget::Cell {
        fiscal_year: 2024,
        field: "eps".into(),
    }
}

fn low_pe() -> DraftTarget {
    DraftTarget::Judgment {
        field: "judged_avg_low_pe".into(),
    }
}

fn fp(s: &Study, t: &DraftTarget) -> String {
    draft_fingerprint(s, t, MV).expect("target present")
}

#[test]
fn the_fingerprint_is_versioned_and_deterministic() {
    let s = study();
    let a = fp(&s, &eps_2024());
    assert!(a.starts_with("fp1:"), "{a}");
    assert_eq!(a.len(), 4 + 64);
    assert_eq!(a, fp(&s.clone(), &eps_2024()));
    assert_ne!(a, fp(&s, &low_pe()));
}

#[test]
fn a_refresh_of_the_eps_history_marks_a_judgment_draft_stale() {
    let s = study();
    let mut refreshed = s.clone();
    refreshed.years[0].eps.value = Some(m("2.2"));
    assert_ne!(fp(&s, &low_pe()), fp(&refreshed, &low_pe()));
}

#[test]
fn a_method_version_change_marks_a_judgment_draft_stale() {
    let s = study();
    assert_ne!(
        draft_fingerprint(&s, &low_pe(), "ssg-1.2.0"),
        draft_fingerprint(&s, &low_pe(), "ssg-1.3.0")
    );
}

#[test]
fn a_price_refresh_marks_a_judgment_draft_stale() {
    // Documented default (story question 2): the current price is part of the judgment context.
    let s = study();
    let mut priced = s.clone();
    priced.judgment.current_price = Some(m("101"));
    assert_ne!(fp(&s, &low_pe()), fp(&priced, &low_pe()));
}

#[test]
fn an_owner_edit_of_the_judged_field_marks_it_stale() {
    let s = study();
    let mut edited = s.clone();
    edited.judgment.judged_avg_low_pe = Some(m("11"));
    assert_ne!(fp(&s, &low_pe()), fp(&edited, &low_pe()));
}

#[test]
fn a_parked_divergent_provider_value_marks_a_cell_draft_stale() {
    let s = study();
    let mut parked = s.clone();
    parked.years[1].eps.pending = Some(PendingProvider {
        value: Some(m("2.7")),
        provenance: prov(Source::Provider, "2026-09-20T00:00:00Z", "h9"),
    });
    assert_ne!(fp(&s, &eps_2024()), fp(&parked, &eps_2024()));
    // A pending that reports "absent" differs from no pending too.
    let mut absent = s.clone();
    absent.years[1].eps.pending = Some(PendingProvider {
        value: None,
        provenance: prov(Source::Provider, "2026-09-20T00:00:00Z", "h9"),
    });
    assert_ne!(fp(&s, &eps_2024()), fp(&absent, &eps_2024()));
    assert_ne!(fp(&parked, &eps_2024()), fp(&absent, &eps_2024()));
}

#[test]
fn a_value_identical_restamp_does_not_mark_anything_stale() {
    // `restamp_if_predated` re-stamps provenance (timestamp, digest, logical version) on
    // value-identical cells and pendings; freshness and review may move too — none is encoded.
    let s = study();
    let mut restamped = s.clone();
    for y in &mut restamped.years {
        for c in [
            &mut y.sales,
            &mut y.eps,
            &mut y.high_price,
            &mut y.low_price,
        ] {
            c.provenance = prov(Source::Provider, "2026-09-27T12:00:00Z", "other");
            c.provenance.logical_version = 99;
            c.freshness = Freshness::Stale;
            c.review = Review::Validated;
        }
    }
    assert_eq!(fp(&s, &eps_2024()), fp(&restamped, &eps_2024()));
    assert_eq!(fp(&s, &low_pe()), fp(&restamped, &low_pe()));
}

#[test]
fn a_money_scale_change_does_not_mark_anything_stale() {
    let s = study();
    let mut scaled = s.clone();
    scaled.years[1].eps.value = Some(m("2.50"));
    scaled.judgment.judged_avg_low_pe = Some(m("12.0"));
    scaled.judgment.current_price = Some(m("100.00"));
    assert_eq!(fp(&s, &eps_2024()), fp(&scaled, &eps_2024()));
    assert_eq!(fp(&s, &low_pe()), fp(&scaled, &low_pe()));
}

#[test]
fn a_coverage_change_marks_a_cell_draft_stale() {
    // Documented default (story question 4).
    let s = study();
    let mut accepted = s.clone();
    accepted.years[1].eps.value = None;
    accepted.years[1].eps.coverage = Coverage::ToFill;
    let mut na = accepted.clone();
    na.years[1].eps.coverage = Coverage::NotAvailableAccepted;
    assert_ne!(fp(&accepted, &eps_2024()), fp(&na, &eps_2024()));
}

#[test]
fn a_source_change_marks_a_cell_draft_stale() {
    let s = study();
    let mut manual = s.clone();
    manual.years[1].eps.source = Source::Manual;
    assert_ne!(fp(&s, &eps_2024()), fp(&manual, &eps_2024()));
}

#[test]
fn an_absent_optional_slot_differs_from_a_materialized_empty_cell() {
    let s = study();
    let t = DraftTarget::Cell {
        fiscal_year: 2024,
        field: "dividend_per_share".into(),
    };
    let absent = fp(&s, &t);
    let mut materialized = s.clone();
    let mut empty = cell("0");
    empty.value = None;
    empty.coverage = Coverage::ToFill;
    materialized.years[1].dividend_per_share = Some(empty);
    assert_ne!(absent, fp(&materialized, &t));
}

#[test]
fn a_missing_year_is_target_gone() {
    let s = study();
    let t = DraftTarget::Cell {
        fiscal_year: 2019,
        field: "eps".into(),
    };
    assert_eq!(draft_fingerprint(&s, &t, MV), None);
    let unknown = DraftTarget::Judgment {
        field: "current_price".into(),
    };
    assert_eq!(draft_fingerprint(&s, &unknown, MV), None);
}

#[test]
fn a_cell_draft_ignores_other_cells_and_the_judgment() {
    let s = study();
    let mut other = s.clone();
    other.years[0].eps.value = Some(m("9"));
    other.judgment.judged_avg_low_pe = Some(m("1"));
    other.judgment.current_price = Some(m("1"));
    assert_eq!(fp(&s, &eps_2024()), fp(&other, &eps_2024()));
}

#[test]
fn a_parked_pending_re_stamped_with_the_same_value_is_not_stale() {
    // G3 F6: a refresh re-parks the SAME divergent provider value with a new provenance — the
    // fingerprint encodes the pending's value only.
    let mut parked = study();
    parked.years[1].eps.pending = Some(PendingProvider {
        value: Some(m("2.7")),
        provenance: prov(Source::Provider, "2026-09-20T00:00:00Z", "h9"),
    });
    let mut restamped = parked.clone();
    restamped.years[1].eps.pending = Some(PendingProvider {
        value: Some(m("2.70")),
        provenance: prov(Source::Provider, "2026-09-27T12:00:00Z", "other"),
    });
    assert_eq!(fp(&parked, &eps_2024()), fp(&restamped, &eps_2024()));
}
