//! Epic 3 seam-verification spike (retrospective action B3) — the **headless** half.
//!
//! Epic 2 built three provider/reconciliation seams but only exercised them via test fixtures:
//! (1) the **stale murmur** (`Freshness::Stale` → the dimmed `◦`), (2) the **divergent-edit
//! `✓→?` demotion** (`Cell::edited`), (3) **verdict degradation** on a stale load-bearing input.
//! Before Epic 3's reconciliation stories build on them, this spike drives all three through the
//! **real** rails (contract `Cell::edited`, the engine `build_frame`/gate mapping, the form
//! adapter) and asserts they fire correctly — no real provider needed. The remaining GO/NO-GO is
//! the perceptual on-display check, recorded in `docs/spikes/spike-d-stale-reconcile.md`.
//!
//! Spike finding for Story 3.3: `state::JournalState::mutate_cell` hardcodes `manual_provenance()`,
//! so there is **no provider-provenance path** today — a refresh rail must thread a
//! `Source::Provider` provenance through (the contract primitive `Cell::edited` already accepts it,
//! proven below). This is a kept regression guard, not throwaway code.

#[cfg(test)]
mod tests {
    use crate::viewmodel::engine::{self, PlausibilityWarnings};
    use crate::viewmodel::form;
    use crate::viewmodel::format::NumberFormat;
    use rust_decimal::Decimal;
    use slint::Model;
    use steadyinvest_contract::{
        Cell, Coverage, Freshness, Judgment, Money, Provenance, Review, Source, Study, Timestamp,
        YearData,
    };
    use steadyinvest_core::verdict::{GateState, Verdict};

    fn money(s: &str) -> Money {
        Money::from(Decimal::from_str_exact(s).expect("decimal literal"))
    }

    fn provenance(source: Source) -> Provenance {
        Provenance {
            source,
            logical_version: 1,
            timestamp: Timestamp("2026-06-15T00:00:00Z".to_string()),
            hash_of_dependencies: "spike".to_string(),
        }
    }

    fn validated_cell(value: &str) -> Cell {
        Cell {
            value: Some(money(value)),
            source: Source::Manual,
            freshness: Freshness::Current,
            review: Review::Validated,
            coverage: Coverage::Present,
            provenance: provenance(Source::Manual),
            pending: None,
        }
    }

    /// A study that derives a `Full` verdict through the real engine (mirrors the core
    /// `all_green_study` shape: five validated years + a complete judgment).
    fn green_study() -> Study {
        let mut study: Study = serde_json::from_value(serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "journal_id": "00000000-0000-0000-0000-000000000002",
            "security_ticker": "SPIKE",
            "native_currency": "USD",
            "years": [],
            "judgment": { "forecast_low_option": "avg_low_pe_times_eps" },
            "created_at": "2026-06-12T00:00:00Z",
            "schema_version": 1
        }))
        .expect("study skeleton deserializes");
        study.years = (0u32..5)
            .map(|i| {
                let step = Decimal::from(i);
                YearData {
                    year: 2021 + i as i32,
                    sales: validated_cell(
                        &(Decimal::from(100) + step * Decimal::from(10)).to_string(),
                    ),
                    eps: validated_cell(
                        &(money("1.00").as_decimal() + step * money("0.10").as_decimal())
                            .to_string(),
                    ),
                    high_price: validated_cell(
                        &(Decimal::from(20) + step * Decimal::from(2)).to_string(),
                    ),
                    low_price: validated_cell(&(Decimal::from(10) + step).to_string()),
                    dividend_per_share: None,
                    pre_tax_profit: None,
                    book_value_per_share: None,
                }
            })
            .collect();
        study.judgment = Judgment {
            estimated_high_eps: Some(money("2.00")),
            estimated_low_eps: Some(money("1.50")),
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: Some(money("20")),
            judged_avg_low_pe: Some(money("10")),
            forecast_low_option: study.judgment.forecast_low_option,
            recent_severe_low: None,
            current_price: Some(money("25")),
            present_full_year_dividend: None,
            ttm_eps: None,
        };
        study
    }

    /// SEAM 2 — the reconciliation rail. A provider re-fetch with a **divergent** value over a
    /// `✓ validated` cell demotes it to `? to-review` and stamps `Source::Provider`; a re-fetch of
    /// the **same** value keeps `✓` (Epic 3's "divergence → auto-?" rule, on the existing primitive).
    #[test]
    fn seam2_provider_divergent_edit_demotes_validated_keeps_unchanged() {
        let validated = validated_cell("100");
        assert_eq!(validated.review, Review::Validated);

        let diverged = validated.edited(Some(money("110")), provenance(Source::Provider));
        assert_eq!(
            diverged.review,
            Review::ToReview,
            "divergent provider value must auto-?"
        );
        assert_eq!(
            diverged.source,
            Source::Provider,
            "provenance source flips to provider"
        );
        assert_eq!(
            diverged.freshness,
            Freshness::Current,
            "a fresh fetch is current"
        );

        let unchanged = validated.edited(Some(money("100")), provenance(Source::Provider));
        assert_eq!(
            unchanged.review,
            Review::Validated,
            "a non-divergent re-fetch keeps the human ✓ (value equality, not byte equality)"
        );
    }

    /// SEAM 3 — verdict integrity. A green study derives `Full`; flipping one load-bearing input to
    /// `Stale` degrades it to `Provisional` and names the gate `Stale` — through the real engine.
    #[test]
    fn seam3_stale_load_bearing_input_degrades_full_to_provisional() {
        let mut study = green_study();
        let baseline = engine::build_frame(&study).expect("green study normalizes");
        assert!(
            matches!(baseline.snapshot.verdict(), Verdict::Full(_)),
            "baseline is Full before any stale input"
        );

        study.years[4].high_price.freshness = Freshness::Stale;
        assert_eq!(
            steadyinvest_report::form::cell_to_gate_state(Some(&study.years[4].high_price)),
            GateState::Stale,
            "a validated-but-stale cell maps to the Stale gate"
        );

        let degraded = engine::build_frame(&study).expect("study still normalizes");
        assert!(
            matches!(degraded.snapshot.verdict(), Verdict::Provisional(_)),
            "one stale load-bearing input degrades Full → Provisional"
        );
    }

    /// SEAM 1 — the stale murmur. A present-but-stale cell surfaces as `GridCellState.stale == true`
    /// through the real form adapter (the bool the `◦` marker + 60% dimming bind to). The
    /// *perceptual* render is the on-display GO/NO-GO (see the spike doc).
    #[test]
    fn seam1_stale_cell_surfaces_in_the_form_grid() {
        let mut study = green_study();
        study.years[4].sales.freshness = Freshness::Stale;

        let rows = form::mgmt_rows(
            &study,
            NumberFormat::Comma,
            &PlausibilityWarnings::default(),
        );
        // mgmt_rows row 0 is the sales row; columns are oldest→newest, so the 2025 cell is last.
        let sales_row = &rows[0];
        let newest = sales_row
            .cells
            .row_data(sales_row.cells.row_count() - 1)
            .expect("newest cell");
        assert!(
            newest.stale,
            "a stale sales cell drives GridCellState.stale (the murmur)"
        );

        // A current sibling stays silent — the murmur is per-cell, not global.
        let older = sales_row.cells.row_data(0).expect("oldest cell");
        assert!(!older.stale, "a current cell shows no murmur");
    }
}
