//! The provider fetch/refresh cell rail (Stories 3.1/3.3–3.6 — FR21/FR22/FR23/FR29): apply a
//! fetched [`FetchedFinancials`] to the study grid cell by cell — fill gaps, re-stamp changed
//! provider values (an equal re-fetch is a true no-op), reconcile a divergent **manual** cell
//! non-destructively (manual wins, the provider value is preserved alongside as pending), flag /
//! clear `Freshness::Stale` around an outage — and tally everything into a [`RefreshReport`] so the
//! notice can name the recompute cause (price / fundamentals) and the `✓ → ?` re-validation scope.
//! Also home to the provenance builders (manual + provider) the editing rails stamp cells with.

use rust_decimal::Decimal;
use steadyinvest_contract::{
    Cell, Coverage, Freshness, Money, Provenance, Review, Source, Study, YearData,
};
use steadyinvest_ingestion::{CanonicalYear, FetchedFinancials};
use uuid::Uuid;

use crate::viewmodel::refresh::RefreshCause;

use super::JournalState;

impl JournalState {
    /// Build the manual [`Provenance`] for an edit (Story 2.4): `source = Manual`, `timestamp` from
    /// the injected [`Clock`] (ADD15 — never a scattered wall clock). For a manually-entered **leaf**
    /// input there is no app-side per-cell version counter and no upstream dependency digest, so v1
    /// uses defensible sentinels — `logical_version = 1` and `hash_of_dependencies = "manual"` —
    /// recorded in the 2.4 interpretations issue; both earn real meaning in Epic 3 reconciliation.
    /// (`Provenance` performs no validation on these strings — `contract` module doc.)
    pub(crate) fn manual_provenance(&self) -> Provenance {
        Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: self.clock.now(),
            hash_of_dependencies: "manual".to_string(),
        }
    }

    /// Provenance for a provider-fetched leaf (Story 3.1): `Source::Provider`, the injected clock's
    /// timestamp, and the **real** dependency digest from the fetch (#21 — no longer the `"manual"`
    /// sentinel). `logical_version` stays the app-side sentinel `1` (there is no per-cell counter;
    /// the study-level bump on `put_study` records the act's timing).
    fn provider_provenance(&self, digest: String) -> Provenance {
        Provenance {
            source: Source::Provider,
            logical_version: 1,
            timestamp: self.clock.now(),
            hash_of_dependencies: digest,
        }
    }

    /// Apply a manual provider **refresh** (Story 3.3, FR21/FR29) — the single deliberate online
    /// action that re-fetches and recomputes. Subsumes the Story-3.1 first-fetch (an empty study
    /// builds its grid) and generalises it: an already-populated study now **updates** its
    /// provider/derived cells with the new value + timestamp, on top of filling former gaps.
    ///
    /// Per cell, branching on the **current** cell's source (see [`refresh_cell`]):
    /// - a **gap** (no value) is **filled** from the provider, whatever its skeleton source;
    /// - a present **`Source::Manual`** value is **skipped** — manual wins, never overwritten here
    ///   (non-destructive dual-value reconciliation of a divergent manual cell is Story 3.4);
    /// - a present **provider/derived** value is **re-stamped via [`Cell::edited`] only when the
    ///   value actually changed** — an equal re-fetch is a no-op (idempotency: no timestamp churn,
    ///   no phantom undo step, no `✓→?` demotion). A divergent value auto-demotes a `✓` provider
    ///   cell to `?` and degrades the dependent verdict in the same frame (the Epic-1 invariant 2b).
    ///
    /// Returns a [`RefreshReport`] (updated / filled counts + the classified [`RefreshCause`]) so the
    /// caller can state *why* it recomputed (price / input / FX). Routed through the atomic
    /// [`Self::mutate_study`] rail (one `put_study`, guards, undo-only-on-real-change). Provider cells
    /// are `Review::None`, so the verdict stays Provisional/Withheld until the user validates.
    pub fn apply_provider_refresh(
        &mut self,
        study_id: Uuid,
        fetched: &FetchedFinancials,
    ) -> Result<RefreshReport, String> {
        let provenance = self.provider_provenance(fetched.digest.clone());
        let years: Vec<CanonicalYear> = fetched.canonical.years.clone();
        // Story 4.4 (AC2/AC6): the latest `/eod` close is the present market price for the §4 zone.
        // `None` for a provider with no current price → `current_price` left untouched (pre-4.4 shape).
        let latest_price = fetched.latest_price;
        let report = std::cell::Cell::new(RefreshReport::default());
        let report_ref = &report;
        self.mutate_study(study_id, move |study| {
            // A successful refresh means the provider responded → the outage (Story 3.5) is over.
            // Clear the stale flag on EVERY provider cell up front, so cells this fetch does not
            // re-visit (an omitted optional field, a year outside the fetched set) also recover —
            // not just the ones whose value is re-confirmed below. A freshness-only recovery; it is
            // not counted in the report (no value moved), it just lets the verdict come back.
            for year in &mut study.years {
                for cell in year_cells_mut(year) {
                    if cell.source == Source::Provider && cell.freshness == Freshness::Stale {
                        cell.freshness = Freshness::Current;
                    }
                }
            }
            // A fresh (never-edited) study first gets empty to-fill provider rows, so the SAME
            // per-cell accounting path then fills + classifies them — one rail, one tally.
            if study.years.is_empty() {
                study.years = years
                    .iter()
                    .map(|cy| empty_provider_year(cy.year, &provenance))
                    .collect();
            }
            let mut acc = RefreshReport::default();
            for cy in &years {
                if let Some(yd) = study.years.iter_mut().find(|y| y.year == cy.year) {
                    acc = acc.merge(refresh_year(yd, cy, &provenance));
                }
            }
            // Story 4.4 (AC2/AC6): fill `current_price` from the latest close — a present *market
            // fact*, not a user-owned judgment (the forecast high/low EPS + P/E stay strictly manual,
            // FR33-safe). Written in the SAME mutation so the §4 zone recomputes in one undo step.
            // `mutate_study`'s `before != study` guard persists/records this even when no yearly cell
            // moved (a price-only refresh). `None` → unchanged.
            if let Some(price) = latest_price {
                study.judgment.current_price = Some(Money::from(price));
            }
            report_ref.set(acc);
        })?;
        // Story 5.1: cache the latest close into the price-history trajectory (confront's source).
        if let Some(price) = latest_price {
            if let Some(study) = self.get_study(study_id) {
                self.cache_close(&study.security_ticker, price);
            }
        }
        Ok(report.get())
    }

    /// Set a study's `current_price` from a price-only holdings refresh (Story 4.4 / issue #50): the
    /// latest `/eod` close, fetched WITHOUT `/fundamentals` (so it works on the free EODHD tier). A
    /// present **market fact** (not a user-owned judgment — FR33-safe; the forecast high/low EPS + P/E
    /// stay manual), written through the atomic [`Self::mutate_study`] rail so the §4 zone recomputes
    /// and it is one undo step. Unlike [`Self::apply_provider_refresh`], it touches ONLY
    /// `current_price` — never the yearly provider cells (the holding refresh is price-led).
    pub fn apply_holding_price(&mut self, study_id: Uuid, price: Decimal) -> Result<(), String> {
        self.mutate_study(study_id, move |study| {
            study.judgment.current_price = Some(Money::from(price));
        })?;
        // Story 5.1: cache the close into the price-history trajectory (confront's source). Keyed by
        // the study's ticker + today's date; idempotent (one point per ticker/day).
        if let Some(study) = self.get_study(study_id) {
            self.cache_close(&study.security_ticker, price);
        }
        Ok(())
    }

    /// Flag the open study's **provider-sourced** cells `Freshness::Stale` after a failed (or
    /// empty) refresh (Story 3.5, FR23/NFR-R1). Only the **freshness** axis moves — `value`, `source`,
    /// `review`, `coverage`, `provenance`, and any Story-3.4 `pending` are all retained (last-known
    /// values are never cleared). Manual/derived cells are untouched (the user owns manual data). The
    /// engine already degrades a validated-but-stale load-bearing input to `Verdict::Provisional`
    /// (Story 2.6 wiring), and the form already renders the dimmed `◦` murmur (Story 2.4) — this is
    /// the first production caller that SETS the flag. Returns the count flagged; idempotent (an
    /// already-stale cell is left untouched, so `mutate_study`'s `before != study` guard records no
    /// phantom undo step). Routed through the atomic [`Self::mutate_study`] rail.
    pub fn mark_provider_stale(&mut self, study_id: Uuid) -> Result<usize, String> {
        // Pre-check: if there is nothing to flag (no provider cells, or all already stale), return a
        // true no-op WITHOUT entering `mutate_study` — so a failed refresh on an already-stale study
        // (repeated offline retries), an empty study, or a manual-only study writes no journal
        // revision and bumps no `logical_version` (mirrors the Story-3.4 accept/keep guard; the
        // Synology-sync corruption risk makes avoidable writes worth suppressing).
        let candidates = self
            .get_study(study_id)
            .map(|s| count_provider_to_stale(&s))
            .unwrap_or(0);
        if candidates == 0 {
            return Ok(0);
        }
        let count = std::cell::Cell::new(0usize);
        let count_ref = &count;
        self.mutate_study(study_id, move |study| {
            let mut flagged = 0usize;
            for year in &mut study.years {
                for cell in year_cells_mut(year) {
                    if cell.source == Source::Provider && cell.freshness != Freshness::Stale {
                        cell.freshness = Freshness::Stale;
                        flagged += 1;
                    }
                }
            }
            count_ref.set(flagged);
        })?;
        Ok(count.get())
    }
}

// ── Provider fetch/refresh cell helpers (Story 3.1 / 3.3) ───────────────────────────────────────

/// The outcome of an [`JournalState::apply_provider_refresh`] (Story 3.3): how many cells were
/// **updated** (a present provider/derived value changed) vs **filled** (a former gap), and the
/// classified [`RefreshCause`] of the recompute (price / input / FX). `updated + filled == 0` means
/// an idempotent no-op (the study was already current). Merged across years.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RefreshReport {
    pub updated: usize,
    pub filled: usize,
    /// Manual cells whose divergent provider value was preserved alongside (Story 3.4).
    pub reconciled: usize,
    /// Cells this refresh reset `✓ → ?` — the re-validation scope of an annual update (Story 3.6).
    pub revalidate: usize,
    pub cause: RefreshCause,
}

impl RefreshReport {
    /// Accumulate another year's report into this one (sum counts, OR-merge the cause).
    fn merge(self, other: RefreshReport) -> RefreshReport {
        RefreshReport {
            updated: self.updated + other.updated,
            filled: self.filled + other.filled,
            reconciled: self.reconciled + other.reconciled,
            revalidate: self.revalidate + other.revalidate,
            cause: self.cause.merge(other.cause),
        }
    }

    /// Whether the refresh changed anything (filled a gap, updated a provider value, or reconciled a
    /// divergent manual cell — any of which can move the verdict).
    pub fn changed(self) -> bool {
        self.updated + self.filled + self.reconciled > 0
    }
}

/// Mutable refs to every present cell of a year — the 4 load-bearing cells plus any present optional
/// cell. The shared walk for the freshness rails (Story 3.5: flag/clear `Freshness::Stale`).
fn year_cells_mut(year: &mut YearData) -> Vec<&mut Cell> {
    let mut cells: Vec<&mut Cell> = vec![
        &mut year.sales,
        &mut year.eps,
        &mut year.high_price,
        &mut year.low_price,
    ];
    for slot in [
        &mut year.dividend_per_share,
        &mut year.pre_tax_profit,
        &mut year.book_value_per_share,
    ] {
        if let Some(cell) = slot.as_mut() {
            cells.push(cell);
        }
    }
    cells
}

/// How many provider cells of `study` are not yet `Stale` — the [`JournalState::mark_provider_stale`]
/// pre-check (a `&Study` read, no mutation), so a no-op failure writes no journal revision.
fn count_provider_to_stale(study: &Study) -> usize {
    study
        .years
        .iter()
        .flat_map(|y| {
            let req = [&y.sales, &y.eps, &y.high_price, &y.low_price];
            let opt = [
                y.dividend_per_share.as_ref(),
                y.pre_tax_profit.as_ref(),
                y.book_value_per_share.as_ref(),
            ];
            req.into_iter()
                .map(Some)
                .chain(opt)
                .flatten()
                .collect::<Vec<_>>()
        })
        .filter(|c| c.source == Source::Provider && c.freshness != Freshness::Stale)
        .count()
}

/// A provider-sourced cell: `Source::Provider`, `Freshness::Current`, `Review::None` (unvalidated),
/// `Coverage::Present` for a value / `ToFill` for a gap (absent stays hand-editable, never `0`).
fn provider_cell(value: Option<Decimal>, provenance: &Provenance) -> Cell {
    Cell {
        value: value.map(Money::from),
        source: Source::Provider,
        freshness: Freshness::Current,
        review: Review::None,
        coverage: if value.is_some() {
            Coverage::Present
        } else {
            Coverage::ToFill
        },
        provenance: provenance.clone(),
        // A fresh provider cell carries no pending divergence (it IS the provider value).
        pending: None,
    }
}

/// Build an empty (all to-fill) provider year row — the fresh-study seed the [`refresh_cell`] rail
/// then fills, so one accounting path covers both the first fetch and a later refresh.
fn empty_provider_year(year: i32, provenance: &Provenance) -> YearData {
    YearData {
        year,
        sales: provider_cell(None, provenance),
        eps: provider_cell(None, provenance),
        high_price: provider_cell(None, provenance),
        low_price: provider_cell(None, provenance),
        dividend_per_share: None,
        pre_tax_profit: None,
        book_value_per_share: None,
    }
}

/// What a single cell's refresh did — drives the per-year tally + cause classification.
enum CellRefresh {
    /// A cell left untouched (a `NotAvailableAccepted` decision — never refilled or reconciled).
    Skipped,
    /// No change (an equal re-fetch, or the provider has no value for this cell).
    Unchanged,
    /// A former gap was filled from the provider.
    Filled,
    /// A present provider/derived value changed and was re-stamped.
    Updated,
    /// A present **manual** value diverged from the provider: the manual value stands, the divergent
    /// provider value is preserved alongside (pending), and a `✓` demoted (Story 3.4, FR22).
    Reconciled,
}

/// Refresh one **required** load-bearing cell. Returns `(outcome, demoted)` where `demoted` is `true`
/// iff this refresh reset the cell's `Review::Validated → ToReview` (Story 3.6: the count of cells the
/// user must re-verify after an annual update). The demotion itself is the existing
/// `Cell::edited`/`reconcile` rule — this wrapper only observes the `✓ → ?` transition around the
/// in-place mutation done by [`refresh_cell_inner`].
fn refresh_cell(
    cell: &mut Cell,
    value: Option<Decimal>,
    provenance: &Provenance,
) -> (CellRefresh, bool) {
    let was_validated = cell.review == Review::Validated;
    let outcome = refresh_cell_inner(cell, value, provenance);
    let demoted = was_validated && cell.review == Review::ToReview;
    (outcome, demoted)
}

/// The branching that actually mutates the cell (Story 3.3):
/// - empty (gap) → fill from the provider, whatever the skeleton source;
/// - present + `Source::Manual` → reconcile (manual wins, divergent provider value preserved, 3.4);
/// - present + provider/derived → re-stamp via [`Cell::edited`] **only when the value changed** (a
///   divergent value auto-demotes a `✓` and is `Current`; an equal value is a true no-op). A
///   provider that returns no value for an existing cell keeps the last-known value (FR23 spirit).
fn refresh_cell_inner(
    cell: &mut Cell,
    value: Option<Decimal>,
    provenance: &Provenance,
) -> CellRefresh {
    // A deliberate "not available" decision (FR19) is a user gesture, NOT a gap — never refilled by a
    // refresh (it would silently flip the accepted-blank back to a provider value). Checked before the
    // empty-cell gap-fill, because an N/A-accepted cell also carries `value: None`.
    if cell.coverage == Coverage::NotAvailableAccepted {
        CellRefresh::Skipped
    } else if cell.value.is_none() {
        match value {
            Some(v) => {
                *cell = provider_cell(Some(v), provenance);
                CellRefresh::Filled
            }
            None => CellRefresh::Unchanged,
        }
    } else if cell.source == Source::Manual {
        // Non-destructive reconciliation (Story 3.4, FR22/NFR-R4): the manual value wins and is
        // never overwritten. A divergent provider value is preserved ALONGSIDE (pending) and demotes
        // a `✓`; an agreeing fetch clears any stale pending. A provider with no value is no contradiction.
        match value {
            Some(v) => {
                let reconciled = cell.reconcile(Some(Money::from(v)), provenance.clone());
                if reconciled == *cell {
                    CellRefresh::Unchanged
                } else {
                    *cell = reconciled;
                    CellRefresh::Reconciled
                }
            }
            None => CellRefresh::Unchanged,
        }
    } else {
        match value {
            Some(v) => {
                let new_value = Some(Money::from(v));
                if cell.value == new_value {
                    // The value agrees → a true no-op. (Any `Stale` flag from a prior failed refresh
                    // was already cleared up front by `apply_provider_refresh`'s outage-recovery pass,
                    // Story 3.5 — so this stays a pure value-based idempotency check.)
                    CellRefresh::Unchanged
                } else {
                    *cell = cell.edited(new_value, provenance.clone());
                    CellRefresh::Updated
                }
            }
            // The provider has no value now → retain the last-known value (never blank it).
            None => CellRefresh::Unchanged,
        }
    }
}

/// Refresh one **optional** cell slot (same semantics as [`refresh_cell`]; an absent slot is a gap).
/// Any present slot — including a value-less `ToFill` or `NotAvailableAccepted` cell — delegates to
/// [`refresh_cell`] so the N/A-accepted skip and the manual-skip rules apply uniformly; only a truly
/// absent (`None`) slot is filled directly. Returns `(outcome, demoted)` like [`refresh_cell`].
fn refresh_optional(
    slot: &mut Option<Cell>,
    value: Option<Decimal>,
    provenance: &Provenance,
) -> (CellRefresh, bool) {
    match slot {
        Some(cell) => refresh_cell(cell, value, provenance),
        None => match value {
            Some(v) => {
                *slot = Some(provider_cell(Some(v), provenance));
                (CellRefresh::Filled, false)
            }
            None => (CellRefresh::Unchanged, false),
        },
    }
}

/// Refresh every cell of one matching year, tallying updated/filled counts and OR-merging the
/// recompute cause from each cell that actually changed (a fill counts toward the cause too — it
/// feeds the recompute). Field names drive [`refresh::classify_field`] (no parallel list).
fn refresh_year(yd: &mut YearData, cy: &CanonicalYear, provenance: &Provenance) -> RefreshReport {
    let mut report = RefreshReport::default();
    let mut account = |(outcome, demoted): (CellRefresh, bool), field: &str| {
        // Story 3.6: a cell this refresh reset `✓ → ?` is one the user must re-verify after the
        // annual update — the re-validation scope, independent of the value-change tally below.
        if demoted {
            report.revalidate += 1;
        }
        match outcome {
            CellRefresh::Updated => {
                report.updated += 1;
                report.cause = report
                    .cause
                    .merge(crate::viewmodel::refresh::classify_field(field));
            }
            CellRefresh::Filled => {
                report.filled += 1;
                report.cause = report
                    .cause
                    .merge(crate::viewmodel::refresh::classify_field(field));
            }
            CellRefresh::Reconciled => {
                report.reconciled += 1;
                // A reconciled divergence can degrade the verdict (a demoted load-bearing ✓) — feed
                // the cause so the recompute notice names what moved.
                report.cause = report
                    .cause
                    .merge(crate::viewmodel::refresh::classify_field(field));
            }
            CellRefresh::Skipped | CellRefresh::Unchanged => {}
        }
    };
    // Required load-bearing cells (disjoint &mut borrows of distinct struct fields).
    account(refresh_cell(&mut yd.sales, cy.sales, provenance), "sales");
    account(refresh_cell(&mut yd.eps, cy.eps, provenance), "eps");
    account(
        refresh_cell(&mut yd.high_price, cy.high_price, provenance),
        "high_price",
    );
    account(
        refresh_cell(&mut yd.low_price, cy.low_price, provenance),
        "low_price",
    );
    // Optional cells.
    account(
        refresh_optional(
            &mut yd.dividend_per_share,
            cy.dividend_per_share,
            provenance,
        ),
        "dividend_per_share",
    );
    account(
        refresh_optional(&mut yd.pre_tax_profit, cy.pre_tax_profit, provenance),
        "pre_tax_profit",
    );
    account(
        refresh_optional(
            &mut yd.book_value_per_share,
            cy.book_value_per_share,
            provenance,
        ),
        "book_value_per_share",
    );
    report
}
