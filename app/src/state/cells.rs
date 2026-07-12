//! The cell / judgment editing rail (Epic 2 — FR16/FR19/FR20/FR31/FR49): manual value edits with
//! the soft-lock backstop (a validated `✓` cell refuses a direct edit until its sign-off is
//! deliberately cleared), review tags + bulk unlock ([`UnlockScope`]), the not-available-accepted
//! gap (N/A vs to-fill vs `0` kept distinct — a cleared value is `None`, **never `0`**),
//! paste-a-column, the numeric judgment fields + forecast-low option, the decision rationale and
//! the annual +1-year roll-forward — all through the shared re-read → mutate → `put_study` paths
//! that guard (read-only / no-journal / save-failure) and record an undo snapshot only on a real
//! change.

use steadyinvest_contract::{
    Cell, Coverage, ForecastLowOption, Judgment, Money, PendingProvider, Provenance, Review, Study,
};
use steadyinvest_persistence::Error as PersistError;
use uuid::Uuid;

use crate::viewmodel::entry;

use super::{
    JournalState, MSG_NO_JOURNAL, MSG_READ_ONLY_WRITE, MSG_SAVE_FAILED, MSG_SOFT_LOCKED,
    MSG_YEARS_MAX,
};

/// The scope of a bulk "unlock all" (Story 2.5): the whole study, a single year column, or a single
/// metric (one §3 column / §2 row) across all years. Each flips every `✓` it covers back to `?`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnlockScope {
    /// Every validated cell in the study.
    Study,
    /// Every validated cell in one materialized year (by index into the year window).
    Year(usize),
    /// Every validated cell of one field (a §3 column letter / §2 row key) across all years.
    Metric(String),
}

impl UnlockScope {
    /// Whether this scope covers the `(year_index, field)` cell address.
    fn covers(&self, year_index: usize, field: &str) -> bool {
        match self {
            UnlockScope::Study => true,
            UnlockScope::Year(y) => *y == year_index,
            UnlockScope::Metric(f) => f == field,
        }
    }
}

impl JournalState {
    /// Manually set/clear a cell's value (FR16): parse-side `value` (already a [`Money`] or `None`)
    /// is routed through the one mutation rail [`contract::Cell::edited`] with a manual provenance,
    /// then the whole [`Study`] is upserted via [`Journal::put_study`] (bumps `logical_version`,
    /// appends the FR51 time-series). A `None` value clears the cell to a [`Coverage::ToFill`] gap —
    /// **never `0`**. Reuses the read-only / no-journal / save-failure guards (no silent `.ok()`).
    pub fn edit_cell(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
        value: Option<Money>,
    ) -> Result<(), String> {
        // Soft-lock (Story 2.5): a validated (`✓`) cell refuses a DIRECT typed edit — the sign-off is
        // load-bearing and must be cleared deliberately first (clear-✓ → `?`), never undone by a
        // stray keystroke. This is the Rust-side backstop behind the read-only TextInput; the UI also
        // guards visually, but the rail is the testable, authoritative refusal. (Bulk `paste_column`
        // keeps the `Cell::edited` auto-demote backstop instead — recorded interpretation: typing is
        // blocked (a), paste is allowed-with-demote (b).)
        if self.current_review(study_id, year_index, field) == Some(Review::Validated) {
            return Err(MSG_SOFT_LOCKED.to_string());
        }
        self.mutate_cell(study_id, year_index, field, |base, provenance| {
            base.edited(value, provenance)
        })
    }

    /// The current review tag of a cell, or `None` if the study/year/cell is absent (a never-touched
    /// optional column reads as no cell). Used by the soft-lock guard before a direct value edit.
    fn current_review(&self, study_id: Uuid, year_index: usize, field: &str) -> Option<Review> {
        let study = self.get_study(study_id)?;
        let year = study.years.get(year_index)?;
        entry::get_cell(year, field).map(|cell| cell.review)
    }

    /// The cell's current pending provider divergence, if any (Story 3.4) — lets the resolve actions
    /// short-circuit to a true no-op (no journal write) when there is nothing to reconcile.
    fn current_pending(
        &self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
    ) -> Option<PendingProvider> {
        let study = self.get_study(study_id)?;
        let year = study.years.get(year_index)?;
        entry::get_cell(year, field).and_then(|cell| cell.pending)
    }

    /// Set a cell's **review tag only** (Story 2.5, FR20): a review-only change that preserves the
    /// value, coverage, source, freshness and provenance verbatim — it does NOT route through
    /// [`Cell::edited`] (which would re-stamp source/freshness from a fresh provenance and re-derive
    /// coverage). The cycle `none → ? → ✓ → none` and the deliberate clear-✓ (`✓ → ?`) both land
    /// here; the UI computes the target. Setting a tag on a never-touched optional column materializes
    /// a to-fill gap carrying the tag — the value stays `None`, **never `0`**. Persisted via
    /// [`Journal::put_study`]; reuses the read-only / no-journal / save-failure guards.
    ///
    /// **Interpretation (recorded):** a review-only edit changes ONLY the `review` field — the cell's
    /// provenance (and its origin timestamp) is preserved verbatim, never re-stamped. The sign-off
    /// act's timing is captured by the study-level `logical_version` bump (FR51), not a per-cell
    /// provenance overwrite, so a value's source/fetch time is never lost to a review toggle.
    pub fn set_review(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
        review: Review,
    ) -> Result<(), String> {
        // #47: a value-less cell may be flagged `?` (a gap to fill) but must NEVER be `✓`-validated.
        // Validating "nothing" — an existing to-fill cell OR a never-touched optional column that this
        // call would materialize as an empty gap — is degenerate: a later refresh gap-fills it and
        // `provider_cell` resets the review to `None`, so the `✓` vanishes `Validated → None` (NOT
        // `→ ToReview`), silently dropping the badge and escaping the Story-3.6 re-validate count.
        // Refuse it as a neutral no-op (no journal write, no undo step); `?`/`none` stay allowed.
        if review == Review::Validated {
            let value_present = self
                .get_study(study_id)
                .and_then(|study| {
                    study
                        .years
                        .get(year_index)
                        .and_then(|year| entry::get_cell(year, field))
                        .map(|cell| cell.value.is_some())
                })
                .unwrap_or(false);
            if !value_present {
                return Ok(());
            }
        }
        self.mutate_cell(study_id, year_index, field, move |base, _provenance| Cell {
            review,
            // Re-validating reconciles a pending divergence (Story 3.4 AC4): the kept value stands
            // and the "provider differs" annotation clears. A non-✓ review leaves any pending intact.
            pending: if review == Review::Validated {
                None
            } else {
                base.pending.clone()
            },
            ..base
        })
    }

    /// Resolve a pending divergence by **accepting the provider value** (Story 3.4, AC4): the cell
    /// takes its pending provider value through the edit rail (→ `Source::Provider`,
    /// `Review::ToReview` so it is re-checked, pending cleared by `edited`). A neutral no-op if there
    /// is no pending. Routed through the atomic `mutate_cell` rail (guards, undo).
    pub fn accept_provider_value(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
    ) -> Result<(), String> {
        // True no-op when there is nothing to reconcile (no journal write, no undo step).
        let Some(pending) = self.current_pending(study_id, year_index, field) else {
            return Ok(());
        };
        // A pending with no value (only representable by a future caller — the refresh path never
        // produces one) would BLANK the manual value; treat it as keep-manual instead (never destroy).
        if pending.value.is_none() {
            return self.keep_manual_value(study_id, year_index, field);
        }
        self.mutate_cell(study_id, year_index, field, move |base, _provenance| {
            base.edited(pending.value, pending.provenance)
        })
    }

    /// Resolve a pending divergence by **keeping the manual value** (Story 3.4, AC4): the live value
    /// stands; only the pending "provider differs" annotation is cleared (the `✓` was already demoted
    /// to `?` by the divergence — keep-manual just dismisses the annotation, leaving the review as-is
    /// for the user to re-validate). A neutral no-op if there is no pending.
    pub fn keep_manual_value(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
    ) -> Result<(), String> {
        // True no-op when there is no pending to dismiss (no journal write, no undo step).
        if self.current_pending(study_id, year_index, field).is_none() {
            return Ok(());
        }
        self.mutate_cell(study_id, year_index, field, |base, _provenance| Cell {
            pending: None,
            ..base
        })
    }

    /// Bulk "unlock all" (Story 2.5, FR20): flip every `Review::Validated → ToReview` within `scope`
    /// (study / year / metric), leaving `None`/`ToReview` cells untouched, in **one** persisted
    /// upsert (one `logical_version` bump). Returns the count of cells actually flipped (surfaced in a
    /// neutral notice). A review-only flip — values/coverage are never touched. Reuses the read-only /
    /// no-journal / save-failure guards.
    pub fn unlock_all(&mut self, study_id: Uuid, scope: &UnlockScope) -> Result<usize, String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let before = study.clone(); // pre-mutation snapshot for undo (Story 2.9)
        let mut flipped = 0usize;
        for (year_index, year) in study.years.iter_mut().enumerate() {
            for field in entry::ALL_FIELDS {
                if !scope.covers(year_index, field) {
                    continue;
                }
                let Some(cell) = entry::get_cell(year, field) else {
                    continue; // an absent optional column carries no sign-off
                };
                if cell.review != Review::Validated {
                    continue;
                }
                let demoted = Cell {
                    review: Review::ToReview,
                    ..cell
                };
                entry::set_cell(year, field, demoted).map_err(|()| MSG_SAVE_FAILED.to_string())?;
                flipped += 1;
            }
        }
        // Issue #34 (FR51): the save also appends the durable snapshot — same transaction,
        // deduplicated (a no-op re-save records no phantom history entry).
        let now = self.clock.now();
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study_with_history(&study, &now)
        };
        match result {
            Ok(()) => {
                // Only an unlock that actually flipped a ✓ is an undoable change.
                if flipped > 0 {
                    self.history.record(before);
                }
                Ok(flipped)
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Count the validated (`✓`) cells the given `scope` would flip — for the confirmation prompt
    /// (the bulk change is never silent). A read-only/pure query; never mutates or persists.
    pub fn count_validated(&self, study_id: Uuid, scope: &UnlockScope) -> usize {
        let Some(study) = self.get_study(study_id) else {
            return 0;
        };
        let mut count = 0usize;
        for (year_index, year) in study.years.iter().enumerate() {
            for field in entry::ALL_FIELDS {
                if !scope.covers(year_index, field) {
                    continue;
                }
                if entry::get_cell(year, field).map(|c| c.review) == Some(Review::Validated) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Mark a cell **not-available-accepted** (a deliberate, permanent gap — FR19), or clear that
    /// back to a to-fill gap (`accepted = false`). The value is cleared to `None` either way; only
    /// the coverage differs, so this is N/A vs to-fill vs 0 kept distinct. Persisted through the
    /// same upsert rail as [`Self::edit_cell`].
    pub fn set_not_available(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
        accepted: bool,
    ) -> Result<(), String> {
        // Soft-lock (Story 2.5): the not-available gesture is a value/coverage mutation, and AC 2
        // names it among the edits a `✓` cell must refuse — routed through `Cell::edited(None, …)` it
        // would both blank the value AND demote `✓ → ?` (a divergent edit), silently undoing the
        // sign-off. The UI swallows Ctrl+Space on a locked cell; this is the authoritative, testable
        // Rust backstop, symmetric with `edit_cell`, so the sign-off can never be lost by this path.
        if self.current_review(study_id, year_index, field) == Some(Review::Validated) {
            return Err(MSG_SOFT_LOCKED.to_string());
        }
        self.mutate_cell(study_id, year_index, field, move |base, provenance| {
            // Reuse the edit rail for value/source/freshness/review, then override coverage only —
            // `NotAvailableAccepted` is a coverage-only gesture, not reachable through `edited`.
            let cleared = base.edited(None, provenance);
            Cell {
                coverage: if accepted {
                    Coverage::NotAvailableAccepted
                } else {
                    Coverage::ToFill
                },
                ..cleared
            }
        })
    }

    /// The shared validate→materialize→mutate→persist path for a single cell. `make` builds the new
    /// cell from the current one (cloned, snapshot semantics) and a fresh manual provenance. The
    /// year-grid skeleton is materialized **on first edit** (not before — an untouched study is
    /// never written as all-empty rows).
    fn mutate_cell(
        &mut self,
        study_id: Uuid,
        year_index: usize,
        field: &str,
        make: impl FnOnce(Cell, Provenance) -> Cell,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        // Re-read the authoritative study, mutate one cell, write it back (the 2.2/2.3 rail).
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        // The pre-mutation snapshot for undo (Story 2.9) — captured as read, before any materialize.
        let before = study.clone();
        if study.years.is_empty() {
            study.years =
                entry::materialize_year_window(&study.created_at, &self.manual_provenance());
        }
        let year = study
            .years
            .get_mut(year_index)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let base = entry::get_cell(year, field)
            .unwrap_or_else(|| entry::tofill_cell(self.manual_provenance()));
        let new_cell = make(base, self.manual_provenance());
        entry::set_cell(year, field, new_cell).map_err(|()| MSG_SAVE_FAILED.to_string())?;

        // Issue #34 (FR51): the save also appends the durable snapshot — same transaction,
        // deduplicated (a no-op re-save records no phantom history entry).
        let now = self.clock.now();
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study_with_history(&study, &now)
        };
        match result {
            Ok(()) => {
                // Only a REAL change is undoable — a no-op edit (same value re-typed, same option
                // re-selected, same review tag) must not push a phantom step or clear redo (review P4).
                if before != study {
                    self.history.record(before);
                }
                Ok(())
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Paste a parsed column into consecutive years of the **same field**, downward from
    /// `start_year` (FR16). Each value is already a locale-parsed [`Money`] / `None`
    /// ([`entry::parse_pasted_column`]); a `None` line leaves its cell an empty to-fill gap (**never
    /// `0`**). Lines past the last year are dropped. One upsert for the whole column (one
    /// `logical_version` bump). Returns the number of cells actually filled (the caller compares it
    /// with the column length to surface a neutral "some lines dropped" notice).
    pub fn paste_column(
        &mut self,
        study_id: Uuid,
        start_year: usize,
        field: &str,
        values: &[Option<Money>],
    ) -> Result<usize, String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let before = study.clone(); // pre-mutation snapshot for undo (Story 2.9)
        if study.years.is_empty() {
            study.years =
                entry::materialize_year_window(&study.created_at, &self.manual_provenance());
        }
        let mut filled = 0usize;
        for (offset, value) in values.iter().enumerate() {
            let Some(year) = study.years.get_mut(start_year + offset) else {
                break; // ran past the grid bottom — drop the surplus line
            };
            let base = entry::get_cell(year, field)
                .unwrap_or_else(|| entry::tofill_cell(self.manual_provenance()));
            let new_cell = base.edited(*value, self.manual_provenance());
            entry::set_cell(year, field, new_cell).map_err(|()| MSG_SAVE_FAILED.to_string())?;
            filled += 1;
        }
        // Issue #34 (FR51): the save also appends the durable snapshot — same transaction,
        // deduplicated (a no-op re-save records no phantom history entry).
        let now = self.clock.now();
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study_with_history(&study, &now)
        };
        match result {
            Ok(()) => {
                // Only a real change is undoable (review P4).
                if before != study {
                    self.history.record(before);
                }
                Ok(filled)
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// Set (or clear) one **numeric judgment field** (Story 2.6, FR6/FR31): re-read the study, set
    /// the one `Judgment` field on the mutation rail, then `put_study` — reusing the read-only /
    /// no-journal / save-failure guards (no silent `.ok()`). A cleared field is `None`, **never `0`**
    /// (the project's most-repeated rail). The `field` key is the Slint callback's wire identifier
    /// ([`judgment_field`] maps it to the struct field).
    pub fn set_judgment_field(
        &mut self,
        study_id: Uuid,
        field: &str,
        value: Option<Money>,
    ) -> Result<(), String> {
        self.mutate_judgment(study_id, |judgment| {
            apply_judgment_field(judgment, field, value)
        })
    }

    /// Select the §4 forecast-low option (Story 2.6) — a judgment edit through the same rail.
    pub fn set_forecast_low_option(
        &mut self,
        study_id: Uuid,
        option: ForecastLowOption,
    ) -> Result<(), String> {
        self.mutate_judgment(study_id, |judgment| {
            judgment.forecast_low_option = option;
            true
        })
    }

    /// Set (or clear) the study-level **decision rationale** (Story 2.10, FR49): the user's free-text
    /// "why I judged this way" note. Trims the incoming text and stores `Some(trimmed)`, or `None`
    /// when it is empty/whitespace-only — the project's "absence ≠ empty value" rail (never the
    /// empty-but-present `Some("")` surprise). Routed through [`Self::mutate_study`] so it is atomic
    /// (one `put_study`, `logical_version` bumped), guarded (read-only / no-journal / save-failure →
    /// a neutral notice, never a silent `.ok()`), and undoable (recorded only on a real change). The
    /// rationale is the user's own words and is therefore **never** posture-scanned — only the
    /// system-supplied label/placeholder are (FR13).
    pub fn set_rationale(&mut self, study_id: Uuid, text: Option<String>) -> Result<(), String> {
        let normalized = text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        self.mutate_study(study_id, move |study| {
            study.rationale = normalized;
        })
    }

    /// Commit the header card's company name (2026-07-12). Same rail as [`Self::set_rationale`]:
    /// free text trimmed → `Some`/`None` (empty ⇒ absence, never `Some("")`), atomic + guarded +
    /// undoable via [`Self::mutate_study`]. User's own text — never posture-scanned.
    pub fn set_company_name(&mut self, study_id: Uuid, text: Option<String>) -> Result<(), String> {
        let normalized = text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        self.mutate_study(study_id, move |study| {
            study.company_name = normalized;
        })
    }

    /// Extend the study's data window forward by one year (Story 2.11, FR3): the **annual roll-forward**
    /// ritual. Appends a fresh [`entry::tofill_year`] column for `latest_year + 1` (all cells
    /// [`Coverage::ToFill`], no value computed — adding a year is structure, not calculation). The
    /// engine then re-bases its canonical 5-year forward projection off the new latest **usable** year
    /// once that year's EPS is entered ([`core`] reads `latest_usable`), so zones/verdict recompute in
    /// the same coherence frame — **no method change** (`FORECAST_HORIZON_YEARS` stays `5`). Rides
    /// [`Self::mutate_study`]: atomic (one `put_study`, `logical_version` bumped), guarded (read-only /
    /// no-journal / save-failure → a neutral notice), and undoable (an append always changes `years`,
    /// so it always records one undo step). A never-edited study (empty `years`) first materializes the
    /// canonical window (the in-memory view the user sees), then appends — so "+ année" on a fresh study
    /// grows it 5 → 6, never errors. The degraded all-empty case (unreadable `created_at`) appends
    /// `year 0` — safe, never a panic.
    pub fn extend_history(&mut self, study_id: Uuid) -> Result<(), String> {
        // Read-only takes precedence over the cap (you cannot extend a read-only journal at all).
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        // Issue #35: bound the window so repeated "+ année" cannot overflow the §2 horizontal layout.
        // A fresh (never-edited) study shows the materialized `YEAR_WINDOW`, so count THAT, not 0 —
        // the append below materializes first, then grows it. A neutral notice, never a silent stop.
        if let Some(study) = self.get_study(study_id) {
            let current = if study.years.is_empty() {
                entry::YEAR_WINDOW
            } else {
                study.years.len()
            };
            if current >= entry::MAX_HISTORY_YEARS {
                return Err(MSG_YEARS_MAX.to_string());
            }
        }
        let provenance = self.manual_provenance();
        self.mutate_study(study_id, move |study| {
            // Materialize the canonical window on first touch (the 2.4 materialize-on-first-edit rail),
            // so extending a never-edited study grows the window the user actually sees, not an empty one.
            if study.years.is_empty() {
                study.years = entry::materialize_year_window(&study.created_at, &provenance);
            }
            let next_year = study
                .years
                .iter()
                .map(|y| y.year)
                .max()
                // `saturating_add` keeps the year monotonic even at the i32 ceiling (defensive — real
                // fiscal years never approach it, but the value is read from the journal blob).
                .map(|latest| latest.saturating_add(1))
                .or_else(|| entry::created_year(&study.created_at))
                .unwrap_or(0);
            // Newest sits at the bottom (oldest→newest SSG order); the appended year is the new max.
            study.years.push(entry::tofill_year(next_year, provenance));
        })
    }

    /// The shared re-read → mutate-whole-study → persist path for a study-level field that is neither
    /// a review-tagged [`Cell`] nor a [`Judgment`] input (Story 2.10: the decision rationale). Mirrors
    /// [`Self::mutate_judgment`] but hands `apply` the whole [`Study`]. Records an undo snapshot only
    /// on a real change (`before != study`, the Story-2.9 guard) so a no-op re-save pushes no phantom
    /// step. Reuses the read-only / no-journal / save-failure guards.
    pub(crate) fn mutate_study(
        &mut self,
        study_id: Uuid,
        apply: impl FnOnce(&mut Study),
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let before = study.clone(); // pre-mutation snapshot for undo (Story 2.9)
        apply(&mut study);
        // Issue #34 (FR51): the save also appends the durable snapshot — same transaction,
        // deduplicated (a no-op re-save records no phantom history entry).
        let now = self.clock.now();
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study_with_history(&study, &now)
        };
        match result {
            Ok(()) => {
                // Only a REAL change is undoable — re-saving the same rationale must not push a
                // phantom step or clear redo (review P4).
                if before != study {
                    self.history.record(before);
                }
                Ok(())
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }

    /// The shared re-read → mutate-judgment → persist path. `apply` returns `false` for an unknown
    /// field key (a neutral save-failure notice; never a panic). Mirrors [`Self::mutate_cell`] but
    /// for the bare `Judgment` snapshot (judgment inputs are not review-tagged `Cell`s).
    fn mutate_judgment(
        &mut self,
        study_id: Uuid,
        apply: impl FnOnce(&mut Judgment) -> bool,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        if self.journal.is_none() {
            return Err(MSG_NO_JOURNAL.to_string());
        }
        let mut study = self
            .get_study(study_id)
            .ok_or_else(|| MSG_SAVE_FAILED.to_string())?;
        let before = study.clone(); // pre-mutation snapshot for undo (Story 2.9)
        if !apply(&mut study.judgment) {
            return Err(MSG_SAVE_FAILED.to_string());
        }
        // Issue #34 (FR51): the save also appends the durable snapshot — same transaction,
        // deduplicated (a no-op re-save records no phantom history entry).
        let now = self.clock.now();
        let result = {
            let journal = self
                .journal
                .as_mut()
                .expect("journal presence checked above");
            journal.put_study_with_history(&study, &now)
        };
        match result {
            Ok(()) => {
                // Only a REAL change is undoable — a no-op edit (same value re-typed, same option
                // re-selected, same review tag) must not push a phantom step or clear redo (review P4).
                if before != study {
                    self.history.record(before);
                }
                Ok(())
            }
            Err(PersistError::NewerJournalSchema { .. }) => Err(MSG_READ_ONLY_WRITE.to_string()),
            Err(error) => Err(format!("{MSG_SAVE_FAILED} {error}")),
        }
    }
}

/// Set one numeric judgment field by its Slint wire key (Story 2.6). Returns `false` for an unknown
/// key (the caller surfaces a neutral notice; never a panic). A `None` value clears the field —
/// **never `0`**. The `forecast_low_option` selector has its own rail ([`JournalState::
/// set_forecast_low_option`]) — it is not routed here.
pub(crate) fn apply_judgment_field(
    judgment: &mut Judgment,
    field: &str,
    value: Option<Money>,
) -> bool {
    match field {
        "sales_growth" => judgment.projected_sales_growth_pct = value,
        "eps_growth" => judgment.projected_eps_growth_pct = value,
        "est_high_eps" => judgment.estimated_high_eps = value,
        "est_low_eps" => judgment.estimated_low_eps = value,
        "high_pe" => judgment.judged_avg_high_pe = value,
        "low_pe" => judgment.judged_avg_low_pe = value,
        "recent_severe_low" => judgment.recent_severe_low = value,
        "current_price" => judgment.current_price = value,
        "dividend" => judgment.present_full_year_dividend = value,
        _ => return false,
    }
    true
}
