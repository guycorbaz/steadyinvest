//! Tests for the `state` rails (Stories 2.x–6.x): undo/redo, manual entry + soft-lock, provider
//! refresh / reconcile / stale, watchlist, portfolios & holdings & sells, restore-from-backup,
//! journal location & locks, export/import, confront. Moved **verbatim** from the pre-split
//! monolithic `state.rs`; kept as one file for the mechanical split (a per-topic split is a later
//! pass). `use super::*` sees the whole re-exported `state` surface.

use super::*;
use crate::clock::{FixedClock, FixedIdGen};
use crate::viewmodel::{engine, entry};
use rust_decimal::Decimal;
use steadyinvest_contract::{Coverage, Freshness, Money, Review, Source, Study};
use steadyinvest_ingestion::FetchedFinancials;
use steadyinvest_persistence::ImportSummary;
use tempfile::TempDir;

fn fixed(id: u128, ts: &str) -> (Box<dyn Clock>, Box<dyn IdGen>) {
    (
        Box::new(FixedClock(Timestamp(ts.to_string()))),
        Box::new(FixedIdGen(Uuid::from_u128(id))),
    )
}

// ── Story 2.9 — undo/redo history ──

/// Open a fresh temp journal + state (creating the file on first use), with injected clock/id.
fn undo_state(dir: &TempDir, seed: u128, ts: &str) -> JournalState {
    let path = dir.path().join("journal.db");
    if !path.exists() {
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xC0FFEE),
                &Timestamp("2026-06-14T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
    }
    let (clock, idgen) = fixed(seed, ts);
    let (state, _) = JournalState::open_or_create(Some(&path), clock, idgen);
    state
}

/// Like [`watch_state`] but with a caller-chosen **journal id** — for the foreign-envelope
/// arbitration tests (issue #65: a foreign journal has no version axis to compare).
fn watch_state_with_journal_id(dir: &TempDir, seed: u128, jid: u128) -> JournalState {
    let path = dir.path().join("journal.db");
    if !path.exists() {
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(jid),
                &Timestamp("2026-06-14T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
    }
    let clock: Box<dyn Clock> = Box::new(FixedClock(Timestamp("2026-06-27T15:00:00Z".to_string())));
    let idgen: Box<dyn IdGen> = Box::new(crate::clock::SeqIdGen::starting_at(seed));
    let (state, _) = JournalState::open_or_create(Some(&path), clock, idgen);
    state
}

/// Like [`undo_state`] but with a **sequential** id source — for tests that create several
/// entities (Story 4.1 watchlist: each `add_watch_item` needs a distinct id).
fn watch_state(dir: &TempDir, seed: u128) -> JournalState {
    let path = dir.path().join("journal.db");
    if !path.exists() {
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xC0FFEE),
                &Timestamp("2026-06-14T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
    }
    let clock: Box<dyn Clock> = Box::new(FixedClock(Timestamp("2026-06-27T15:00:00Z".to_string())));
    let idgen: Box<dyn IdGen> = Box::new(crate::clock::SeqIdGen::starting_at(seed));
    let (state, _) = JournalState::open_or_create(Some(&path), clock, idgen);
    state
}

fn und_money(v: i64) -> Money {
    Money::from(rust_decimal::Decimal::new(v, 0))
}

// ── Story 3.1 — provider fetch pipeline (FakeProvider-style, offline) ──

/// A normalized provider result covering the given fiscal years, every load-bearing field present.
fn fetched_for(years: &[i32]) -> FetchedFinancials {
    fetched_custom(years, 1000, 5, 100, 50, "deadbeefcafe")
}

/// A normalized provider result with caller-chosen load-bearing values + digest — for refresh
/// divergence/idempotency tests (Story 3.3).
fn fetched_custom(
    years: &[i32],
    sales: i64,
    eps: i64,
    high: i64,
    low: i64,
    digest: &str,
) -> FetchedFinancials {
    use steadyinvest_core::normalize::{RawAmount, RawFinancials, RawYear, normalize};
    let amt = |v: i64| {
        Some(RawAmount {
            value: rust_decimal::Decimal::new(v, 0),
            currency: "CHF".to_string(),
        })
    };
    let rows = years
        .iter()
        .map(|&y| RawYear {
            sales: amt(sales),
            eps: amt(eps),
            high_price: amt(high),
            low_price: amt(low),
            ..RawYear::empty(y)
        })
        .collect();
    let raw = RawFinancials {
        native_currency: "CHF".to_string(),
        years: rows,
        splits: vec![],
    };
    FetchedFinancials {
        canonical: normalize(raw).expect("the test raw normalizes"),
        digest: digest.to_string(),
        latest_price: None,
        latest_session_date: None,
        ttm_eps: None,
    }
}

/// A normalized provider result that also carries a latest `/eod` close (Story 4.4) — drives the
/// §4 zone recompute deterministically in `apply_provider_refresh` tests.
fn fetched_with_price(years: &[i32], latest_price: i64) -> FetchedFinancials {
    FetchedFinancials {
        latest_price: Some(rust_decimal::Decimal::new(latest_price, 0)),
        ..fetched_for(years)
    }
}

#[test]
fn a_refresh_surfaces_provider_years_that_fall_outside_the_grid() {
    // Issue #37 (Finding 5): a fresh study builds its grid from the provider years (all integrated →
    // 0 unmatched). A SECOND refresh whose provider years do NOT overlap the existing grid fills
    // nothing (fill-gaps-only scope — the grid never grows, cf. #35), but the dropped years are
    // COUNTED so the notice is not a silent "filled = 0" indistinguishable from "nothing to do".
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x37, "2026-06-15T10:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();

    // First fetch builds the grid 2016–2018; every provider year is integrated.
    let first = state
        .apply_provider_refresh(id, &fetched_for(&[2016, 2017, 2018]))
        .unwrap();
    assert_eq!(
        first.unmatched_years, 0,
        "a fresh grid integrates all years"
    );
    assert_eq!(state.get_study(id).unwrap().years.len(), 3);

    // A second fetch with DISJOINT years: the grid stays 2016–2018 (no growth), nothing is filled,
    // and the two non-overlapping provider years are surfaced as unmatched.
    let second = state
        .apply_provider_refresh(id, &fetched_for(&[2023, 2024]))
        .unwrap();
    assert_eq!(
        second.filled, 0,
        "disjoint years fill nothing (fill-gaps-only)"
    );
    assert_eq!(
        second.unmatched_years, 2,
        "the two provider years outside the grid are counted, not silently dropped"
    );
    assert_eq!(
        state.get_study(id).unwrap().years.len(),
        3,
        "the grid never grows from a refresh (no #35 regression)"
    );
    // The user-facing summary names the unmatched years.
    assert!(
        crate::state::refresh_summary(second).contains("hors de la grille"),
        "the notice surfaces the unmatched provider years"
    );
}

#[test]
fn provider_fetch_fills_a_fresh_study_with_provider_stamped_cells() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x3F, "2026-06-15T10:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();

    let fetched = fetched_for(&[2020, 2021, 2022, 2023, 2024]);
    let report = state.apply_provider_refresh(id, &fetched).unwrap();
    assert_eq!(
        report.filled, 20,
        "5 years × 4 load-bearing cells were filled"
    );
    assert_eq!(report.updated, 0, "a first fetch fills, it does not update");
    assert!(
        report.cause.price && report.cause.input,
        "filling prices + fundamentals classifies as both"
    );

    let study = state.get_study(id).unwrap();
    assert_eq!(study.years.len(), 5);
    let sales = &study.years[0].sales;
    assert_eq!(sales.source, Source::Provider);
    assert_eq!(
        sales.review,
        Review::None,
        "fresh provider data is unvalidated"
    );
    assert_eq!(sales.freshness, Freshness::Current);
    assert_eq!(sales.coverage, Coverage::Present);
    assert_eq!(sales.provenance.source, Source::Provider);
    assert_eq!(
        sales.provenance.hash_of_dependencies, "deadbeefcafe",
        "the real fetch digest replaces the manual placeholder (#21)"
    );
}

/// Issue #109: a fetch drops the in-progress current year — a provider `/eod`-only row for a fiscal
/// year whose annual statements are not filed (price + EPS, but NO `sales`) is not a usable analysis
/// year. The study materializes the COMPLETE years only (matching the manual window).
#[test]
fn provider_fetch_drops_the_in_progress_year_without_annual_statements() {
    use steadyinvest_core::normalize::{RawAmount, RawFinancials, RawYear, normalize};
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x4C, "2026-06-15T10:00:00Z");
    let id = state.create_study("AAPL", "CHF").unwrap();

    let amt = |v: i64| {
        Some(RawAmount {
            value: rust_decimal::Decimal::new(v, 0),
            currency: "CHF".to_string(),
        })
    };
    // 2021..=2024 are complete; 2025 is the in-progress year — a price + EPS row with NO sales.
    let mut rows: Vec<RawYear> = (2021..=2024)
        .map(|y| RawYear {
            sales: amt(1000),
            eps: amt(5),
            high_price: amt(100),
            low_price: amt(50),
            ..RawYear::empty(y)
        })
        .collect();
    rows.push(RawYear {
        eps: amt(6),
        high_price: amt(120),
        low_price: amt(60),
        ..RawYear::empty(2025)
    });
    let fetched = FetchedFinancials {
        canonical: normalize(RawFinancials {
            native_currency: "CHF".to_string(),
            years: rows,
            splits: vec![],
        })
        .expect("normalizes"),
        digest: "d109".to_string(),
        latest_price: None,
        latest_session_date: None,
        ttm_eps: None,
    };

    state.apply_provider_refresh(id, &fetched).unwrap();
    let years: Vec<i32> = state
        .get_study(id)
        .unwrap()
        .years
        .iter()
        .map(|y| y.year)
        .collect();
    assert_eq!(
        years,
        vec![2021, 2022, 2023, 2024],
        "the in-progress year (no sales) is dropped — analysis uses complete years only"
    );
}

/// Issue #113: a fetch that carries the latest price + the trailing-twelve-months EPS lets the engine
/// compute the current P/E (`current_price / TTM`). The TTM is stored on the judgment (a market fact,
/// like current_price); `to_observations` feeds it to the engine as the current-P/E denominator.
#[test]
fn a_fetch_with_ttm_eps_computes_the_current_pe() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x4D, "2026-06-20T13:00:00Z");
    let id = state.create_study("AAPL", "CHF").unwrap();
    let fetched = FetchedFinancials {
        latest_price: Some(rust_decimal::Decimal::new(100, 0)), // current price 100
        ttm_eps: Some(rust_decimal::Decimal::new(5, 0)),        // TTM EPS 5 → current P/E = 20
        ..fetched_for(&[2020, 2021, 2022, 2023, 2024])
    };
    state.apply_provider_refresh(id, &fetched).unwrap();
    let study = state.get_study(id).unwrap();
    assert_eq!(
        study.judgment.ttm_eps.map(|m| m.as_decimal()),
        Some(rust_decimal::Decimal::new(5, 0)),
        "the TTM EPS is stored on the judgment (a market fact, like current_price)"
    );
    let snap = crate::viewmodel::engine::build_snapshot(&study).expect("normalizes");
    assert_eq!(
        snap.outputs().valuation.current_pe,
        Some(rust_decimal::Decimal::new(20, 0)),
        "#113: current P/E = current_price(100) / TTM(5) = 20"
    );
}

// ── Story 5.2 — export / import a single study ──

#[test]
fn export_import_round_trips_an_equal_study_preserving_identity() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x520);
    let id = state.create_study("NESN", "CHF").unwrap();
    let original = state.get_study(id).expect("the study exists");

    let envelope = state.export_study(id).expect("export succeeds");
    state.delete_study(id).expect("delete the study");
    assert!(
        state.get_study(id).is_none(),
        "the study is gone before import"
    );

    let (imported_id, overwrote) = state.import_study(&envelope).expect("import succeeds");
    assert_eq!(imported_id, id, "the study id is preserved on round-trip");
    assert!(
        !overwrote,
        "a fresh import (the study was deleted) is not an overwrite"
    );
    assert_eq!(
        state.get_study(id).expect("the study is back"),
        original,
        "export → import yields an equal study"
    );

    // A second import of the same envelope is an idempotent update, surfaced as an overwrite.
    let (_id, overwrote_again) = state
        .import_study(&envelope)
        .expect("re-import updates in place");
    assert!(
        overwrote_again,
        "re-import onto an existing id is surfaced as an overwrite"
    );
    assert_eq!(state.list_studies().len(), 1, "no duplicate study");
}

#[test]
fn importing_onto_an_archived_study_reactivates_it() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x523);
    let id = state.create_study("NESN", "CHF").unwrap();
    let envelope = state.export_study(id).unwrap();
    state.archive_study(id).expect("archive the study");
    assert_eq!(
        state.list_studies()[0].status,
        "archived",
        "the study is hidden before re-import"
    );

    let (_id, overwrote) = state.import_study(&envelope).expect("re-import succeeds");
    assert!(overwrote, "re-import onto the archived id is an overwrite");
    assert_eq!(
        state.list_studies()[0].status,
        "active",
        "an imported study is reactivated, never left silently hidden"
    );
}

#[test]
fn export_of_a_missing_study_is_a_neutral_refusal() {
    let dir = TempDir::new().unwrap();
    let state = watch_state(&dir, 0x521);
    assert_eq!(
        state.export_study(Uuid::from_u128(0xDEAD)),
        Err(MSG_EXPORT_MISSING.to_string())
    );
}

#[test]
fn export_of_a_present_but_unreadable_study_names_the_row_not_missing() {
    // Issue #63 — a row whose stored schema_version is newer than this build is PRESENT (the
    // dashboard's list_studies reads only indexed columns, no payload parse) but unreadable. Write
    // one directly via `journal.put_study` (bypassing the app-level create/normalize path, which
    // never produces this) to simulate a file written by a newer build.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x524);
    let id = state.create_study("NESN", "CHF").unwrap();
    let mut future = state.get_study(id).expect("the study exists");
    future.schema_version = steadyinvest_contract::SCHEMA_VERSION + 1;
    state.journal.as_mut().unwrap().put_study(&future).unwrap();

    assert_eq!(
        state.list_studies().len(),
        1,
        "the row is still listed — list_studies never parses the payload"
    );
    assert_eq!(
        state.export_study(id),
        Err(MSG_EXPORT_UNREADABLE.to_string()),
        "present-but-unreadable is distinct from MSG_EXPORT_MISSING"
    );
}

#[test]
fn import_maps_each_rejection_to_its_neutral_notice_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x522);
    let id = state.create_study("NESN", "CHF").unwrap();
    let good = state.export_study(id).unwrap();

    // Tamper → integrity refusal.
    let tampered = good.replacen("NESN", "ROG0", 1);
    assert_eq!(
        state.import_study(&tampered),
        Err(MSG_IMPORT_INTEGRITY.to_string())
    );
    // Garbage → malformed refusal.
    assert_eq!(
        state.import_study("not an envelope"),
        Err(MSG_IMPORT_MALFORMED.to_string())
    );
    assert_eq!(
        state.list_studies().len(),
        1,
        "a rejected import wrote nothing"
    );
}

// ── Story 5.3 — export / import the whole journal ──

#[test]
fn journal_export_import_round_trips_into_a_fresh_journal() {
    // Populate journal A with a study, a linked watchlist row and a holding.
    let dir_a = TempDir::new().unwrap();
    let mut state_a = watch_state(&dir_a, 0x530);
    let study_id = state_a.create_study("NESN", "CHF").unwrap();
    state_a.add_watch_item("NESN", Some(study_id)).unwrap();
    state_a.add_holding("NESN", "10", "100.00", "CHF").unwrap();
    let envelope = state_a.export_journal().expect("export succeeds");

    // Import into a fresh, empty journal B (a different dir → a different journal_id).
    let dir_b = TempDir::new().unwrap();
    let mut state_b = watch_state(&dir_b, 0x531);
    assert!(state_b.list_studies().is_empty(), "B starts empty");
    let summary = state_b.import_journal(&envelope).expect("import succeeds");

    assert_eq!(summary.studies, 1);
    assert_eq!(summary.watch_items, 1);
    assert_eq!(summary.holdings, 1);
    assert_eq!(state_b.list_studies().len(), 1, "the study landed in B");
    assert_eq!(state_b.list_watch_items().len(), 1, "the watch row landed");
    assert_eq!(state_b.list_holdings().len(), 1, "the holding landed");
    // The study's journal_id is rebound to B (seed semantics), id preserved.
    assert!(state_b.get_study(study_id).is_some(), "study id preserved");
}

#[test]
fn multiple_portfolios_round_trip_through_the_whole_journal_export() {
    // Story 6.1 / AC4: more than one portfolio must survive the 5.3 export/import unchanged.
    let dir_a = TempDir::new().unwrap();
    let mut state_a = watch_state(&dir_a, 0x612);
    state_a.add_holding("NESN", "10", "100", "CHF").unwrap(); // creates + fills the default portfolio
    let pf2 = state_a.add_portfolio("PostFinance").unwrap();
    state_a.add_holding("ROG", "5", "248", "CHF").unwrap(); // lands in the active (PostFinance)
    assert_eq!(state_a.list_portfolios().len(), 2);
    let envelope = state_a.export_journal().expect("export succeeds");

    let dir_b = TempDir::new().unwrap();
    let mut state_b = watch_state(&dir_b, 0x613);
    state_b.import_journal(&envelope).expect("import succeeds");

    let names: Vec<_> = state_b
        .list_portfolios()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert!(
        names.iter().any(|n| n == "PostFinance"),
        "the second portfolio round-trips, got {names:?}"
    );
    assert_eq!(
        state_b.list_portfolios().len(),
        2,
        "both portfolios land in B"
    );
    // The PostFinance holding is reachable when that portfolio is active.
    state_b.set_active_portfolio(pf2);
    let active: Vec<_> = state_b
        .list_holdings()
        .iter()
        .map(|h| h.security_ticker.clone())
        .collect();
    assert_eq!(
        active,
        ["ROG"],
        "the holding stays in its portfolio across the round-trip"
    );
}

#[test]
fn journal_import_maps_each_rejection_to_its_neutral_notice_and_writes_nothing() {
    let dir_a = TempDir::new().unwrap();
    let mut state_a = watch_state(&dir_a, 0x532);
    state_a.create_study("NESN", "CHF").unwrap();
    let good = state_a.export_journal().unwrap();

    let dir_b = TempDir::new().unwrap();
    let mut state_b = watch_state(&dir_b, 0x533);

    let tampered = good.replacen("NESN", "ROG0", 1);
    assert_eq!(
        state_b.import_journal(&tampered),
        Err(MSG_IMPORT_INTEGRITY.to_string())
    );
    assert_eq!(
        state_b.import_journal("not an envelope"),
        Err(MSG_IMPORT_MALFORMED.to_string())
    );
    assert!(
        state_b.list_studies().is_empty(),
        "a rejected whole-journal import wrote nothing"
    );
}

#[test]
fn journal_imported_message_fills_the_counts() {
    let summary = ImportSummary {
        fx_rates: 0,
        source_journal_id: Uuid::from_u128(1),
        source_logical_version: 7,
        studies: 3,
        watch_items: 2,
        portfolios: 1,
        holdings: 5,
        transactions: 4,
    };
    let msg = journal_imported_message(&summary);
    assert!(msg.contains("3 étude"));
    assert!(msg.contains("2 valeur"));
    assert!(msg.contains("5 ligne"));
    assert!(msg.contains("4 mouvement"));
}

// ── Story 5.4 — restore from backup ──

/// Create a standalone backup journal at `dir/<name>` with a chosen identity + an optional study,
/// then drop the handle (so it is a static file to inspect/restore).
fn make_backup(dir: &TempDir, name: &str, jid: u128, with_study: bool) {
    let path = dir.path().join(name);
    let mut j = Journal::create(
        &path,
        Uuid::from_u128(jid),
        &Timestamp("2026-06-20T00:00:00Z".to_string()),
    )
    .unwrap();
    if with_study {
        let s = Study::new(
            Uuid::from_u128(0xDA7A),
            Uuid::from_u128(jid),
            "ROG",
            "CHF",
            empty_judgment(),
            Timestamp("2026-06-20T00:00:00Z".to_string()),
        );
        j.put_study(&s).unwrap();
    }
    drop(j);
}

#[test]
fn request_restore_classifies_a_foreign_backup_and_parks_it() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x540); // live journal_id = 0xC0FFEE
    make_backup(&dir, "foreign.db", 0xBEEF, true);
    let assessment = state
        .request_restore(dir.path().join("foreign.db").to_str().unwrap())
        .unwrap();
    assert_eq!(assessment.verdict, RestoreVerdict::ForeignJournal);
    assert_eq!(assessment.journal_id, Uuid::from_u128(0xBEEF));
    assert!(
        state.has_pending_restore(),
        "a confirmable restore is parked"
    );
}

#[test]
fn request_restore_flags_an_older_same_journal_backup_as_stale() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x541);
    // Advance the live journal so it is newer than a fresh same-id backup (version 0).
    state.create_study("NESN", "CHF").unwrap();
    make_backup(&dir, "old.db", 0xC0FFEE, false); // same id, version 0
    let assessment = state
        .request_restore(dir.path().join("old.db").to_str().unwrap())
        .unwrap();
    assert!(
        matches!(assessment.verdict, RestoreVerdict::StaleOlder { backup: 0, current } if current >= 1),
        "an older same-journal backup is StaleOlder, got {:?}",
        assessment.verdict
    );
    // The confirm prompt surfaces the identity + the stale warning.
    let prompt = restore_confirm_message(&assessment);
    assert!(prompt.contains("plus ancienne"));
}

#[test]
fn request_restore_refuses_a_non_journal_file_and_parks_nothing() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x542);
    let garbage = dir.path().join("notjournal.txt");
    std::fs::write(&garbage, b"definitely not a sqlite journal").unwrap();
    let result = state.request_restore(garbage.to_str().unwrap());
    assert!(result.is_err(), "a non-journal file is refused");
    assert!(
        !state.has_pending_restore(),
        "a hard refusal parks no pending restore (confirm cannot fire)"
    );
}

#[test]
fn confirm_restore_swaps_the_live_journal_then_cancel_clears() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x543); // live id 0xC0FFEE, empty
    assert!(state.list_studies().is_empty());
    make_backup(&dir, "src.db", 0xBEEF, true); // foreign backup carrying one study

    // Cancel path: park then cancel → nothing applied.
    state
        .request_restore(dir.path().join("src.db").to_str().unwrap())
        .unwrap();
    state.cancel_restore();
    assert!(!state.has_pending_restore());
    assert!(state.list_studies().is_empty(), "cancel applied nothing");

    // Confirm path: park then confirm → the live journal becomes the backup.
    state
        .request_restore(dir.path().join("src.db").to_str().unwrap())
        .unwrap();
    state.confirm_restore().unwrap();
    assert_eq!(
        state.journal_id(),
        Some(Uuid::from_u128(0xBEEF)),
        "the live journal is now the restored backup"
    );
    assert_eq!(
        state.list_studies().len(),
        1,
        "the backup's study is now live"
    );
    assert!(!state.has_pending_restore(), "pending cleared");
}

#[test]
fn restoring_the_journal_onto_itself_is_a_safe_no_op() {
    // Review CRITICAL: fs::copy(live, live) truncates to 0 bytes — the same-path guard must make a
    // self-restore a no-op that loses nothing.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x544);
    let id = state.create_study("NESN", "CHF").unwrap();
    let live_path = dir.path().join("journal.db");
    state.request_restore(live_path.to_str().unwrap()).unwrap();
    state.confirm_restore().unwrap();
    assert_eq!(state.journal_id(), Some(Uuid::from_u128(0xC0FFEE)));
    assert!(state.get_study(id).is_some(), "the study was not zeroed");
}

#[test]
fn confirm_re_validates_and_refuses_a_tampered_backup_without_touching_the_journal() {
    // Review HIGH (TOCTOU): a backup validated at request time but replaced before confirm must be
    // re-checked — and a now-garbage file refused without overwriting the live journal.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x545); // live id 0xC0FFEE, empty
    let backup = dir.path().join("src.db");
    make_backup(&dir, "src.db", 0xBEEF, true);
    state.request_restore(backup.to_str().unwrap()).unwrap(); // ForeignJournal, parked
    std::fs::write(&backup, b"no longer a journal").unwrap(); // tamper after validation
    let result = state.confirm_restore();
    assert!(
        result.is_err(),
        "the re-validation refuses the tampered file"
    );
    assert_eq!(
        state.journal_id(),
        Some(Uuid::from_u128(0xC0FFEE)),
        "the live journal was not overwritten"
    );
    assert!(state.list_studies().is_empty(), "nothing was applied");
}

// ── Story 5.5 — journal location, recent journals & sync-safety ──

#[test]
fn is_sync_folder_matches_known_providers_and_rejects_a_plain_path() {
    assert!(is_sync_folder(Path::new(
        "/home/g/SynologyDrive/journal.db"
    )));
    assert!(is_sync_folder(Path::new("/home/g/Dropbox/sub/journal.db")));
    assert!(is_sync_folder(Path::new("/home/g/OneDrive/journal.db")));
    assert!(is_sync_folder(Path::new(
        "/Users/g/Library/Mobile Documents/journal.db"
    )));
    assert!(!is_sync_folder(Path::new(
        "/home/g/.local/share/steadyinvest/journal.db"
    )));
}

#[test]
fn open_and_create_journal_switch_between_journals() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x550); // journal A at dir/journal.db (id 0xC0FFEE)
    let id_in_a = state.create_study("NESN", "CHF").unwrap();

    // Create a second journal in a subdir → switches to it (empty).
    let sub = dir.path().join("other");
    std::fs::create_dir_all(&sub).unwrap();
    let outcome = state.create_journal(&sub, "second").unwrap();
    assert!(
        state.list_studies().is_empty(),
        "the new journal B is empty"
    );
    assert_eq!(state.journal_id(), Some(outcome.journal_id));
    assert!(
        !outcome.sync_warning,
        "a plain temp dir is not a sync folder"
    );

    // Open journal A back → its study is there (a clean switch round-trip).
    let path_a = dir.path().join("journal.db");
    state.open_journal(&path_a).unwrap();
    assert!(
        state.get_study(id_in_a).is_some(),
        "switched back to journal A"
    );
}

#[test]
fn open_journal_failure_leaves_the_previous_journal_open() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x551);
    state.create_study("NESN", "CHF").unwrap();

    // A journal held by a foreign, live process (forged lock with PID 1 = init).
    let sub = dir.path().join("locked");
    std::fs::create_dir_all(&sub).unwrap();
    let locked = sub.join("j.db");
    drop(
        Journal::create(
            &locked,
            Uuid::from_u128(0xBEEF),
            &Timestamp("2026-06-20T00:00:00Z".to_string()),
        )
        .unwrap(),
    );
    let mut lock = locked.as_os_str().to_os_string();
    lock.push("-lock");
    std::fs::write(&lock, "1").unwrap();

    let result = state.open_journal(&locked);
    assert_eq!(result, Err(MSG_JOURNAL_LOCKED.to_string()));
    // The previous journal stayed open with its study (never journal-less).
    assert_eq!(
        state.list_studies().len(),
        1,
        "the previous journal stayed open after a refused switch"
    );
}

#[test]
fn journal_stale_message_surfaces_both_versions() {
    let msg = journal_stale_message(57, 41);
    assert!(msg.contains("57"));
    assert!(msg.contains("41"));
}

#[test]
fn create_backup_lands_beside_the_journal() {
    // Review patch (5.4 deferral): backups follow the journal's location.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x552); // journal at dir/journal.db
    state.create_study("NESN", "CHF").unwrap();
    let backup = state.create_backup().unwrap();
    assert_eq!(
        backup.parent().unwrap(),
        dir.path().join("backups"),
        "the backup sits in a backups/ folder beside the journal"
    );
    assert!(backup.exists());
}

// ── Story 5.1 — confront (price-history cache + read-only overlay) ──

#[test]
fn a_refresh_caches_a_close_that_confront_reads_back() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x510); // FixedClock 2026-06-27
    let id = state.create_study("NESN", "CHF").unwrap();
    // A holdings price refresh caches today's close into the price-history trajectory.
    state
        .apply_holding_price(id, Decimal::from_str_exact("104.50").unwrap(), None)
        .unwrap();
    let view = state.confront(id);
    assert_eq!(
        view.actual,
        vec![(
            "2026-06-27".to_string(),
            Decimal::from_str_exact("104.5").unwrap()
        )],
        "confront reads back the cached post-decision close"
    );
    assert_eq!(view.decision_date, "2026-06-27");
}

/// Open a state whose fixed clock is `ts` (so `created_at`/`decision_date` predate a later session
/// date) — issue #72 tests need the cached session date to fall INSIDE the confront window.
fn dated_state(dir: &TempDir, seed: u128, ts: &str) -> JournalState {
    let path = dir.path().join("journal.db");
    if !path.exists() {
        drop(
            Journal::create(
                &path,
                Uuid::from_u128(0xC0FFEE),
                &Timestamp("2026-06-01T00:00:00Z".to_string()),
            )
            .unwrap(),
        );
    }
    let clock: Box<dyn Clock> = Box::new(FixedClock(Timestamp(ts.to_string())));
    let idgen: Box<dyn IdGen> = Box::new(crate::clock::SeqIdGen::starting_at(seed));
    let (state, _) = JournalState::open_or_create(Some(&path), clock, idgen);
    state
}

#[test]
fn a_refresh_keys_the_close_by_the_provider_session_date_not_the_clock_day() {
    // Issue #72: when the provider supplies a real EOD session date, the cached close is keyed by
    // THAT date, not the refresh (clock) day. Decision date 2026-06-20; the provider's session was
    // 2026-06-26 (a later real trading day) — the trajectory point reads under the session date.
    let dir = TempDir::new().unwrap();
    let mut state = dated_state(&dir, 0x513, "2026-06-20T15:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    state
        .apply_holding_price(
            id,
            Decimal::from_str_exact("104.50").unwrap(),
            Some("2026-06-26".to_string()),
        )
        .unwrap();
    let view = state.confront(id);
    assert_eq!(view.decision_date, "2026-06-20");
    assert_eq!(
        view.actual,
        vec![(
            "2026-06-26".to_string(),
            Decimal::from_str_exact("104.5").unwrap()
        )],
        "the close is keyed by the provider's session date (2026-06-26), not the clock day (2026-06-20)"
    );
}

#[test]
fn a_weekend_refetch_of_the_same_session_does_not_duplicate_the_close() {
    // Issue #72: two refreshes both reporting the same finalized session close → one trajectory
    // point, not one per calendar day (the pre-fix ordinal-axis duplication).
    let dir = TempDir::new().unwrap();
    let mut state = dated_state(&dir, 0x514, "2026-06-20T15:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let close = Decimal::from_str_exact("104.50").unwrap();
    state
        .apply_holding_price(id, close, Some("2026-06-26".to_string()))
        .unwrap();
    state
        .apply_holding_price(id, close, Some("2026-06-26".to_string()))
        .unwrap();
    assert_eq!(
        state.confront(id).actual.len(),
        1,
        "the same session's close is cached once, never duplicated across calendar days"
    );
    // A malformed provider date is rejected → the close falls back to the clock day (2026-06-20,
    // inside the window) rather than becoming a nonsense key, so the close is never lost.
    state
        .apply_holding_price(id, close, Some("not-a-date".to_string()))
        .unwrap();
    let dates: Vec<String> = state
        .confront(id)
        .actual
        .into_iter()
        .map(|(d, _)| d)
        .collect();
    assert!(
        dates.contains(&"2026-06-20".to_string()),
        "a malformed session date falls back to the clock day, keeping the close: {dates:?}"
    );
}

#[test]
fn confront_is_unavailable_with_no_cached_closes() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x511);
    let id = state.create_study("NESN", "CHF").unwrap();
    let view = state.confront(id);
    assert!(!view.available, "no cached closes → neutral empty state");
    assert!(view.actual.is_empty());
}

#[test]
fn confront_does_not_bump_the_version_read_only() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x512);
    let id = state.create_study("NESN", "CHF").unwrap();
    state
        .apply_holding_price(id, Decimal::from_str_exact("104.50").unwrap(), None)
        .unwrap();
    let before = state.logical_version_or_zero();
    let _ = state.confront(id);
    let _ = state.confront(id);
    assert_eq!(
        state.logical_version_or_zero(),
        before,
        "confront is strictly read-only — no journal write, no version bump"
    );
}

#[test]
fn reopening_the_currently_open_journal_is_a_no_op() {
    // Review patch (E7): re-selecting the open journal must not close+reopen (which would wipe undo).
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x553);
    let id = state.create_study("NESN", "CHF").unwrap();
    let path = dir.path().join("journal.db");
    let outcome = state.open_journal(&path).unwrap();
    assert_eq!(outcome.journal_id, Uuid::from_u128(0xC0FFEE));
    assert!(
        state.get_study(id).is_some(),
        "the journal stayed open, study intact"
    );
}

#[test]
fn provider_data_is_unvalidated_so_the_verdict_is_not_full() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x41, "2026-06-15T10:30:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    state
        .apply_provider_refresh(id, &fetched_for(&[2020, 2021, 2022, 2023, 2024]))
        .unwrap();

    let study = state.get_study(id).unwrap();
    let snapshot = engine::build_snapshot(&study).expect("normalizes");
    assert!(
        !matches!(
            snapshot.verdict(),
            steadyinvest_core::verdict::Verdict::Full(_)
        ),
        "unvalidated (Review::None) provider cells can never yield a Full verdict"
    );
}

#[test]
fn provider_fetch_does_not_overwrite_a_manual_value() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x42, "2026-06-15T11:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();

    // A manual edit on year 0, field "a" (high_price) — this also materializes the year grid.
    state.edit_cell(id, 0, "a", Some(und_money(999))).unwrap();
    let years: Vec<i32> = state
        .get_study(id)
        .unwrap()
        .years
        .iter()
        .map(|y| y.year)
        .collect();

    // Fetch covering those exact years; year-0 high_price is held manually, low_price is empty.
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();

    let study = state.get_study(id).unwrap();
    assert_eq!(
        study.years[0].high_price.value,
        Some(und_money(999)),
        "the manual value survives the fetch (fill-gaps-only)"
    );
    assert_eq!(study.years[0].high_price.source, Source::Manual);
    assert_eq!(
        study.years[0].low_price.source,
        Source::Provider,
        "the empty sibling cell was filled by the provider"
    );
}

// ── Story 3.3 — manual refresh: update / freshness / cause / idempotency ──

/// A refresh re-stamps a present **provider** cell whose value changed (new value + provenance
/// digest), and reports it as `updated` (not `filled`). (AC1/AC2)
#[test]
fn refresh_updates_a_changed_provider_cell() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x51, "2026-06-20T10:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];

    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    // Second refresh: high_price 100 → 200 (price diverges), everything else identical.
    let report = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 200, 50, "feed0042"))
        .unwrap();

    assert_eq!(report.filled, 0, "no gaps remain to fill");
    assert_eq!(report.updated, 5, "one high_price per year changed");
    assert!(
        report.cause.price && !report.cause.input,
        "only a price moved → price cause only"
    );

    let high = &state.get_study(id).unwrap().years[0].high_price;
    assert_eq!(high.value, Some(und_money(200)), "the new value is stamped");
    assert_eq!(high.source, Source::Provider);
    assert_eq!(
        high.provenance.hash_of_dependencies, "feed0042",
        "the cell carries the new fetch digest (re-stamped)"
    );
}

/// An identical re-fetch is a true no-op: nothing changes, the cause is empty, and **no phantom
/// undo step** is recorded (the timestamp-churn trap). (AC1 idempotency)
#[test]
fn idempotent_refresh_changes_nothing_and_records_no_undo_step() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x52, "2026-06-20T11:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];

    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    let depth_after_fill = state.undo_depth();
    assert_eq!(depth_after_fill, 1, "the first fill is one undo step");

    // Re-run the SAME refresh (same values, same digest) — must be a no-op.
    let report = state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    assert!(!report.changed(), "an identical re-fetch changes nothing");
    assert!(!report.cause.price && !report.cause.input);
    assert_eq!(
        state.undo_depth(),
        depth_after_fill,
        "a no-op refresh records no phantom undo step"
    );
}

/// A present **manual** cell is never overwritten by a refresh — even a divergent one (manual
/// wins; the divergent dual-value case is Story 3.4). (AC2)
#[test]
fn refresh_skips_a_manual_cell_even_when_divergent() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x53, "2026-06-20T12:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();

    // Manual high_price on year 0 (also materializes the grid).
    state.edit_cell(id, 0, "a", Some(und_money(999))).unwrap();
    let years: Vec<i32> = state
        .get_study(id)
        .unwrap()
        .years
        .iter()
        .map(|y| y.year)
        .collect();

    // Refresh with a DIVERGENT high_price (100 ≠ 999) for those years.
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();

    let high = &state.get_study(id).unwrap().years[0].high_price;
    assert_eq!(
        high.value,
        Some(und_money(999)),
        "the manual value stands; the divergent fetch never overwrites it"
    );
    assert_eq!(high.source, Source::Manual);
}

/// A deliberate "not available" decision (FR19) is never refilled by a refresh — neither a
/// load-bearing cell nor an optional one (it carries `value: None` but is a user choice, not a
/// gap). Regression guard for the code-review HIGH finding. (AC2)
#[test]
fn refresh_never_refills_a_not_available_accepted_cell() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x57, "2026-06-20T16:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();

    // Mark a load-bearing cell (year-0 sales) AND an optional cell (year-0 dividend, "f")
    // as not-available-accepted (this also materializes the grid).
    state
        .set_not_available(id, 0, entry::FIELD_SALES, true)
        .unwrap();
    state
        .set_not_available(id, 0, entry::FIELD_DIVIDEND, true)
        .unwrap();
    let years: Vec<i32> = state
        .get_study(id)
        .unwrap()
        .years
        .iter()
        .map(|y| y.year)
        .collect();

    // A refresh that supplies values for those exact cells must NOT refill them.
    let report = state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();

    let y0 = &state.get_study(id).unwrap().years[0];
    assert_eq!(
        y0.sales.coverage,
        Coverage::NotAvailableAccepted,
        "an N/A-accepted load-bearing cell is preserved, never refilled"
    );
    assert_eq!(y0.sales.value, None);
    assert_eq!(y0.sales.source, Source::Manual);
    // `fetched_for` leaves dividend absent, but assert the optional N/A slot is preserved anyway.
    assert!(
        y0.dividend_per_share
            .as_ref()
            .is_some_and(|c| c.coverage == Coverage::NotAvailableAccepted),
        "an N/A-accepted optional cell is preserved too"
    );
    // The empty sibling (low_price) was still filled — only the N/A decisions are protected.
    assert_eq!(y0.low_price.source, Source::Provider);
    assert!(report.filled > 0, "ordinary gaps still fill");
}

/// Issue #110 (b): a divergent refresh of a **validated** provider cell is FROZEN — the value AND the
/// `✓` are kept, the divergent provider value is parked as `pending` (a *contradiction*, surfaced by
/// notice), never demoted. A non-divergent re-fetch still keeps the human `✓`.
#[test]
fn refresh_freezes_a_divergent_validated_provider_cell() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x54, "2026-06-20T13:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];

    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    // The user reviews & validates year-0 high_price (a provider cell).
    state.set_review(id, 0, "a", Review::Validated).unwrap();
    assert_eq!(
        state.get_study(id).unwrap().years[0].high_price.review,
        Review::Validated
    );

    // A non-divergent re-fetch (same 100) keeps the ✓.
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    assert_eq!(
        state.get_study(id).unwrap().years[0].high_price.review,
        Review::Validated,
        "an equal re-fetch keeps the human ✓"
    );

    // Issue #110 (b): a divergent re-fetch (100 → 250) FREEZES the cell — the ✓ + value stay, the
    // divergent 250 is parked as pending (a contradiction the notice will flag).
    let report = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 250, 50, "beadfeed"))
        .unwrap();
    let high = &state.get_study(id).unwrap().years[0].high_price;
    assert_eq!(
        high.review,
        Review::Validated,
        "a validated cell keeps its ✓ (frozen)"
    );
    assert_eq!(
        high.value,
        Some(und_money(100)),
        "the checked value is frozen, not overwritten"
    );
    assert_eq!(
        high.pending.as_ref().and_then(|p| p.value),
        Some(und_money(250)),
        "the divergent provider value is parked as pending"
    );
    assert_eq!(
        report.contradicted, 1,
        "the frozen divergence is counted as a contradiction"
    );
}

/// The recompute cause distinguishes a pure-fundamental change from a pure-price change (FR29,
/// AC5) — driven through the real `apply_provider_refresh`, not a hand-built diff.
#[test]
fn refresh_classifies_input_only_vs_price_only_cause() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x55, "2026-06-20T14:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];

    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();

    // Only EPS moves (5 → 6): an input cause, no price cause.
    let input_only = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 6, 100, 50, "d1"))
        .unwrap();
    assert!(
        input_only.cause.input && !input_only.cause.price,
        "an EPS-only change is an input cause"
    );
    assert_eq!(refresh_notice(input_only), MSG_REFRESH_INPUT);

    // Only low_price moves (50 → 40): a price cause, no input cause.
    let price_only = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 6, 100, 40, "d2"))
        .unwrap();
    assert!(
        price_only.cause.price && !price_only.cause.input,
        "a price-only change is a price cause"
    );
    assert_eq!(refresh_notice(price_only), MSG_REFRESH_PRICE);
}

/// Issue #110 (b): a fully-validated provider study reads `Full`; a divergent refresh of a load-bearing
/// validated cell FREEZES it (value + `✓` kept), so the verdict STAYS `Full` — the checked study is not
/// destabilised by the provider later disagreeing (the divergence is parked as pending + surfaced by
/// notice, not applied). (Was: the divergence demoted the ✓ and degraded Full → Provisional pre-#110.)
#[test]
fn a_divergent_refresh_of_a_validated_cell_freezes_it_and_keeps_the_verdict() {
    use steadyinvest_core::verdict::Verdict;
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x56, "2026-06-20T15:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];

    // Fill from the provider, then the user validates every load-bearing year cell …
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    for y in 0..years.len() {
        for field in [
            entry::FIELD_SALES,
            entry::FIELD_HIGH,
            entry::FIELD_LOW,
            entry::FIELD_EPS,
        ] {
            state.set_review(id, y, field, Review::Validated).unwrap();
        }
    }
    // … and completes the judgment (the five load-bearing judgment inputs).
    for (field, v) in [
        ("est_high_eps", 8),
        ("est_low_eps", 6),
        ("high_pe", 20),
        ("low_pe", 10),
        ("current_price", 60),
    ] {
        state
            .set_judgment_field(id, field, Some(und_money(v)))
            .unwrap();
    }

    let study = state.get_study(id).unwrap();
    assert!(
        matches!(
            engine::build_snapshot(&study)
                .expect("normalizes")
                .verdict(),
            Verdict::Full(_)
        ),
        "an all-validated provider study with a complete judgment reads Full"
    );

    // Issue #110 (b): a divergent refresh of the (validated, provider) high_price FREEZES it …
    let report = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 250, 50, "deg"))
        .unwrap();
    let study = state.get_study(id).unwrap();
    assert_eq!(
        study.years[0].high_price.review,
        Review::Validated,
        "the validated cell keeps its ✓ (frozen) — the provider disagreement does not demote it"
    );
    assert_eq!(
        report.contradicted, 5,
        "all 5 years' validated high_price are contradicted (frozen), not demoted"
    );
    assert!(
        matches!(
            engine::build_snapshot(&study)
                .expect("normalizes")
                .verdict(),
            Verdict::Full(_)
        ),
        "the frozen ✓ keeps the load-bearing input valid → the verdict STAYS Full"
    );
}

// ── Story 3.4 — non-destructive reconciliation ──

/// Set up a study with a single manual, validated high_price cell that the provider will
/// diverge from. Returns (state, id, years).
fn study_with_validated_manual_high(
    dir: &TempDir,
    seed: u128,
    manual_high: i64,
) -> (JournalState, Uuid, Vec<i32>) {
    let mut state = undo_state(dir, seed, "2026-06-27T10:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    // Manual high_price on year 0 (materializes the grid), then validate it.
    state
        .edit_cell(id, 0, entry::FIELD_HIGH, Some(und_money(manual_high)))
        .unwrap();
    state
        .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
        .unwrap();
    let years: Vec<i32> = state
        .get_study(id)
        .unwrap()
        .years
        .iter()
        .map(|y| y.year)
        .collect();
    (state, id, years)
}

/// A divergent refresh of a validated MANUAL cell: the manual value stands, the provider value is
/// preserved alongside (pending), never merged. Issue #110 (b): the `✓` is KEPT (frozen), and the
/// divergence is flagged as a contradiction — the checked manual value is not destabilised. (AC1/2/3)
#[test]
fn refresh_reconciles_a_divergent_manual_cell_non_destructively() {
    let dir = TempDir::new().unwrap();
    let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x60, 999);

    // Provider diverges on high_price (100 ≠ 999).
    let report = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "recon"))
        .unwrap();
    assert!(
        report.reconciled >= 1,
        "the manual divergence is reconciled"
    );
    assert!(report.changed(), "a reconciliation is a change");

    assert_eq!(
        report.contradicted, 1,
        "the frozen validated divergence is a contradiction (#110 b)"
    );
    let high = &state.get_study(id).unwrap().years[0].high_price;
    assert_eq!(high.value, Some(und_money(999)), "manual value stands");
    assert_eq!(high.source, Source::Manual);
    assert_eq!(
        high.review,
        Review::Validated,
        "issue #110: the ✓ is FROZEN, not demoted"
    );
    let pending = high
        .pending
        .as_ref()
        .expect("the provider value is preserved");
    assert_eq!(pending.value, Some(und_money(100)));
    assert_eq!(pending.provenance.source, Source::Provider);
}

/// An agreeing refresh on a manual cell records no pending and keeps `✓` — and an identical
/// re-run is a no-op (idempotency, no phantom undo step). (AC1)
#[test]
fn refresh_agreement_on_a_manual_cell_keeps_validation_and_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x61, 100);

    // Provider AGREES with the manual high_price (100 == 100).
    state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "agree"))
        .unwrap();
    let high = &state.get_study(id).unwrap().years[0].high_price;
    assert_eq!(high.review, Review::Validated, "agreement keeps ✓");
    assert!(high.pending.is_none(), "no divergence → no pending");

    let depth = state.undo_depth();
    // Re-run the same agreeing refresh — a true no-op.
    let report = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "agree"))
        .unwrap();
    assert_eq!(report.reconciled, 0);
    assert_eq!(
        state.undo_depth(),
        depth,
        "an agreeing re-refresh records no phantom undo step"
    );
}

/// Accept-provider resolution: the cell takes the pending provider value (Source::Provider,
/// Review::ToReview, pending cleared). (AC4)
#[test]
fn accept_provider_value_takes_the_pending_and_clears_it() {
    let dir = TempDir::new().unwrap();
    let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x62, 999);
    state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "recon"))
        .unwrap();

    state
        .accept_provider_value(id, 0, entry::FIELD_HIGH)
        .unwrap();
    let high = &state.get_study(id).unwrap().years[0].high_price;
    assert_eq!(
        high.value,
        Some(und_money(100)),
        "the provider value is taken"
    );
    assert_eq!(high.source, Source::Provider);
    assert_eq!(high.review, Review::ToReview, "re-check the accepted value");
    assert!(high.pending.is_none(), "the pending is cleared");
}

/// Keep-manual resolution: the manual value stands, only the pending is dismissed. (AC4)
#[test]
fn keep_manual_value_dismisses_the_pending_only() {
    let dir = TempDir::new().unwrap();
    let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x63, 999);
    state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "recon"))
        .unwrap();

    state.keep_manual_value(id, 0, entry::FIELD_HIGH).unwrap();
    let high = &state.get_study(id).unwrap().years[0].high_price;
    assert_eq!(high.value, Some(und_money(999)), "manual value stands");
    assert_eq!(high.source, Source::Manual);
    assert!(high.pending.is_none(), "the pending is dismissed");
}

/// Re-validating a cell with a pending clears the pending (the user reconciled). (AC4)
#[test]
fn revalidating_a_cell_clears_its_pending() {
    let dir = TempDir::new().unwrap();
    let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x64, 999);
    state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "recon"))
        .unwrap();
    // The divergence demoted it to ?; the user re-validates their kept value.
    state
        .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
        .unwrap();
    let high = &state.get_study(id).unwrap().years[0].high_price;
    assert_eq!(high.review, Review::Validated);
    assert!(high.pending.is_none(), "re-validating clears the pending");
}

/// AC6 guard: the engine ignores `pending` — a cell carrying a pending yields the SAME frame as
/// the same cell with `pending = None`.
#[test]
fn the_engine_ignores_a_pending_divergence() {
    let dir = TempDir::new().unwrap();
    let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x65, 999);
    state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "recon"))
        .unwrap();
    let mut with_pending = state.get_study(id).unwrap();
    assert!(
        with_pending.years[0].high_price.pending.is_some(),
        "precondition: a pending exists"
    );
    let frame_with = engine::build_snapshot(&with_pending).expect("normalizes");

    // Strip the pending and rebuild — the verdict frame must be identical.
    with_pending.years[0].high_price.pending = None;
    let frame_without = engine::build_snapshot(&with_pending).expect("normalizes");
    assert_eq!(
        frame_with.verdict(),
        frame_without.verdict(),
        "the engine reads only the live value, never `pending`"
    );
}

/// A pending divergence survives a journal close + reopen (AC5 — NFR-R4 "preserved").
#[test]
fn a_pending_divergence_survives_reopen() {
    let dir = TempDir::new().unwrap();
    let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x66, 999);
    state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "recon"))
        .unwrap();
    drop(state);

    // Reopen the journal from disk and confirm the pending is intact.
    let reopened = open_state(&dir.path().join("journal.db"));
    let high = reopened.get_study(id).unwrap().years[0].high_price.clone();
    assert_eq!(
        high.value,
        Some(und_money(999)),
        "manual value survives reopen"
    );
    assert_eq!(
        high.review,
        Review::Validated,
        "issue #110: the frozen ✓ survives reopen"
    );
    let pending = high.pending.expect("the pending survives reopen");
    assert_eq!(pending.value, Some(und_money(100)));
}

/// accept/keep on a cell with NO pending is a true no-op — no undo step, no journal write
/// (the resolve buttons can linger; re-clicking them must not churn the journal). (review fix)
#[test]
fn accept_or_keep_with_no_pending_is_a_true_noop() {
    let dir = TempDir::new().unwrap();
    let (mut state, id, _years) = study_with_validated_manual_high(&dir, 0x67, 999);
    let depth = state.undo_depth();
    let version = state.logical_version();

    state
        .accept_provider_value(id, 0, entry::FIELD_HIGH)
        .unwrap();
    state.keep_manual_value(id, 0, entry::FIELD_HIGH).unwrap();

    assert_eq!(state.undo_depth(), depth, "no pending → no undo step");
    assert_eq!(
        state.logical_version(),
        version,
        "no pending → no journal revision (no phantom logical_version bump)"
    );
}

/// A repeated DIVERGENT refresh (same provider value, a later fetch timestamp) is idempotent —
/// the pending is not re-stamped, so no phantom undo step accrues. (review fix)
#[test]
fn a_repeated_divergent_refresh_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let (mut state, id, years) = study_with_validated_manual_high(&dir, 0x68, 999);
    state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "fetch-a"))
        .unwrap();
    let depth = state.undo_depth();

    // Re-fetch the SAME divergent value with a DIFFERENT digest (a later fetch) — a no-op.
    let report = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 100, 50, "fetch-b"))
        .unwrap();
    assert_eq!(
        report.reconciled, 0,
        "the same divergence is not re-reconciled"
    );
    assert_eq!(
        state.undo_depth(),
        depth,
        "a repeated divergence records no phantom undo step"
    );
}

// ── Story 3.5 — graceful provider failure ──

#[test]
fn provider_failure_notice_maps_each_cause() {
    use steadyinvest_ingestion::{IngestionError, ProviderError};
    let p = |e: ProviderError| provider_failure_notice(&IngestionError::Provider(e));
    assert_eq!(
        p(ProviderError::Network {
            detail: "dns".into()
        }),
        MSG_PROVIDER_OFFLINE
    );
    assert_eq!(
        p(ProviderError::Quota {
            retry_after_secs: Some(60)
        }),
        MSG_PROVIDER_QUOTA
    );
    assert_eq!(p(ProviderError::InvalidOrAbsentKey), MSG_KEY_INVALID);
    assert_eq!(
        p(ProviderError::Forbidden {
            detail: "plan".into()
        }),
        MSG_KEY_FORBIDDEN
    );
    assert_eq!(
        p(ProviderError::TickerNotFound {
            ticker: "AAPL.US".into()
        }),
        MSG_PROVIDER_NO_DATA
    );
    assert_eq!(
        p(ProviderError::Parse {
            detail: "shape".into()
        }),
        MSG_NORMALIZE_FAILED
    );
    let normalize = IngestionError::Normalize(
        steadyinvest_core::normalize::NormalizeError::DuplicateYear { year: 2020 },
    );
    assert_eq!(provider_failure_notice(&normalize), MSG_NORMALIZE_FAILED);
}

#[test]
fn mark_provider_stale_flags_provider_cells_and_retains_values() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x70, "2026-06-27T10:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();

    let flagged = state.mark_provider_stale(id).unwrap();
    assert_eq!(
        flagged, 20,
        "5 years × 4 load-bearing provider cells flagged"
    );
    let high = &state.get_study(id).unwrap().years[0].high_price;
    assert_eq!(high.freshness, Freshness::Stale);
    assert_eq!(
        high.value,
        Some(und_money(100)),
        "the last-known value is retained (NFR-R1)"
    );
    assert_eq!(high.source, Source::Provider);
}

#[test]
fn mark_provider_stale_leaves_manual_cells_current_and_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x71, "2026-06-27T10:30:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    // A manual high_price on year 0 (materializes the grid); the rest are empty (manual to-fill).
    state.edit_cell(id, 0, "a", Some(und_money(999))).unwrap();

    let flagged = state.mark_provider_stale(id).unwrap();
    assert_eq!(flagged, 0, "a study with no provider cells flags nothing");
    assert_eq!(
        state.get_study(id).unwrap().years[0].high_price.freshness,
        Freshness::Current,
        "a manual cell is never flagged stale (the user owns it)"
    );

    // Now fill the rest from the provider, flag, then RE-flag — the second is a no-op.
    let years: Vec<i32> = state
        .get_study(id)
        .unwrap()
        .years
        .iter()
        .map(|y| y.year)
        .collect();
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    state.mark_provider_stale(id).unwrap();
    let depth = state.undo_depth();
    let version = state.logical_version();
    let again = state.mark_provider_stale(id).unwrap();
    assert_eq!(again, 0, "already-stale cells are not re-flagged");
    assert_eq!(
        state.undo_depth(),
        depth,
        "an idempotent re-flag records no phantom undo step"
    );
    assert_eq!(
        state.logical_version(),
        version,
        "an idempotent re-flag writes no journal revision (no version bump)"
    );
    // The manually-held cell is still Current after both flags.
    assert_eq!(
        state.get_study(id).unwrap().years[0].high_price.source,
        Source::Manual
    );
    assert_eq!(
        state.get_study(id).unwrap().years[0].high_price.freshness,
        Freshness::Current
    );
}

/// A failed refresh that flags a validated provider study stale degrades the verdict to
/// Provisional in the same frame (the production path through `mark_provider_stale`). (AC3)
#[test]
fn a_stale_flag_degrades_a_full_verdict_to_provisional() {
    use steadyinvest_core::verdict::Verdict;
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x72, "2026-06-27T11:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    for y in 0..years.len() {
        for field in [
            entry::FIELD_SALES,
            entry::FIELD_HIGH,
            entry::FIELD_LOW,
            entry::FIELD_EPS,
        ] {
            state.set_review(id, y, field, Review::Validated).unwrap();
        }
    }
    for (field, v) in [
        ("est_high_eps", 8),
        ("est_low_eps", 6),
        ("high_pe", 20),
        ("low_pe", 10),
        ("current_price", 60),
    ] {
        state
            .set_judgment_field(id, field, Some(und_money(v)))
            .unwrap();
    }
    assert!(
        matches!(
            engine::build_snapshot(&state.get_study(id).unwrap())
                .unwrap()
                .verdict(),
            Verdict::Full(_)
        ),
        "precondition: a validated provider study reads Full"
    );

    // A failed refresh flags the provider cells stale → the validated inputs degrade.
    state.mark_provider_stale(id).unwrap();
    assert!(
        matches!(
            engine::build_snapshot(&state.get_study(id).unwrap())
                .unwrap()
                .verdict(),
            Verdict::Provisional(_)
        ),
        "a stale validated load-bearing input degrades Full → Provisional"
    );
}

/// A later successful refresh re-confirms currency and clears the stale flag, even when the
/// provider returns the SAME values. (AC2 lifecycle)
#[test]
fn a_successful_refresh_clears_the_stale_flag() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x73, "2026-06-27T11:30:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    state.mark_provider_stale(id).unwrap();
    assert_eq!(
        state.get_study(id).unwrap().years[0].high_price.freshness,
        Freshness::Stale
    );

    // The same data comes back on a successful retry — currency confirmed → Current again.
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    assert_eq!(
        state.get_study(id).unwrap().years[0].high_price.freshness,
        Freshness::Current,
        "a successful refresh clears the stale flag (even on unchanged values)"
    );
}

/// A successful refresh that covers only a SUBSET of the grid's years still clears the stale flag
/// on the years it omits — the outage is over, so the recovery is study-wide, not per-fetched-cell
/// (review-fix: a year/field the fetch omits must not stay stale forever). (AC2)
#[test]
fn a_successful_refresh_clears_stale_on_years_it_omits() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x74, "2026-06-27T12:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let all_years = [2020, 2021, 2022, 2023, 2024];
    state
        .apply_provider_refresh(id, &fetched_for(&all_years))
        .unwrap();
    state.mark_provider_stale(id).unwrap();

    // A narrower successful refresh (only the last 3 years) — the omitted 2020/2021 must recover.
    state
        .apply_provider_refresh(id, &fetched_for(&[2022, 2023, 2024]))
        .unwrap();
    let study = state.get_study(id).unwrap();
    let year_2020 = study.years.iter().find(|y| y.year == 2020).unwrap();
    assert_eq!(
        year_2020.high_price.freshness,
        Freshness::Current,
        "a year the successful fetch omitted still recovers from stale (outage over)"
    );
}

// ── Story 3.6 — annual update journey ──

/// Issue #110 (b): the `contradicted` count is the number of validated (✓) cells the provider now
/// disagrees with — frozen (value + ✓ kept, pending parked), NOT demoted. A second identical refresh
/// is idempotent (the pending already matches) → 0.
#[test]
fn contradicted_counts_the_frozen_validated_cells_the_provider_disagrees_with() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x80, "2026-06-27T13:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    // Validate every high_price; leave the rest unvalidated.
    for y in 0..years.len() {
        state.set_review(id, y, "a", Review::Validated).unwrap();
    }
    // Refresh: high_price 100 → 200 diverges. The 5 validated ✓ cells FREEZE (value 100 kept, pending
    // 200 parked); eps/sales/low unchanged.
    let report = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 200, 50, "annual"))
        .unwrap();
    assert_eq!(
        report.contradicted, 5,
        "the 5 frozen validated high_price cells the provider disagrees with"
    );
    let high0 = &state.get_study(id).unwrap().years[0].high_price;
    assert_eq!(high0.review, Review::Validated, "frozen — the ✓ stays");
    assert_eq!(
        high0.value,
        Some(und_money(100)),
        "frozen — the checked value stays"
    );
    // A second identical refresh contradicts nothing new (the pending already matches) → 0.
    let again = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 200, 50, "annual"))
        .unwrap();
    assert_eq!(
        again.contradicted, 0,
        "an idempotent re-fetch adds no new contradiction"
    );
}

#[test]
fn refresh_summary_appends_the_contradiction_clause_only_when_needed() {
    let no_contradiction = RefreshReport {
        updated: 1,
        cause: crate::viewmodel::refresh::RefreshCause {
            price: true,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        refresh_summary(no_contradiction),
        refresh_notice(no_contradiction),
        "with no contradictions the summary is exactly the cause notice (no regression)"
    );
    let with_contradiction = RefreshReport {
        contradicted: 3,
        ..no_contradiction
    };
    let summary = refresh_summary(with_contradiction);
    assert!(summary.starts_with(refresh_notice(with_contradiction)));
    assert!(
        summary.contains("3 cellule(s) validée(s)"),
        "the contradiction is named: {summary}"
    );
}

/// The Journey-2b ritual end-to-end through the real rails: reopen a saved validated study, re-fetch
/// new annual data, and confirm manual + judgment preserved, changed ✓ → ?, unchanged ✓ kept, the
/// contradiction count correct (#110 b: validated cells freeze, not demote), and the projection
/// extends. (AC1, AC2, AC3, AC4)
#[test]
fn the_annual_update_journey_preserves_manual_and_judgment_and_freezes_the_validated() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x81, "2026-06-27T13:30:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];

    // A saved study: provider-fetched, with a MANUAL override on year-0 sales, fully validated.
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    state
        .edit_cell(id, 0, entry::FIELD_SALES, Some(und_money(5000)))
        .unwrap();
    for y in 0..years.len() {
        for field in [
            entry::FIELD_SALES,
            entry::FIELD_HIGH,
            entry::FIELD_LOW,
            entry::FIELD_EPS,
        ] {
            state.set_review(id, y, field, Review::Validated).unwrap();
        }
    }
    for (field, v) in [
        ("est_high_eps", 8),
        ("est_low_eps", 6),
        ("high_pe", 20),
        ("low_pe", 10),
        ("current_price", 60),
    ] {
        state
            .set_judgment_field(id, field, Some(und_money(v)))
            .unwrap();
    }
    let judgment_before = state.get_study(id).unwrap().judgment;

    // A year later: the annual report lands. high_price 100 → 200 (diverges, provider cells);
    // sales 1000 (year-0 sales is held manually at 5000 → diverges → reconcile); eps/low agree.
    let report = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 200, 50, "annual-2027"))
        .unwrap();

    let study = state.get_study(id).unwrap();
    // AC1 — the manual value stands (never overwritten); the judgment is untouched.
    let y0 = &study.years[0];
    assert_eq!(
        y0.sales.value,
        Some(und_money(5000)),
        "manual sales preserved"
    );
    assert_eq!(y0.sales.source, Source::Manual);
    assert_eq!(
        y0.sales.pending.as_ref().map(|p| p.value),
        Some(Some(und_money(1000))),
        "the divergent provider sales is preserved alongside (Story 3.4)"
    );
    assert_eq!(study.judgment, judgment_before, "judgment lines preserved");
    // AC2 (#110 b) — a validated cell the provider contradicts is FROZEN: value + ✓ kept, provider
    // value parked as pending; an unchanged one keeps ✓ too. Nothing is demoted.
    assert_eq!(
        y0.high_price.review,
        Review::Validated,
        "contradicted high stays ✓ (frozen)"
    );
    assert_eq!(
        y0.high_price.value,
        Some(und_money(100)),
        "the checked high value is frozen"
    );
    assert_eq!(
        y0.high_price.pending.as_ref().and_then(|p| p.value),
        Some(und_money(200)),
        "the divergent provider high is parked as pending"
    );
    assert_eq!(y0.eps.review, Review::Validated, "unchanged eps keeps ✓");
    assert_eq!(
        y0.sales.review,
        Review::Validated,
        "contradicted manual sales stays ✓ (frozen)"
    );
    // AC3 — the contradiction scope: 5 high_price + the 1 manual sales the provider disagrees with = 6.
    assert_eq!(
        report.contradicted, 6,
        "the provider contradicts 6 validated cells"
    );
    assert!(refresh_summary(report).contains("6 cellule(s) validée(s)"));

    // AC4 — extend the projection: the new fiscal year row appends, prior years intact.
    let max_before = study.years.iter().map(|y| y.year).max().unwrap();
    state.extend_history(id).unwrap();
    let extended = state.get_study(id).unwrap();
    assert_eq!(
        extended.years.iter().map(|y| y.year).max().unwrap(),
        max_before + 1,
        "the projection extends by one fiscal year"
    );
    assert_eq!(
        extended.years[0].sales.value,
        Some(und_money(5000)),
        "extending leaves the existing years intact"
    );
}

/// AC5 — the "unlock all → re-fetch" path: after unlocking, a refresh demotes nothing (nothing
/// was ✓) and the manual values are still preserved.
#[test]
fn unlock_all_then_refresh_demotes_nothing_and_preserves_manual() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x82, "2026-06-27T14:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    state
        .edit_cell(id, 0, entry::FIELD_SALES, Some(und_money(5000)))
        .unwrap();
    for y in 0..years.len() {
        state.set_review(id, y, "a", Review::Validated).unwrap();
    }
    // Unlock the whole study, THEN refresh with divergent data.
    state.unlock_all(id, &UnlockScope::Study).unwrap();
    let report = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 200, 50, "annual"))
        .unwrap();
    assert_eq!(
        report.contradicted, 0,
        "nothing was ✓ after unlock → no frozen validated cell to contradict"
    );
    assert_eq!(
        state.get_study(id).unwrap().years[0].sales.value,
        Some(und_money(5000)),
        "the manual value is still preserved after unlock + refresh"
    );
}

// ── Story 4.1 — watchlist app rails ──

fn watch_id(state: &JournalState, ticker: &str) -> Uuid {
    state
        .list_watch_items()
        .into_iter()
        .find(|w| w.security_ticker == ticker)
        .map(|w| w.id)
        .expect("the watch item exists")
}

#[test]
fn watchlist_add_list_move_delete_through_the_app_rails() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x900);
    state.add_watch_item("NESN", None).unwrap();
    state.add_watch_item("ROG", None).unwrap();
    state.add_watch_item("NOVN", None).unwrap();
    let order = |s: &JournalState| {
        s.list_watch_items()
            .into_iter()
            .map(|w| w.security_ticker)
            .collect::<Vec<_>>()
    };
    assert_eq!(order(&state), ["NESN", "ROG", "NOVN"]);

    // Move ROG up → ROG, NESN, NOVN.
    let rog = watch_id(&state, "ROG");
    state.move_watch_item(rog, true).unwrap();
    assert_eq!(order(&state), ["ROG", "NESN", "NOVN"]);

    // Move ROG up again at the top edge → no-op.
    state.move_watch_item(rog, true).unwrap();
    assert_eq!(order(&state), ["ROG", "NESN", "NOVN"]);

    // Delete NESN → re-packed contiguous.
    state.delete_watch_item(watch_id(&state, "NESN")).unwrap();
    assert_eq!(order(&state), ["ROG", "NOVN"]);
    assert_eq!(
        state
            .list_watch_items()
            .into_iter()
            .map(|w| w.position)
            .collect::<Vec<_>>(),
        [0, 1]
    );
}

#[test]
fn add_watch_blank_ticker_is_refused_and_link_round_trips() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x910);
    assert!(
        state.add_watch_item("   ", None).is_err(),
        "blank ticker refused"
    );

    let study = state.create_study("NESN", "CHF").unwrap();
    state.add_watch_item("NESN", Some(study)).unwrap();
    assert_eq!(
        state.list_watch_items()[0].study_id,
        Some(study),
        "the study link round-trips through the app rail"
    );
    // Clearing it via update.
    let wid = watch_id(&state, "NESN");
    state.update_watch_item(wid, "NESN", None).unwrap();
    assert_eq!(state.list_watch_items()[0].study_id, None);
}

#[test]
fn study_id_for_ticker_matches_case_insensitively_and_picks_most_recent() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x920);
    let first = state.create_study("NESN", "CHF").unwrap();
    let second = state.create_study("NESN", "CHF").unwrap();
    // A lowercase watched ticker still resolves to the (most recent) "NESN" study.
    assert_eq!(
        state.study_id_for_ticker("nesn"),
        Some(second),
        "case-insensitive + most-recent"
    );
    assert_ne!(state.study_id_for_ticker("nesn"), Some(first));
    assert_eq!(state.study_id_for_ticker("UNKNOWN"), None);
}

/// Issue #81: the holdings auto-match never crosses currencies — a CHF holding resolves the CHF study
/// of the ticker, a USD holding the USD study; a currency with no study yields NO match (safer than a
/// wrong-currency one); a holding with no declared currency falls back to the ticker-only match.
#[test]
fn study_id_for_ticker_in_currency_never_crosses_currencies() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x921);
    let chf = state.create_study("AAPL", "CHF").unwrap();
    let usd = state.create_study("AAPL", "USD").unwrap();
    assert_eq!(
        state.study_id_for_ticker_in_currency("aapl", Some("CHF")),
        Some(chf),
        "a CHF holding matches the CHF study"
    );
    assert_eq!(
        state.study_id_for_ticker_in_currency("aapl", Some("USD")),
        Some(usd),
        "a USD holding matches the USD study"
    );
    assert_eq!(
        state.study_id_for_ticker_in_currency("aapl", Some("EUR")),
        None,
        "no EUR study → no match, never a cross-currency one"
    );
    assert_eq!(
        state.study_id_for_ticker_in_currency("aapl", None),
        Some(usd),
        "no declared currency → ticker-only fallback (most recent)"
    );
}

// ── Story 4.2 — buy-zone alert (the app-surface read of the engine zone) ──

#[test]
fn study_in_buy_zone_reflects_the_current_price_and_is_verdict_independent() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x95, "2026-06-27T16:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];
    // Provider-fill (cells stay Review::None → the verdict is NOT Full) + a complete judgment so
    // the §4 forecast band exists; est_low_eps 6 × low_pe 10 ⇒ forecast low ≈ 60.
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    // Forecast band ≈ [low 50–60, high 160] (high = est_high_eps 8 × high_pe 20). The buy third
    // is ≈ [low, 93] (buy_top = low + (high − low)/3). A current_price of 70 sits in it.
    for (field, v) in [
        ("est_high_eps", 8),
        ("est_low_eps", 6),
        ("high_pe", 20),
        ("low_pe", 10),
        ("current_price", 70),
    ] {
        state
            .set_judgment_field(id, field, Some(und_money(v)))
            .unwrap();
    }
    // The verdict is NOT Full (unvalidated provider cells), yet the buy-zone fact still holds
    // (AC6 — verdict-independent).
    assert!(
        engine::study_in_buy_zone(&state.get_study(id).unwrap()),
        "a current price in the bottom third of the band is in the buy zone, regardless of verdict"
    );

    // Move the price into the upper band (sell third) → not in the buy zone.
    state
        .set_judgment_field(id, "current_price", Some(und_money(150)))
        .unwrap();
    assert!(!engine::study_in_buy_zone(&state.get_study(id).unwrap()));

    // No current price → no defined zone → not in the buy zone.
    state.set_judgment_field(id, "current_price", None).unwrap();
    assert!(!engine::study_in_buy_zone(&state.get_study(id).unwrap()));
}

// ── Story 4.4 — manual price refresh fills current_price from the latest close ──

#[test]
fn provider_refresh_fills_current_price_from_latest_close_and_moves_the_zone() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x4C, "2026-06-27T16:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];
    // Provider-fill the yearly cells, then a complete forecast band — but NO current_price yet, so
    // the §4 zone is undefined (band ≈ [low 60, high 160]; buy third ≈ [60, 93]).
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    for (field, v) in [
        ("est_high_eps", 8),
        ("est_low_eps", 6),
        ("high_pe", 20),
        ("low_pe", 10),
    ] {
        state
            .set_judgment_field(id, field, Some(und_money(v)))
            .unwrap();
    }
    assert!(
        state
            .get_study(id)
            .unwrap()
            .judgment
            .current_price
            .is_none(),
        "no current_price yet → no defined zone"
    );
    assert!(!engine::study_in_buy_zone(&state.get_study(id).unwrap()));

    // A refresh carrying a latest close of 70 sets current_price (a market fact, AC6) and the buy
    // third ≈ [60, 93] now brackets it → in the buy zone, verdict-independent.
    state
        .apply_provider_refresh(id, &fetched_with_price(&years, 70))
        .unwrap();
    assert_eq!(
        state.get_study(id).unwrap().judgment.current_price,
        Some(und_money(70)),
        "the latest /eod close fills current_price"
    );
    assert!(
        engine::study_in_buy_zone(&state.get_study(id).unwrap()),
        "current_price 70 sits in the buy third → in the buy zone"
    );

    // A later refresh with no latest price (the pre-4.4 shape) leaves current_price untouched.
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    assert_eq!(
        state.get_study(id).unwrap().judgment.current_price,
        Some(und_money(70)),
        "latest_price = None must not clear the last-known current_price"
    );
}

#[test]
fn study_zone_reports_the_full_buy_neutral_sell_zone_for_holdings() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x4D, "2026-06-27T16:00:00Z");
    let id = state.create_study("ROG", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    for (field, v) in [
        ("est_high_eps", 8),
        ("est_low_eps", 6),
        ("high_pe", 20),
        ("low_pe", 10),
    ] {
        state
            .set_judgment_field(id, field, Some(und_money(v)))
            .unwrap();
    }
    // Band ≈ [60, 160]; thirds: buy ≤ ~93, neutral ~93–127, sell ≥ ~127. The holdings register
    // reads the FULL zone (Achat/Neutre/Vente), not just "in the buy zone".
    let zone = |st: &JournalState| engine::zone_key(engine::study_zone(&st.get_study(id).unwrap()));
    assert_eq!(zone(&state), "", "no current_price yet → undefined zone");
    for (price, expected) in [(70, "buy"), (110, "neutral"), (150, "sell")] {
        state
            .set_judgment_field(id, "current_price", Some(und_money(price)))
            .unwrap();
        assert_eq!(zone(&state), expected, "current_price {price}");
    }
    // A price outside `[forecast_low, forecast_high]` has no defined zone (the register shows "—").
    state
        .set_judgment_field(id, "current_price", Some(und_money(300)))
        .unwrap();
    assert_eq!(zone(&state), "", "a price above the band → no zone");
}

#[test]
fn apply_holding_price_sets_current_price_only_and_moves_the_zone() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x4E, "2026-06-27T16:00:00Z");
    let id = state.create_study("ROG", "CHF").unwrap();
    let years = [2020, 2021, 2022, 2023, 2024];
    state
        .apply_provider_refresh(id, &fetched_for(&years))
        .unwrap();
    for (field, v) in [
        ("est_high_eps", 8),
        ("est_low_eps", 6),
        ("high_pe", 20),
        ("low_pe", 10),
    ] {
        state
            .set_judgment_field(id, field, Some(und_money(v)))
            .unwrap();
    }
    // Snapshot the yearly cells to prove the price-only refresh (issue #50) leaves them untouched.
    let before_years = state.get_study(id).unwrap().years.clone();

    state
        .apply_holding_price(id, rust_decimal::Decimal::new(70, 0), None)
        .unwrap();

    let after = state.get_study(id).unwrap();
    assert_eq!(
        after.judgment.current_price,
        Some(und_money(70)),
        "the price-only fill sets current_price"
    );
    assert_eq!(
        after.years, before_years,
        "a price-only holding refresh must NOT touch the yearly provider cells"
    );
    assert!(
        engine::study_in_buy_zone(&after),
        "price 70 sits in the buy third → the zone recomputes"
    );
}

// ── Story 4.5 — trailing stop per holding (validate, seed, ratchet) ──

#[test]
fn set_holding_trailing_stop_validates_seeds_from_purchase_price_and_clears() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x55, "2026-06-28T10:00:00Z");
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;

    // Out-of-range / non-numeric pct → refused, nothing written.
    for bad in ["0", "100", "150", "-5", "abc", "1.2.3"] {
        assert_eq!(
            state.set_holding_trailing_stop(id, bad),
            Err(MSG_HOLDING_INVALID_STOP.to_string()),
            "pct {bad:?} is refused"
        );
    }
    assert!(state.list_holdings()[0].trailing_stop_pct.is_none());

    // No linked study → the level seeds from the purchase price 100: 100 × (1 − 0.15) = 85.
    state.set_holding_trailing_stop(id, "15").unwrap();
    let h = state.list_holdings().into_iter().next().unwrap();
    assert_eq!(h.trailing_stop_pct.as_deref(), Some("15"));
    assert_eq!(h.trailing_stop_level.as_deref(), Some("85"));

    // Review fix: an EXPLICIT re-set seeds FRESH (the user's pct wins) — a looser 50% LOWERS the
    // level to 100 × (1 − 0.50) = 50, even though 50 < the prior 85 (ratchet-up-only governs only
    // the automatic refresh path, not an explicit re-parametrisation).
    state.set_holding_trailing_stop(id, "50").unwrap();
    let h = state.list_holdings().into_iter().next().unwrap();
    assert_eq!(h.trailing_stop_pct.as_deref(), Some("50"));
    assert_eq!(h.trailing_stop_level.as_deref(), Some("50"));

    // An empty pct clears the stop (both fields → None).
    state.set_holding_trailing_stop(id, "").unwrap();
    let h = state.list_holdings().into_iter().next().unwrap();
    assert!(h.trailing_stop_pct.is_none() && h.trailing_stop_level.is_none());
}

#[test]
fn ratchet_trailing_stops_moves_up_only_on_a_price_refresh() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x56, "2026-06-28T10:00:00Z");
    // A holding linked to a study of the same ticker (so the ratchet keys on the study's price).
    let study = state.create_study("NESN", "CHF").unwrap();
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    // Seed a 20% stop → level 80 (from purchase 100, no current_price yet).
    state.set_holding_trailing_stop(id, "20").unwrap();
    assert_eq!(
        state.list_holdings()[0].trailing_stop_level.as_deref(),
        Some("80")
    );

    // A refresh to 150 ratchets the level up: 150 × 0.80 = 120.
    state
        .ratchet_trailing_stops_for_study(study, Decimal::from(150))
        .unwrap();
    assert_eq!(
        state.list_holdings()[0].trailing_stop_level.as_deref(),
        Some("120")
    );

    // A refresh to a LOWER 90 leaves the level at 120 (ratchet-up only).
    state
        .ratchet_trailing_stops_for_study(study, Decimal::from(90))
        .unwrap();
    assert_eq!(
        state.list_holdings()[0].trailing_stop_level.as_deref(),
        Some("120"),
        "a falling price never lowers the stop"
    );
}

// ── Story 4.6 — simple capital-at-risk (the portfolio downside figure) ──

#[test]
fn portfolio_capital_at_risk_sums_below_cost_stops_and_invested() {
    let dir = TempDir::new().unwrap();
    // `watch_state` uses a SEQUENTIAL idgen — two holdings get distinct ids (a FixedIdGen would
    // collide on the second insert).
    let mut state = watch_state(&dir, 0x570);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    state.add_holding("ROG", "20", "50", "CHF").unwrap();
    let ids: Vec<_> = state.list_holdings().iter().map(|h| h.id).collect();
    // NESN: a 15% stop with no study → level 85 (below cost 100) → (100−85)×10 = 150.
    state.set_holding_trailing_stop(ids[0], "15").unwrap();
    // ROG: no stop → contributes 0 to capital-at-risk (but to invested).

    // Both holdings are CHF → a single bucket; the figures match the pre-6.2 single sum.
    let buckets = state.portfolio_capital_at_risk_by_currency("CHF");
    assert_eq!(buckets.len(), 1, "one currency → one bucket");
    let (ccy, car, invested) = &buckets[0];
    assert_eq!(ccy, "CHF");
    assert_eq!(
        *car,
        Decimal::from(150),
        "only the below-cost stop contributes"
    );
    assert_eq!(
        *invested,
        Decimal::from(100 * 10 + 50 * 20),
        "invested = Σ cost × qty"
    );
}

#[test]
fn capital_at_risk_groups_by_currency_without_a_cross_currency_total() {
    // Story 6.2 (FR38): a EUR holding and a USD holding must NOT be summed together — the read
    // returns a per-currency bucket each, never a mixed total (FX only at consolidation, FR28).
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x620);
    state.add_holding("ASML", "10", "100", "EUR").unwrap();
    state.add_holding("AAPL", "20", "50", "USD").unwrap();
    let ids: Vec<_> = state.list_holdings().iter().map(|h| h.id).collect();
    // A 15% stop on each → EUR level 85 (CaR (100−85)×10 = 150); USD level 42.5 (CaR (50−42.5)×20 = 150).
    state.set_holding_trailing_stop(ids[0], "15").unwrap();
    state.set_holding_trailing_stop(ids[1], "15").unwrap();

    let buckets = state.portfolio_capital_at_risk_by_currency("CHF");
    assert_eq!(
        buckets,
        vec![
            ("EUR".to_string(), Decimal::from(150), Decimal::from(1000)),
            ("USD".to_string(), Decimal::from(150), Decimal::from(1000)),
        ],
        "two currencies → two independent buckets, sorted by code, no global total"
    );
}

#[test]
fn an_unsupported_holding_currency_is_refused_with_a_neutral_notice() {
    // Story 6.2: the app validates a chosen holding currency against the allow-list (defensive —
    // the UI only offers members) and refuses a stray code with a neutral, cause-named message.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x622);
    assert_eq!(
        state.add_holding("NESN", "10", "100", "XXX").unwrap_err(),
        MSG_HOLDING_INVALID_CURRENCY.to_string()
    );
    assert!(
        state.list_holdings().is_empty(),
        "nothing was written on a bad currency"
    );
}

// ── Story 6.1 — multiple portfolios (FR37) ──

#[test]
fn adding_a_portfolio_makes_it_active_and_scopes_the_register() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x610);
    // The first holding lazily creates the default portfolio (the active one).
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let default_id = state.active_portfolio().expect("a default portfolio").id;
    assert_eq!(state.list_holdings().len(), 1);

    // A new portfolio becomes active; its register is empty until something lands in it.
    let bank2 = state.add_portfolio("PostFinance").unwrap();
    assert_eq!(
        state.active_portfolio().unwrap().id,
        bank2,
        "the new one is active"
    );
    assert!(
        state.list_holdings().is_empty(),
        "the new portfolio starts empty"
    );

    // A holding added now lands in the active (PostFinance), not the default.
    state.add_holding("ROG", "5", "248", "CHF").unwrap();
    let active_tickers: Vec<_> = state
        .list_holdings()
        .iter()
        .map(|h| h.security_ticker.clone())
        .collect();
    assert_eq!(
        active_tickers,
        ["ROG"],
        "the active register shows only PostFinance"
    );

    // Switching back surfaces the default portfolio's holdings again.
    state.set_active_portfolio(default_id);
    let back: Vec<_> = state
        .list_holdings()
        .iter()
        .map(|h| h.security_ticker.clone())
        .collect();
    assert_eq!(back, ["NESN"], "switching active re-scopes the register");
}

#[test]
fn deleting_a_portfolio_is_guarded_and_reselects() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x611);
    state.add_holding("NESN", "10", "100", "CHF").unwrap(); // creates + fills the default
    let default_id = state.active_portfolio().unwrap().id;
    let bank2 = state.add_portfolio("PostFinance").unwrap(); // empty, now active

    // The default has a holding → deleting it is refused (FK never orphaned).
    assert_eq!(
        state.delete_portfolio(default_id),
        Err(MSG_PORTFOLIO_HAS_HOLDINGS.to_string())
    );
    // The empty active portfolio deletes; the active selection falls back to the first.
    state.delete_portfolio(bank2).unwrap();
    assert_eq!(state.list_portfolios().len(), 1);
    assert_eq!(
        state.active_portfolio().unwrap().id,
        default_id,
        "deleting the active one reselects a remaining portfolio"
    );
    // Now only the holding-bearing default remains → it can't be deleted either.
    assert_eq!(
        state.delete_portfolio(default_id),
        Err(MSG_PORTFOLIO_HAS_HOLDINGS.to_string())
    );
}

// ── Story 4.7 — recorded sell on a neutral trigger ──

#[test]
fn sell_holding_records_the_sell_and_drops_it_from_the_register() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x470);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    state.add_holding("ROG", "20", "50", "CHF").unwrap();
    let ids: Vec<_> = state.list_holdings().iter().map(|h| h.id).collect();
    // NESN gets a 15% stop (no study) → level 85, below cost 100 → CaR 150 before the sell.
    state.set_holding_trailing_stop(ids[0], "15").unwrap();
    assert_eq!(
        state.portfolio_capital_at_risk_by_currency("CHF")[0].1,
        Decimal::from(150)
    );

    state
        .sell_holding(ids[0], "", "  stop touché  ", "CHF")
        .expect("the sell records");

    let remaining: Vec<_> = state
        .list_holdings()
        .iter()
        .map(|h| h.security_ticker.clone())
        .collect();
    assert_eq!(remaining, vec!["ROG".to_string()], "NESN left the register");
    // ROG (CHF, no stop) remains → its CHF bucket's capital-at-risk is 0 (the at-risk NESN is gone).
    assert_eq!(
        state.portfolio_capital_at_risk_by_currency("CHF")[0].1,
        Decimal::ZERO,
        "the only at-risk holding is gone → capital-at-risk drops to 0"
    );
}

#[test]
fn sell_holding_stamps_the_holdings_own_currency_not_the_reference() {
    // Story 6.2 review (HIGH): a USD holding sold under a CHF reference must record a USD SELL
    // row (FR28: quantity × unit_price are native amounts — relabeling them in the reference
    // currency would corrupt the 6.3 ledger's input). The reference is only the coalesce
    // fallback for a pre-6.2 legacy (NULL-currency) row.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x622);
    state.add_holding("AAPL", "5", "150", "USD").unwrap();
    let id = state.list_holdings()[0].id;

    state
        .sell_holding(id, "", "", "CHF")
        .expect("the sell records");

    // Story 6.3: the first ledger mutation also materializes the opening BUY row (AC5), so the
    // ledger holds opening + sell — both in the holding's own currency.
    let transactions = state
        .journal
        .as_ref()
        .unwrap()
        .list_all_transactions()
        .unwrap();
    assert_eq!(transactions.len(), 2, "opening buy + the sell");
    assert!(
        transactions.iter().all(|t| t.currency == "USD"),
        "every ledger row carries the holding's own currency, not the CHF reference"
    );
    let sell = transactions
        .iter()
        .find(|t| t.kind.as_deref() == Some("sell"))
        .expect("the sell row");
    assert_eq!(sell.quantity, "5", "a whole-position sell");
}

#[test]
fn sell_holding_refuses_an_absent_id() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x471);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let ghost = Uuid::from_u128(0xDEAD);
    assert!(
        state.sell_holding(ghost, "", "", "CHF").is_err(),
        "selling a non-existent holding is refused, nothing written"
    );
    assert_eq!(state.list_holdings().len(), 1, "the register is untouched");
}

// ── Story 6.3 — transaction ledger, partial sells, weighted-average cost basis (FR39) ──

#[test]
fn a_buy_on_a_legacy_holding_materializes_the_opening_once_and_re_averages() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x630);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;

    // First buy: 10 @ 110, fees 10 → opening (10 @ 100) materialized + buy → 20 @ 105.5.
    state
        .record_buy_for(id, "2026-07-01", "10", "110", "10", "", "CHF")
        .expect("the buy records");
    let ledger = state.holding_ledger(id);
    assert_eq!(ledger.len(), 2, "opening buy + the recorded buy");
    assert!(
        ledger
            .iter()
            .all(|t| t.kind.as_deref() == Some("buy") && t.currency == "CHF"),
        "both rows are buys in the holding's currency"
    );
    // (10×100 + 10×110 + 10) / 20 = 105.5 — Appendix A, fees INCLUDED.
    let holding = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .unwrap();
    assert_eq!(holding.quantity, "20");
    assert_eq!(
        holding.purchase_price, "105.5",
        "weighted-average, fees included"
    );

    // Second buy: NO re-seed of the opening (a buy row already exists).
    state
        .record_buy_for(id, "2026-07-02", "20", "100", "0", "", "CHF")
        .expect("the second buy records");
    let ledger = state.holding_ledger(id);
    assert_eq!(ledger.len(), 3, "no second opening row");
    let holding = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .unwrap();
    assert_eq!(holding.quantity, "40");
    // (20×105.5 + 20×100) / 40 = 102.75.
    assert_eq!(holding.purchase_price, "102.75");
}

#[test]
fn a_partial_sell_reduces_the_quantity_and_keeps_the_holding_active() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x631);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;

    let notice = state
        .sell_holding(id, "4", "prise partielle", "CHF")
        .expect("the partial sell records");
    assert_eq!(notice, MSG_LEDGER_PARTIAL_SOLD);

    let holding = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .expect("still in the register");
    assert_eq!(holding.quantity, "6", "reduced, not retired");
    assert_eq!(holding.purchase_price, "100", "a sell never re-averages");
    assert!(holding.sold_at.is_none());

    // Selling the rest retires it (the 4.7 flow) with the whole-position notice.
    let notice = state
        .sell_holding(id, "6", "", "CHF")
        .expect("the closing sell records");
    assert_eq!(notice, MSG_HOLDING_SOLD);
    assert!(
        !state.list_holdings().iter().any(|h| h.id == id),
        "the emptied position left the register"
    );
}

#[test]
fn an_over_sell_is_refused_neutrally_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x632);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;

    assert_eq!(
        state.sell_holding(id, "11", "", "CHF"),
        Err(MSG_LEDGER_OVERSELL.to_string())
    );
    assert!(
        state.holding_ledger(id).is_empty(),
        "the refusal materialized nothing"
    );
    let holding = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .unwrap();
    assert_eq!(holding.quantity, "10", "the register is untouched");
}

#[test]
fn deleting_the_retiring_sell_un_retires_the_holding() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x633);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    state
        .sell_holding(id, "", "", "CHF")
        .expect("the sell records");
    assert!(!state.list_holdings().iter().any(|h| h.id == id));

    let sell_id = state
        .holding_ledger(id)
        .iter()
        .find(|t| t.kind.as_deref() == Some("sell"))
        .expect("the sell row")
        .id;
    state
        .delete_transaction_for(id, sell_id, "CHF")
        .expect("the delete applies");

    let holding = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .expect("the holding is back in the register");
    assert_eq!(holding.quantity, "10", "the restored opening position");
    assert_eq!(holding.purchase_price, "100");
    assert!(holding.sold_at.is_none(), "un-retired");
}

#[test]
fn an_edit_that_makes_history_impossible_is_refused() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x634);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    state
        .sell_holding(id, "4", "", "CHF")
        .expect("the partial sell records");
    let sell_id = state
        .holding_ledger(id)
        .iter()
        .find(|t| t.kind.as_deref() == Some("sell"))
        .unwrap()
        .id;

    // Editing the sell to 11 would exceed the 10 ever held → neutral refusal, nothing changed.
    assert_eq!(
        state.update_transaction_for(id, sell_id, "", "11", "100", "0", "", "CHF"),
        Err(MSG_LEDGER_OVERSELL.to_string())
    );
    let holding = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .unwrap();
    assert_eq!(holding.quantity, "6", "the aggregate is untouched");

    // A legal edit applies and re-derives the aggregate: the sell becomes 2 → 8 held.
    state
        .update_transaction_for(id, sell_id, "2026-07-02", "2", "100", "0", "raison", "CHF")
        .expect("the edit applies");
    let holding = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .unwrap();
    assert_eq!(holding.quantity, "8");
}

#[test]
fn ledger_writes_are_refused_on_a_read_only_journal() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x635);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    state.read_only = true;
    assert_eq!(
        state.record_buy_for(id, "", "1", "1", "", "", "CHF"),
        Err(MSG_READ_ONLY_WRITE.to_string())
    );
    assert_eq!(
        state.sell_holding(id, "1", "", "CHF"),
        Err(MSG_READ_ONLY_WRITE.to_string())
    );
    assert_eq!(
        state.delete_transaction_for(id, Uuid::from_u128(1), "CHF"),
        Err(MSG_READ_ONLY_WRITE.to_string())
    );
}

#[test]
fn a_bad_date_or_amount_on_a_buy_is_refused_neutrally() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x636);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    assert_eq!(
        state.record_buy_for(id, "02/07/2026", "1", "1", "", "", "CHF"),
        Err(MSG_LEDGER_INVALID_DATE.to_string())
    );
    assert_eq!(
        state.record_buy_for(id, "", "0", "1", "", "", "CHF"),
        Err(MSG_HOLDING_INVALID_NUMBER.to_string())
    );
    assert_eq!(
        state.record_buy_for(id, "", "1", "-1", "", "", "CHF"),
        Err(MSG_HOLDING_INVALID_NUMBER.to_string())
    );
    assert!(state.holding_ledger(id).is_empty(), "nothing materialized");
}

// ── Story 6.4 — dividends: gross study, net reinvestable (FR41) ──

#[test]
fn a_dividend_records_as_cash_and_touches_neither_position_nor_opening() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x640);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;

    // Explicit withholding (10.50 on a 30 gross).
    state
        .record_dividend_for(id, "2026-07-01", "10", "3", "10.5", "acompte", "CHF", "35")
        .expect("the dividend records");

    let rows = state.holding_ledger(id);
    assert_eq!(rows.len(), 1, "NO opening materialization for a cash event");
    assert_eq!(rows[0].kind.as_deref(), Some("dividend"));
    assert_eq!(rows[0].quantity, "10");
    assert_eq!(rows[0].unit_price, "3", "gross per share");
    assert_eq!(rows[0].fees, "10.5", "the withholding");
    let holding = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .unwrap();
    assert_eq!(holding.quantity, "10", "position untouched");
    assert_eq!(holding.purchase_price, "100", "basis untouched");

    // A later buy still materializes the opening correctly (the dividend is not a buy).
    state
        .record_buy_for(id, "2026-07-02", "10", "110", "0", "", "CHF")
        .expect("the buy records");
    let kinds: Vec<_> = state
        .holding_ledger(id)
        .iter()
        .map(|t| t.kind.clone().unwrap())
        .collect();
    assert_eq!(
        kinds.iter().filter(|k| *k == "buy").count(),
        2,
        "opening + the recorded buy"
    );
}

#[test]
fn an_empty_withholding_auto_computes_at_the_configured_rate() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x641);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;

    // "" at the default 35 % → 10 × 3 × 0.35 = 10.5.
    state
        .record_dividend_for(id, "", "10", "3", "", "", "CHF", "35")
        .unwrap();
    // "" at a configured 15 % → 4.5; an explicit 0 overrides entirely.
    state
        .record_dividend_for(id, "", "10", "3", "", "", "CHF", "15")
        .unwrap();
    state
        .record_dividend_for(id, "", "10", "3", "0", "", "CHF", "35")
        .unwrap();

    let fees: Vec<_> = state
        .holding_ledger(id)
        .iter()
        .map(|t| t.fees.clone())
        .collect();
    assert_eq!(fees, vec!["10.5", "4.5", "0"]);
}

#[test]
fn a_withholding_exceeding_the_gross_refuses_neutrally() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x642);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    assert_eq!(
        state.record_dividend_for(id, "", "10", "3", "30.01", "", "CHF", "35"),
        Err(MSG_DIVIDEND_WITHHOLDING.to_string())
    );
    assert!(state.holding_ledger(id).is_empty(), "nothing written");
}

#[test]
fn reinvestable_cash_groups_per_currency_and_counts_sold_holdings() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x643);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    state.add_holding("AAPL", "5", "150", "USD").unwrap();
    let ids: Vec<_> = state.list_holdings().iter().map(|h| h.id).collect();

    // CHF: net 19.5 (30 gross − 10.5). USD: net 6 (2×3 gross − 0).
    state
        .record_dividend_for(ids[0], "", "10", "3", "", "", "CHF", "35")
        .unwrap();
    state
        .record_dividend_for(ids[1], "", "2", "3", "0", "", "CHF", "35")
        .unwrap();
    // Sell the USD position entirely — its dividend cash must SURVIVE in the panel.
    state.sell_holding(ids[1], "", "", "CHF").unwrap();
    assert!(
        !state.list_holdings().iter().any(|h| h.id == ids[1]),
        "the USD holding is retired"
    );

    let cash = state.portfolio_reinvestable_cash_by_currency("CHF");
    assert_eq!(
        cash,
        vec![
            ("CHF".to_string(), Decimal::from_str_exact("19.5").unwrap()),
            ("USD".to_string(), Decimal::from(6)),
        ],
        "per-currency nets, sold holding's dividends included, no mixed total"
    );
}

#[test]
fn a_dividend_on_a_retired_holding_is_refused_at_the_v1_entry_point() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x644);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    state.sell_holding(id, "", "", "CHF").unwrap();
    assert_eq!(
        state.record_dividend_for(id, "", "10", "3", "", "", "CHF", "35"),
        Err(MSG_DIVIDEND_RETIRED.to_string()),
        "a factual scope refusal, not a fake save failure (#84 owns the sold view)"
    );
}

// ── Story 6.4 review patches (2026-07-02) ──

#[test]
fn editing_a_dividends_withholding_beyond_its_gross_is_refused() {
    // Review HIGH (all three layers): the record-path invariant holds on EDIT too.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x645);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    state
        .record_dividend_for(id, "2026-07-01", "10", "3", "10.5", "", "CHF", "35")
        .unwrap();
    let div_id = state.holding_ledger(id)[0].id;

    assert_eq!(
        state.update_transaction_for(id, div_id, "2026-07-01", "10", "3", "1000", "", "CHF"),
        Err(MSG_DIVIDEND_WITHHOLDING.to_string()),
        "the withholding ≤ gross invariant survives the edit rail"
    );
    assert_eq!(state.holding_ledger(id)[0].fees, "10.5", "nothing changed");
}

#[test]
fn mutating_a_dividend_only_ledger_touches_neither_opening_nor_position() {
    // Review MED: a cash-row edit/delete on a dividend-only ledger must not fabricate an opening
    // « Achat » nor rewrite/retire the stored aggregate (the replay would read an empty position).
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x646);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    state
        .record_dividend_for(id, "2026-07-01", "10", "3", "0", "", "CHF", "35")
        .unwrap();
    let div_id = state.holding_ledger(id)[0].id;

    // Edit the rationale/withholding: still exactly ONE row, position/sold_at untouched.
    state
        .update_transaction_for(
            id,
            div_id,
            "2026-07-01",
            "10",
            "3",
            "10.5",
            "corrigé",
            "CHF",
        )
        .expect("the cash edit applies");
    assert_eq!(state.holding_ledger(id).len(), 1, "no phantom opening buy");
    let holding = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .expect("still ACTIVE — a cash edit never retires");
    assert_eq!(holding.quantity, "10");
    assert_eq!(holding.purchase_price, "100");

    // Delete it: the ledger is empty again — nothing was fabricated, nothing retired.
    state
        .delete_transaction_for(id, div_id, "CHF")
        .expect("the delete applies");
    assert!(state.holding_ledger(id).is_empty(), "truly empty again");
    assert!(
        state.list_holdings().iter().any(|h| h.id == id),
        "the holding stays in the register"
    );
}

#[test]
fn a_sell_after_a_dividend_first_ledger_still_materializes_the_opening() {
    // Review (blind #8): the opening rule keys on buy rows, not "ledger empty" — a pre-existing
    // dividend row must not suppress the opening when a SELL arrives.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x647);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    state
        .record_dividend_for(id, "2026-07-01", "10", "3", "0", "", "CHF", "35")
        .unwrap();

    let notice = state
        .sell_holding(id, "4", "", "CHF")
        .expect("the partial sell records (no spurious over-sell)");
    assert_eq!(notice, MSG_LEDGER_PARTIAL_SOLD);
    let kinds: Vec<_> = state
        .holding_ledger(id)
        .iter()
        .map(|t| t.kind.clone().unwrap())
        .collect();
    assert!(
        kinds.contains(&"buy".to_string()),
        "the opening materialized alongside the sell"
    );
    let holding = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .unwrap();
    assert_eq!(holding.quantity, "6");
}

#[test]
fn one_invalid_dividend_row_does_not_erase_its_currency_bucket() {
    // Review HIGH (panel side): a parseable-but-invalid row (over-withheld — plantable via a
    // foreign 5.3 import) is skipped PER ROW; the bucket keeps its valid cash.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x648);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    state
        .record_dividend_for(id, "2026-07-01", "10", "3", "10.5", "", "CHF", "35")
        .unwrap();
    // Plant an over-withheld row straight at the persistence layer (the import path's freedom).
    state
        .journal
        .as_mut()
        .unwrap()
        .record_dividend(
            id,
            &steadyinvest_persistence::LedgerEntry {
                id: Uuid::from_u128(0xBAD),
                occurred_at: "2026-07-02T00:00:00Z",
                quantity: "1",
                unit_price: "1",
                fees: "1000",
                currency: "CHF",
                rationale: None,
            },
            &Timestamp("2026-07-02T09:00:00Z".to_string()),
        )
        .unwrap();

    let cash = state.portfolio_reinvestable_cash_by_currency("CHF");
    assert_eq!(
        cash,
        vec![("CHF".to_string(), Decimal::from_str_exact("19.5").unwrap())],
        "the valid row's net survives; only the invalid ROW is skipped"
    );
}

// ── Story 6.3 review patches (2026-07-02) ──

#[test]
fn deleting_the_opening_buy_that_sells_depend_on_is_refused_not_reinvented() {
    // Review CRITICAL: the opening must NEVER be re-seeded from the derived aggregate. With a
    // sell in the ledger, deleting the opening buy is an impossible history → neutral refusal,
    // nothing changed (previously: a phantom 6@100 opening appeared and the position dropped to 2).
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x63A);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    state
        .sell_holding(id, "4", "", "CHF")
        .expect("the partial sell records");
    let opening_id = state
        .holding_ledger(id)
        .iter()
        .find(|t| t.kind.as_deref() == Some("buy"))
        .expect("the materialized opening")
        .id;

    assert_eq!(
        state.delete_transaction_for(id, opening_id, "CHF"),
        Err(MSG_LEDGER_OVERSELL.to_string()),
        "deleting the only buy the sell depends on refuses"
    );
    let holding = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .unwrap();
    assert_eq!(holding.quantity, "6", "nothing changed");
    assert_eq!(state.holding_ledger(id).len(), 2, "both rows survive");
}

#[test]
fn a_ledger_backed_holding_refuses_direct_quantity_price_currency_edits() {
    // Review HIGH: once the aggregate is derived from the ledger, the 4.3 register edit must not
    // desynchronize it. Ticker-only edits stay allowed (not ledger-derived).
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x63B);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    state
        .record_buy_for(id, "2026-07-01", "10", "110", "0", "", "CHF")
        .expect("the buy records");

    assert_eq!(
        state.update_holding(id, "NESN", "4", "105", "CHF"),
        Err(MSG_LEDGER_BACKED.to_string()),
        "a direct quantity edit is refused"
    );
    let stored = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .unwrap();
    assert_eq!(stored.quantity, "20", "aggregate untouched");
    // A ticker-only edit (identical amounts/currency) still applies.
    state
        .update_holding(
            id,
            "NESN.SW",
            &stored.quantity,
            &stored.purchase_price,
            "CHF",
        )
        .expect("a ticker-only edit is fine");
    assert_eq!(state.list_holdings()[0].security_ticker, "NESN.SW");
}

#[test]
fn a_ledger_form_sell_records_the_explicit_price_and_fees() {
    // Review decision (FR39 to the letter): the ledger-form sell carries the user's own price,
    // date and fees — unlike the trigger sell (study price, fees 0).
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x63C);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;

    let notice = state
        .record_sell_for(id, "2026-07-01", "4", "123.45", "9.90", "allègement", "CHF")
        .expect("the ledger sell records");
    assert_eq!(notice, MSG_LEDGER_PARTIAL_SOLD);

    let sell = state
        .holding_ledger(id)
        .into_iter()
        .find(|t| t.kind.as_deref() == Some("sell"))
        .expect("the sell row");
    assert_eq!(
        sell.unit_price, "123.45",
        "the explicit price, not the study's"
    );
    assert_eq!(sell.fees, "9.90");
    assert_eq!(sell.occurred_at.0, "2026-07-01T00:00:00Z");
    assert_eq!(sell.rationale.as_deref(), Some("allègement"));
    let holding = state
        .list_holdings()
        .into_iter()
        .find(|h| h.id == id)
        .unwrap();
    assert_eq!(holding.quantity, "6");
    assert_eq!(holding.purchase_price, "100", "a sell never re-averages");
}

#[test]
fn impossible_calendar_dates_are_refused() {
    // Review MED: Feb 30 / Apr 31 / year 0000 must refuse (the copy promises AAAA-MM-JJ réel).
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x63D);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    for bad in ["2026-02-30", "2026-04-31", "0000-01-01", "2026-13-01"] {
        assert_eq!(
            state.record_buy_for(id, bad, "1", "1", "", "", "CHF"),
            Err(MSG_LEDGER_INVALID_DATE.to_string()),
            "{bad} must refuse"
        );
    }
    // A real leap day passes.
    state
        .record_buy_for(id, "2024-02-29", "1", "1", "", "", "CHF")
        .expect("a leap day is a real date");
}

#[test]
fn editing_only_the_rationale_of_a_legacy_sell_keeps_its_timestamp() {
    // Review MED: a legacy 4.7 sell carries a wall-clock occurred_at; an edit that leaves the
    // visible DATE unchanged must keep the stamp verbatim (no silent same-day reorder).
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x63E);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    // A 4.7-style whole-position sell (wall-clock occurred_at, no ledger materialization).
    let sell_id = Uuid::from_u128(0x47);
    state
        .journal
        .as_mut()
        .unwrap()
        .record_sell(
            sell_id,
            id,
            "10",
            "120",
            "0",
            "CHF",
            None,
            &Timestamp("2026-06-27T15:00:00Z".to_string()),
        )
        .unwrap();

    state
        .update_transaction_for(id, sell_id, "2026-06-27", "10", "120", "0", "raison", "CHF")
        .expect("the rationale edit applies");
    let row = state
        .holding_ledger(id)
        .into_iter()
        .find(|t| t.id == sell_id)
        .unwrap();
    assert_eq!(
        row.occurred_at.0, "2026-06-27T15:00:00Z",
        "the wall-clock stamp survives a same-date edit"
    );
    assert_eq!(row.rationale.as_deref(), Some("raison"));
}

// ── Story 4.3 — holdings register (single-portfolio CRUD + decimal validation) ──

#[test]
fn add_holding_persists_lazily_creates_one_portfolio_and_lists_in_order() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x430);
    assert!(
        state.list_holdings().is_empty(),
        "no holdings, no portfolio yet"
    );

    state.add_holding("NESN", "10", "95.40", "CHF").unwrap();
    state.add_holding("ROG", "5", "248.10", "CHF").unwrap();
    let rows = state.list_holdings();
    assert_eq!(
        rows.iter()
            .map(|h| h.security_ticker.as_str())
            .collect::<Vec<_>>(),
        ["NESN", "ROG"],
        "both holdings persist, in creation order"
    );
    assert_eq!(rows[0].quantity, "10");
    assert_eq!(rows[0].purchase_price, "95.40");
    // All holdings share the single lazily-created portfolio.
    assert_eq!(rows[0].portfolio_id, rows[1].portfolio_id, "one portfolio");
}

#[test]
fn holding_amounts_are_validated_and_bad_input_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x431);
    assert_eq!(
        state.add_holding("  ", "10", "5", "CHF").unwrap_err(),
        MSG_HOLDING_INVALID_TICKER
    );
    assert_eq!(
        state.add_holding("NESN", "abc", "5", "CHF").unwrap_err(),
        MSG_HOLDING_INVALID_NUMBER
    );
    assert_eq!(
        state.add_holding("NESN", "0", "5", "CHF").unwrap_err(),
        MSG_HOLDING_INVALID_NUMBER,
        "quantity must be strictly positive"
    );
    assert_eq!(
        state.add_holding("NESN", "-2", "5", "CHF").unwrap_err(),
        MSG_HOLDING_INVALID_NUMBER
    );
    assert_eq!(
        state.add_holding("NESN", "2", "-5", "CHF").unwrap_err(),
        MSG_HOLDING_INVALID_NUMBER,
        "price must be non-negative"
    );
    // Issue #60: an absurd magnitude (qty or price beyond a trillion) is refused, so the saturating
    // capital-at-risk overlay never has to clamp a persisted holding into a misleading total.
    assert_eq!(
        state
            .add_holding("NESN", "10000000000", "100000000000000000000", "CHF")
            .unwrap_err(),
        MSG_HOLDING_AMOUNT_OUT_OF_RANGE,
        "qty 1e10 × price 1e20 (the overflow case) is refused on write"
    );
    assert_eq!(
        state
            .add_holding("NESN", "1000000000001", "1", "CHF")
            .unwrap_err(),
        MSG_HOLDING_AMOUNT_OUT_OF_RANGE,
        "quantity just over a trillion is refused"
    );
    assert!(
        state.list_holdings().is_empty(),
        "no invalid input wrote a row"
    );
    // A free purchase (price 0) is allowed (e.g. a gift/spin-off).
    state.add_holding("FREE", "1", "0", "CHF").unwrap();
    assert_eq!(state.list_holdings().len(), 1);
    // The bound is inclusive: exactly a trillion (the ceiling) is still accepted.
    state
        .add_holding("BIG", "1000000000000", "1000000000000", "CHF")
        .unwrap();
    assert_eq!(
        state.list_holdings().len(),
        2,
        "the magnitude ceiling is inclusive"
    );
}

#[test]
fn edit_and_delete_holding_round_trip_and_survive_reopen() {
    let dir = TempDir::new().unwrap();
    let id = {
        let mut state = watch_state(&dir, 0x432);
        state.add_holding("NESN", "10", "95.40", "CHF").unwrap();
        state.add_holding("ROG", "5", "248.10", "CHF").unwrap();
        let nesn = state.list_holdings()[0].id;
        state
            .update_holding(nesn, "NESN.SW", "12", "96.00", "CHF")
            .unwrap();
        let rog = state.list_holdings()[1].id;
        state.delete_holding(rog).unwrap();
        nesn
    };
    // Reopen the same on-disk journal → the edit and the delete persisted.
    let reopened = watch_state(&dir, 0x999);
    let rows = reopened.list_holdings();
    assert_eq!(rows.len(), 1, "the deleted holding stayed deleted");
    assert_eq!(rows[0].id, id);
    assert_eq!(rows[0].security_ticker, "NESN.SW");
    assert_eq!(rows[0].quantity, "12");
    assert_eq!(rows[0].purchase_price, "96.00");
}

#[test]
fn undo_redo_steps_back_and_forward_and_a_new_edit_clears_redo() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x1D, "2026-06-14T09:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    assert!(
        !state.can_undo() && !state.can_redo(),
        "a fresh study has empty history"
    );

    // Edit one §3 cell (field "a" = high price) on year 0.
    state.edit_cell(id, 0, "a", Some(und_money(100))).unwrap();
    assert!(state.can_undo(), "an edit is undoable");
    assert!(!state.can_redo());
    assert!(
        state.get_study(id).unwrap().years[0]
            .high_price
            .value
            .is_some()
    );

    // Undo → the pre-edit (fresh, no-value) state returns.
    assert_eq!(state.undo(id), Ok(true));
    assert!(state.can_redo());
    let undone = state.get_study(id).unwrap();
    assert!(
        undone.years.is_empty() || undone.years[0].high_price.value.is_none(),
        "undo restores the pre-edit state (no value)"
    );

    // Redo → the value comes back.
    assert_eq!(state.redo(id), Ok(true));
    assert!(
        state.get_study(id).unwrap().years[0]
            .high_price
            .value
            .is_some()
    );

    // A NEW edit after an undo forks history → the redo branch is cleared.
    assert_eq!(state.undo(id), Ok(true));
    assert!(state.can_redo());
    state.edit_cell(id, 0, "b", Some(und_money(50))).unwrap();
    assert!(
        !state.can_redo(),
        "a new edit after an undo clears the redo branch"
    );
    assert!(state.can_undo());
}

#[test]
fn undo_restores_a_judgment_edit() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x2D, "2026-06-14T09:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    state
        .set_judgment_field(id, "est_high_eps", Some(und_money(9)))
        .unwrap();
    assert!(
        state
            .get_study(id)
            .unwrap()
            .judgment
            .estimated_high_eps
            .is_some()
    );
    assert_eq!(state.undo(id), Ok(true));
    assert!(
        state
            .get_study(id)
            .unwrap()
            .judgment
            .estimated_high_eps
            .is_none(),
        "undo restores the prior (unset) judgment — FR32, never destroys a saved input"
    );
}

#[test]
fn undo_redo_on_empty_history_are_noops() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x3D, "2026-06-14T09:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    assert_eq!(state.undo(id), Ok(false), "nothing to undo");
    assert_eq!(state.redo(id), Ok(false), "nothing to redo");
    assert!(!state.can_undo() && !state.can_redo());
}

#[test]
fn reset_undo_clears_history() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x4D, "2026-06-14T09:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    state
        .set_judgment_field(id, "est_high_eps", Some(und_money(9)))
        .unwrap();
    assert!(state.can_undo());
    state.reset_undo(); // a different study is opened
    assert!(
        !state.can_undo() && !state.can_redo(),
        "opening a study starts from an empty history"
    );
}

// ── Story 2.10 — decision rationale: set → reopen restores; clear → None; trim; undo restores ──

#[test]
fn rationale_round_trips_through_reopen_and_clears_to_none() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    drop(
        Journal::create(
            &path,
            Uuid::from_u128(0xC0FFEE),
            &Timestamp("2026-06-14T00:00:00Z".to_string()),
        )
        .unwrap(),
    );
    let mut state = open_state(&path);
    let id = state.create_study("NESN", "CHF").unwrap();

    // Set a rationale → it is restored on reopen (a fresh JournalState on the same journal, FR49).
    state
        .set_rationale(id, Some("Marge en hausse, dette faible".to_string()))
        .unwrap();
    assert_eq!(
        open_state(&path)
            .get_study(id)
            .unwrap()
            .rationale
            .as_deref(),
        Some("Marge en hausse, dette faible"),
        "a saved rationale survives reopen (FR49)"
    );

    // Whitespace-only clears to None (absence ≠ empty value) — never Some("").
    state.set_rationale(id, Some("   ".to_string())).unwrap();
    assert_eq!(
        open_state(&path).get_study(id).unwrap().rationale,
        None,
        "an empty/whitespace rationale stores None, never Some(\"\")"
    );

    // A bare `None` clears it too.
    state
        .set_rationale(id, Some("re-rempli".to_string()))
        .unwrap();
    state.set_rationale(id, None).unwrap();
    assert_eq!(open_state(&path).get_study(id).unwrap().rationale, None);
}

#[test]
fn rationale_is_trimmed_before_storage() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    drop(
        Journal::create(
            &path,
            Uuid::from_u128(0xA),
            &Timestamp("2026-06-14T00:00:00Z".to_string()),
        )
        .unwrap(),
    );
    let mut state = open_state(&path);
    let id = state.create_study("NESN", "CHF").unwrap();
    state
        .set_rationale(id, Some("  garde le texte  ".to_string()))
        .unwrap();
    assert_eq!(
        state.get_study(id).unwrap().rationale.as_deref(),
        Some("garde le texte"),
        "surrounding whitespace is trimmed before storage"
    );
}

#[test]
fn undo_restores_the_prior_rationale() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x6A, "2026-06-14T09:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();

    state
        .set_rationale(id, Some("première raison".to_string()))
        .unwrap();
    state
        .set_rationale(id, Some("raison révisée".to_string()))
        .unwrap();
    assert_eq!(
        state.get_study(id).unwrap().rationale.as_deref(),
        Some("raison révisée")
    );

    // Undo restores the prior rationale (FR32 — a rationale edit is "any edit", never destroyed).
    assert_eq!(state.undo(id), Ok(true));
    assert_eq!(
        state.get_study(id).unwrap().rationale.as_deref(),
        Some("première raison"),
        "undo restores the prior rationale, never destroys it"
    );
}

#[test]
fn re_saving_the_same_rationale_records_no_phantom_undo_step() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x6B, "2026-06-14T09:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    state
        .set_rationale(id, Some("inchangé".to_string()))
        .unwrap();
    state.reset_undo();
    // Re-saving the identical rationale (after trimming) is a no-op → no undo step recorded (P4).
    state
        .set_rationale(id, Some("  inchangé  ".to_string()))
        .unwrap();
    assert!(
        !state.can_undo(),
        "re-saving the same rationale records no phantom undo step (review P4)"
    );
}

// ── Story 2.11 — update an existing study & extend its projection (roll the window forward) ──

#[test]
fn extend_history_appends_next_year_and_survives_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);

    // The window is materialized (2021..=2025); extending rolls it forward by one (2026).
    state.extend_history(id).expect("extend persists");
    let back = open_state(&path).get_study(id).expect("study reopens");
    assert_eq!(
        back.years.len(),
        entry::YEAR_WINDOW + 1,
        "the data window grew forward by one year"
    );
    let added = back.years.last().unwrap();
    assert_eq!(
        added.year, 2026,
        "the appended year is latest+1 (newest at the bottom, SSG order)"
    );
    assert_eq!(
        added.eps.value, None,
        "the appended year is a to-fill gap, never 0"
    );
    assert_eq!(added.eps.coverage, Coverage::ToFill);
}

#[test]
fn extend_history_rolls_the_window_forward_each_call() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);

    state.extend_history(id).unwrap(); // 2026
    state.extend_history(id).unwrap(); // 2027
    let years: Vec<i32> = state
        .get_study(id)
        .unwrap()
        .years
        .iter()
        .map(|y| y.year)
        .collect();
    assert_eq!(
        years,
        vec![
            2016, 2017, 2018, 2019, 2020, 2021, 2022, 2023, 2024, 2025, 2026, 2027
        ],
        "each call appends the next year (oldest→newest, horizon re-bases off the new latest)"
    );
}

#[test]
fn extend_history_is_capped_at_the_max_year_window_with_a_neutral_notice() {
    // Issue #35: the annual roll-forward stops at MAX_HISTORY_YEARS with a neutral notice (never a
    // silent cap), so repeated "+ année" can't overflow the §2 horizontal layout.
    use crate::viewmodel::entry::MAX_HISTORY_YEARS;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path); // starts at YEAR_WINDOW (10) years
    let start = state.get_study(id).unwrap().years.len();
    for _ in 0..(MAX_HISTORY_YEARS - start) {
        state.extend_history(id).expect("extends until the cap");
    }
    assert_eq!(
        state.get_study(id).unwrap().years.len(),
        MAX_HISTORY_YEARS,
        "extends up to exactly the cap"
    );
    // The next roll-forward refuses with the neutral notice and grows nothing.
    assert_eq!(
        state.extend_history(id),
        Err(crate::state::MSG_YEARS_MAX.to_string())
    );
    assert_eq!(
        state.get_study(id).unwrap().years.len(),
        MAX_HISTORY_YEARS,
        "the grid never grows past the cap"
    );
}

#[test]
fn undo_restores_the_pre_extend_year_window() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);
    let before = state.get_study(id).unwrap().years.len();

    state.extend_history(id).unwrap();
    assert_eq!(state.get_study(id).unwrap().years.len(), before + 1);

    // Adding a year is "any edit" — one undo step restores the prior window (FR32, never destroys).
    assert_eq!(state.undo(id), Ok(true));
    assert_eq!(
        state.get_study(id).unwrap().years.len(),
        before,
        "undo restores the pre-add window"
    );
}

#[test]
fn extend_history_on_a_read_only_journal_is_refused() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let id = study_with_entry(&path).1; // seeds + materializes the 5-year window, then drops the state

    let mut state = open_state(&path);
    state.read_only = true;
    assert_eq!(
        state.extend_history(id),
        Err(MSG_READ_ONLY_WRITE.to_string())
    );
    assert_eq!(
        open_state(&path).get_study(id).unwrap().years.len(),
        entry::YEAR_WINDOW,
        "a refused extend appended nothing"
    );
}

#[test]
fn editing_and_the_soft_lock_hold_across_a_reopen() {
    // AC1/AC2 regression: the existing edit + soft-lock rails behave correctly when the study is
    // edited through a fresh reopen (a new JournalState on the same journal), not just in-session.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let id = study_with_entry(&path).1; // high_price@0 = 120.5, window materialized

    // Validate the cell, then REOPEN a fresh state on the same journal.
    {
        let mut state = open_state(&path);
        state
            .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
            .unwrap();
    }
    let mut reopened = open_state(&path);

    // AC2: the soft-lock survives the reopen — a typed edit on the ✓ cell is still refused.
    assert_eq!(
        reopened.edit_cell(id, 0, entry::FIELD_HIGH, Some(money("999"))),
        Err(MSG_SOFT_LOCKED.to_string())
    );

    // AC1: after the deliberate clear-✓, an edit on the reopened study persists (recompute frame).
    reopened
        .set_review(id, 0, entry::FIELD_HIGH, Review::ToReview)
        .unwrap();
    reopened
        .edit_cell(id, 0, entry::FIELD_HIGH, Some(money("130")))
        .expect("a ? cell edits normally after reopen");
    assert_eq!(
        open_state(&path).get_study(id).unwrap().years[0]
            .high_price
            .value,
        Some(money("130")),
        "an edit on a reopened study persists"
    );
}

#[test]
fn undo_restores_a_review_tag_without_destroying_the_value() {
    let dir = TempDir::new().unwrap();
    let mut state = undo_state(&dir, 0x5E, "2026-06-14T09:00:00Z");
    let id = state.create_study("NESN", "CHF").unwrap();
    state.edit_cell(id, 0, "a", Some(und_money(100))).unwrap();
    state.set_review(id, 0, "a", Review::Validated).unwrap();
    assert_eq!(
        state.get_study(id).unwrap().years[0].high_price.review,
        Review::Validated
    );
    assert_eq!(state.undo(id), Ok(true)); // undo the review change only
    let undone = state.get_study(id).unwrap();
    assert_eq!(
        undone.years[0].high_price.review,
        Review::None,
        "undo restores the prior review tag"
    );
    assert!(
        undone.years[0].high_price.value.is_some(),
        "undoing the review tag never destroys the value"
    );
}

#[test]
fn create_then_list_then_reopen_restores_full_state() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (clock, idgen) = fixed(0x5D, "2026-06-13T09:00:00Z");
    // Pre-create a journal at a known path so `open_or_create` opens it (rather than falling
    // through to the OS data dir, which is unavailable / undesirable under test).
    drop(
        Journal::create(
            &path,
            Uuid::from_u128(0xC0FFEE),
            &Timestamp("2026-06-13T00:00:00Z".to_string()),
        )
        .unwrap(),
    );

    let (mut state, notice) = JournalState::open_or_create(Some(&path), clock, idgen);
    assert!(notice.is_none(), "clean open has no notice");
    assert!(!state.is_read_only());
    assert_eq!(state.path(), Some(path.as_path()));
    assert_eq!(state.list_studies().len(), 0, "no studies yet");

    let id = state
        .create_study("  NESN ", " chf ")
        .expect("a valid study is created");
    let rows = state.list_studies();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].security_ticker, "NESN",
        "ticker trimmed, case preserved"
    );
    assert_eq!(rows[0].status, "active");

    let back = state.get_study(id).expect("the study reopens");
    assert_eq!(back.security_ticker, "NESN");
    assert_eq!(
        back.native_currency, "CHF",
        "currency trimmed + upper-cased"
    );
    assert!(back.years.is_empty(), "a fresh study has no years");
    assert_eq!(back.created_at.0, "2026-06-13T09:00:00Z", "injected clock");
    assert_eq!(id, Uuid::from_u128(0x5D), "injected id");
}

#[test]
fn blank_ticker_is_refused_with_a_neutral_message_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    drop(
        Journal::create(
            &path,
            Uuid::from_u128(0xA),
            &Timestamp("2026-06-13T00:00:00Z".to_string()),
        )
        .unwrap(),
    );
    let (clock, idgen) = fixed(0x1, "2026-06-13T09:00:00Z");
    let (mut state, _) = JournalState::open_or_create(Some(&path), clock, idgen);

    assert_eq!(
        state.create_study("   ", "CHF"),
        Err(MSG_BLANK_TICKER.into())
    );
    assert_eq!(
        state.create_study("NESN", "  "),
        Err(MSG_BLANK_CURRENCY.into())
    );
    assert_eq!(state.list_studies().len(), 0, "no study was written");
}

#[test]
fn missing_configured_file_falls_through_to_a_created_default_or_none() {
    // A configured path that does NOT exist must not be opened as an empty journal; the code
    // falls through to the default. In a sandbox the data dir may be unavailable — either a
    // created default (Some path) or a clean no-journal state is acceptable, never a panic.
    let (clock, idgen) = fixed(0x2, "2026-06-13T09:00:00Z");
    let missing = PathBuf::from("/nonexistent/steadyinvest/journal.db");
    let (state, _notice) = JournalState::open_or_create(Some(&missing), clock, idgen);
    assert_ne!(
        state.path(),
        Some(missing.as_path()),
        "a missing configured file is never adopted as-is"
    );
}

#[test]
fn created_at_date_takes_the_date_portion() {
    assert_eq!(
        created_at_date(&Timestamp("2026-06-13T09:00:00Z".to_string())),
        "2026-06-13"
    );
    assert_eq!(created_at_date(&Timestamp("weird".to_string())), "weird");
}

// ── Story 2.4: manual entry → `Cell::edited` → `put_study` → reopen round-trip ──

fn open_state(path: &Path) -> JournalState {
    let (clock, idgen) = fixed(0x5D, "2026-06-13T09:00:00Z");
    let (state, _) = JournalState::open_or_create(Some(path), clock, idgen);
    state
}

fn money(s: &str) -> Money {
    Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
}

#[test]
fn manual_edit_stamps_source_manual_present_and_survives_reopen() {
    use steadyinvest_contract::{Freshness, Source};
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    drop(
        Journal::create(
            &path,
            Uuid::from_u128(0xC0FFEE),
            &Timestamp("2026-06-13T00:00:00Z".to_string()),
        )
        .unwrap(),
    );
    let mut state = open_state(&path);
    let id = state.create_study("NESN", "CHF").unwrap();

    // A fresh study has no years; the first edit materializes the window then sets the cell.
    state
        .edit_cell(id, 0, entry::FIELD_HIGH, Some(money("120.5")))
        .expect("edit persists");

    // Reopen from disk: the entered value, its manual source/freshness and Present coverage survive.
    let back = open_state(&path).get_study(id).expect("study reopens");
    assert_eq!(
        back.years.len(),
        entry::YEAR_WINDOW,
        "the window was materialized"
    );
    let cell = &back.years[0].high_price;
    assert_eq!(cell.value, Some(money("120.5")));
    assert_eq!(
        cell.source,
        Source::Manual,
        "a manual edit is stamped source=manual"
    );
    assert_eq!(
        cell.freshness,
        Freshness::Current,
        "a fresh edit is current"
    );
    assert_eq!(cell.coverage, Coverage::Present);
    assert_eq!(cell.provenance.source, Source::Manual);
    assert_eq!(
        cell.provenance.timestamp.0, "2026-06-13T09:00:00Z",
        "the provenance timestamp comes from the injected clock"
    );
}

#[test]
fn clearing_a_cell_reopens_a_to_fill_gap_never_zero() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    drop(
        Journal::create(
            &path,
            Uuid::from_u128(0xA),
            &Timestamp("2026-06-13T00:00:00Z".to_string()),
        )
        .unwrap(),
    );
    let mut state = open_state(&path);
    let id = state.create_study("NESN", "CHF").unwrap();

    state
        .edit_cell(id, 1, entry::FIELD_EPS, Some(money("4.2")))
        .unwrap();
    state.edit_cell(id, 1, entry::FIELD_EPS, None).unwrap(); // clear it

    let cell = open_state(&path).get_study(id).unwrap().years[1]
        .eps
        .clone();
    assert_eq!(cell.value, None, "a cleared cell holds no value — never 0");
    assert_eq!(
        cell.coverage,
        Coverage::ToFill,
        "a cleared cell is a to-fill gap"
    );
}

#[test]
fn not_available_is_a_distinct_quiet_gap_that_survives_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    drop(
        Journal::create(
            &path,
            Uuid::from_u128(0xB),
            &Timestamp("2026-06-13T00:00:00Z".to_string()),
        )
        .unwrap(),
    );
    let mut state = open_state(&path);
    let id = state.create_study("NESN", "CHF").unwrap();

    // An optional column (dividend) marked not-available: distinct from to-fill and from 0.
    state
        .set_not_available(id, 2, entry::FIELD_DIVIDEND, true)
        .unwrap();
    let cell = open_state(&path).get_study(id).unwrap().years[2]
        .dividend_per_share
        .clone()
        .expect("the cell now exists");
    assert_eq!(cell.value, None);
    assert_eq!(cell.coverage, Coverage::NotAvailableAccepted);

    // Clearing it back returns a to-fill gap.
    state
        .set_not_available(id, 2, entry::FIELD_DIVIDEND, false)
        .unwrap();
    let back = open_state(&path).get_study(id).unwrap().years[2]
        .dividend_per_share
        .clone()
        .unwrap();
    assert_eq!(back.coverage, Coverage::ToFill);
}

// ── Story 2.5: tri-state review tag set/clear → persist → reopen; soft-lock; bulk unlock ──

/// Open a journal at `path`, create a study, and fill A/C on year 0 so there is a present cell to
/// review. Returns the state (still open) and the study id.
fn study_with_entry(path: &Path) -> (JournalState, Uuid) {
    drop(
        Journal::create(
            path,
            Uuid::from_u128(0xC0FFEE),
            &Timestamp("2026-06-13T00:00:00Z".to_string()),
        )
        .unwrap(),
    );
    let mut state = open_state(path);
    let id = state.create_study("NESN", "CHF").unwrap();
    state
        .edit_cell(id, 0, entry::FIELD_HIGH, Some(money("120.5")))
        .unwrap();
    (state, id)
}

#[test]
fn set_review_survives_reopen_and_leaves_value_and_coverage_untouched() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);

    // none → ? → ✓, each persisted; the value and coverage never move (review-only edits).
    state
        .set_review(id, 0, entry::FIELD_HIGH, Review::ToReview)
        .unwrap();
    let cell = open_state(&path).get_study(id).unwrap().years[0]
        .high_price
        .clone();
    assert_eq!(cell.review, Review::ToReview);
    assert_eq!(cell.value, Some(money("120.5")), "value untouched by ?");
    assert_eq!(cell.coverage, Coverage::Present, "coverage untouched by ?");

    state
        .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
        .unwrap();
    let cell = open_state(&path).get_study(id).unwrap().years[0]
        .high_price
        .clone();
    assert_eq!(cell.review, Review::Validated, "✓ survives reopen");
    assert_eq!(cell.value, Some(money("120.5")));
    assert_eq!(cell.coverage, Coverage::Present);

    // ✓ → none clears the tag; still a review-only change.
    state
        .set_review(id, 0, entry::FIELD_HIGH, Review::None)
        .unwrap();
    let cell = open_state(&path).get_study(id).unwrap().years[0]
        .high_price
        .clone();
    assert_eq!(cell.review, Review::None);
    assert_eq!(
        cell.value,
        Some(money("120.5")),
        "clearing ✓ keeps the value"
    );
}

#[test]
fn reviewing_a_to_fill_gap_keeps_the_value_none_never_zero() {
    // Setting a tag on a never-entered optional column materializes a to-fill cell carrying the
    // tag — the value stays None (the project's most-repeated rail: unknown is never 0).
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);

    state
        .set_review(id, 2, entry::FIELD_DIVIDEND, Review::ToReview)
        .unwrap();
    let cell = open_state(&path).get_study(id).unwrap().years[2]
        .dividend_per_share
        .clone()
        .expect("the cell now exists");
    assert_eq!(cell.review, Review::ToReview);
    assert_eq!(cell.value, None, "a reviewed gap holds no value — never 0");
    assert_eq!(cell.coverage, Coverage::ToFill);
}

#[test]
fn an_empty_cell_cannot_be_validated_issue_47() {
    // #47: validating a value-less cell must be refused (a neutral no-op) — otherwise a later
    // refresh gap-fills it, resets the review to None, and the ✓ vanishes silently (escaping the
    // ✓→? re-validate count). `?` on a gap stays allowed (flag a column to fill).
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);

    // A never-touched optional column: `?` materializes a to-fill gap (allowed)…
    state
        .set_review(id, 2, entry::FIELD_DIVIDEND, Review::ToReview)
        .unwrap();
    // …but `✓` on the still-empty cell is refused — review stays `?`, value stays None.
    state
        .set_review(id, 2, entry::FIELD_DIVIDEND, Review::Validated)
        .unwrap();
    let cell = open_state(&path).get_study(id).unwrap().years[2]
        .dividend_per_share
        .clone()
        .expect("the gap cell exists");
    assert_eq!(
        cell.review,
        Review::ToReview,
        "an empty cell cannot reach ✓ — the validate is a no-op"
    );
    assert_eq!(cell.value, None, "still no value — never materialized to 0");

    // Validating a never-touched column (cell does not exist yet) is likewise refused: it must
    // not materialize a Validated empty gap (the same bug, via materialization).
    state
        .set_review(id, 1, entry::FIELD_DIVIDEND, Review::Validated)
        .unwrap();
    assert!(
        open_state(&path).get_study(id).unwrap().years[1]
            .dividend_per_share
            .is_none(),
        "validating a never-touched empty column materializes nothing"
    );
}

#[test]
fn a_validated_cell_is_soft_locked_until_the_tag_is_cleared() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);

    state
        .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
        .unwrap();

    // A direct typed edit on the ✓ cell is refused with the neutral soft-lock notice, and the
    // on-disk value is unchanged (never silently blanked or overwritten).
    assert_eq!(
        state.edit_cell(id, 0, entry::FIELD_HIGH, Some(money("999"))),
        Err(MSG_SOFT_LOCKED.to_string())
    );
    let cell = open_state(&path).get_study(id).unwrap().years[0]
        .high_price
        .clone();
    assert_eq!(
        cell.value,
        Some(money("120.5")),
        "the refused edit wrote nothing"
    );
    assert_eq!(cell.review, Review::Validated, "the sign-off is intact");

    // The deliberate clear-✓ → ? releases the lock (recheck status preserved, not blanked).
    state
        .set_review(id, 0, entry::FIELD_HIGH, Review::ToReview)
        .unwrap();
    // Now the cell edits normally again.
    state
        .edit_cell(id, 0, entry::FIELD_HIGH, Some(money("130")))
        .expect("a ? cell edits normally");
    let cell = open_state(&path).get_study(id).unwrap().years[0]
        .high_price
        .clone();
    assert_eq!(cell.value, Some(money("130")));
    assert_eq!(cell.review, Review::ToReview, "editing a ? cell keeps ?");
}

#[test]
fn set_not_available_is_refused_on_a_validated_cell_so_the_sign_off_is_never_blanked() {
    // The not-available gesture (Ctrl+Space) is a value/coverage mutation; on a `✓` cell it would
    // otherwise route through `Cell::edited(None, …)`, blanking the value AND demoting `✓ → ?`.
    // AC 2 forbids that — the soft-lock backstop must refuse it just like a typed edit does.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);

    state
        .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
        .unwrap();
    assert_eq!(
        state.set_not_available(id, 0, entry::FIELD_HIGH, true),
        Err(MSG_SOFT_LOCKED.to_string()),
        "not-available on a ✓ cell is refused"
    );
    // The on-disk cell is untouched: value kept, sign-off intact, still a present cell.
    let cell = open_state(&path).get_study(id).unwrap().years[0]
        .high_price
        .clone();
    assert_eq!(cell.value, Some(money("120.5")), "value untouched");
    assert_eq!(cell.review, Review::Validated, "sign-off intact");
    assert_eq!(cell.coverage, Coverage::Present, "coverage untouched");
}

#[test]
fn unlock_all_flips_only_validated_cells_at_each_scope_and_persists() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);

    // Build a mixed field of tags across years/fields:
    //   (y0, A) ✓   (y1, A) ✓   (y0, C) ?   (y2, B) ✓
    state
        .edit_cell(id, 1, entry::FIELD_HIGH, Some(money("1")))
        .unwrap();
    state
        .edit_cell(id, 0, entry::FIELD_EPS, Some(money("2")))
        .unwrap();
    state
        .edit_cell(id, 2, entry::FIELD_LOW, Some(money("3")))
        .unwrap();
    state
        .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
        .unwrap();
    state
        .set_review(id, 1, entry::FIELD_HIGH, Review::Validated)
        .unwrap();
    state
        .set_review(id, 0, entry::FIELD_EPS, Review::ToReview)
        .unwrap();
    state
        .set_review(id, 2, entry::FIELD_LOW, Review::Validated)
        .unwrap();

    // ── Per-metric scope (field A): flips (y0,A) and (y1,A) only; (y2,B) ✓ is left.
    assert_eq!(
        state.count_validated(id, &UnlockScope::Metric(entry::FIELD_HIGH.to_string())),
        2
    );
    let flipped = state
        .unlock_all(id, &UnlockScope::Metric(entry::FIELD_HIGH.to_string()))
        .unwrap();
    assert_eq!(flipped, 2, "two A cells flipped");
    let back = open_state(&path).get_study(id).unwrap();
    assert_eq!(back.years[0].high_price.review, Review::ToReview);
    assert_eq!(back.years[1].high_price.review, Review::ToReview);
    assert_eq!(
        back.years[0].eps.review,
        Review::ToReview,
        "the ? cell is untouched"
    );
    assert_eq!(
        back.years[2].low_price.review,
        Review::Validated,
        "a different metric keeps its ✓"
    );

    // ── Per-year scope (year 2): flips (y2,B) only.
    let flipped = state.unlock_all(id, &UnlockScope::Year(2)).unwrap();
    assert_eq!(flipped, 1);
    assert_eq!(
        open_state(&path).get_study(id).unwrap().years[2]
            .low_price
            .review,
        Review::ToReview
    );

    // ── Study scope on an already-cleared study: nothing left to flip.
    assert_eq!(state.unlock_all(id, &UnlockScope::Study).unwrap(), 0);
}

#[test]
fn unlock_all_study_scope_flips_every_validated_cell_in_one_upsert() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);
    state
        .edit_cell(id, 3, entry::FIELD_EPS, Some(money("9")))
        .unwrap();
    state
        .set_review(id, 0, entry::FIELD_HIGH, Review::Validated)
        .unwrap();
    state
        .set_review(id, 3, entry::FIELD_EPS, Review::Validated)
        .unwrap();

    assert_eq!(state.count_validated(id, &UnlockScope::Study), 2);
    let flipped = state.unlock_all(id, &UnlockScope::Study).unwrap();
    assert_eq!(flipped, 2);
    let back = open_state(&path).get_study(id).unwrap();
    assert_eq!(back.years[0].high_price.review, Review::ToReview);
    assert_eq!(back.years[3].eps.review, Review::ToReview);
    assert_eq!(
        back.years[0].high_price.value,
        Some(money("120.5")),
        "values untouched"
    );
}

#[test]
fn unlock_messages_substitute_the_count() {
    assert_eq!(
        unlock_confirm_message(3),
        "Cette action retire la validation de 3 cellule(s)."
    );
    assert_eq!(
        unlock_done_message(1),
        "Validation retirée de 1 cellule(s)."
    );
}

// ── Story 2.6: numeric judgment editing → persist → reopen; snapshot_for engine wiring ──

#[test]
fn judgment_fields_round_trip_and_clear_to_none_never_zero() {
    use steadyinvest_contract::ForecastLowOption;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);

    // Set each numeric judgment field; the option selector; then reopen and verify each survives.
    for (field, value) in [
        ("sales_growth", "12.5"),
        ("eps_growth", "10"),
        ("est_high_eps", "8.4"),
        ("est_low_eps", "3.1"),
        ("high_pe", "22"),
        ("low_pe", "11"),
        ("recent_severe_low", "44.5"),
        ("current_price", "60"),
        ("dividend", "2.25"),
    ] {
        state
            .set_judgment_field(id, field, Some(money(value)))
            .unwrap();
    }
    state
        .set_forecast_low_option(id, ForecastLowOption::RecentSevereLow)
        .unwrap();

    let j = open_state(&path).get_study(id).unwrap().judgment;
    assert_eq!(j.projected_sales_growth_pct, Some(money("12.5")));
    assert_eq!(j.estimated_high_eps, Some(money("8.4")));
    assert_eq!(j.judged_avg_high_pe, Some(money("22")));
    assert_eq!(j.current_price, Some(money("60")));
    assert_eq!(j.present_full_year_dividend, Some(money("2.25")));
    assert_eq!(j.forecast_low_option, ForecastLowOption::RecentSevereLow);

    // Clearing a field stores None — never 0.
    state.set_judgment_field(id, "current_price", None).unwrap();
    let j = open_state(&path).get_study(id).unwrap().judgment;
    assert_eq!(
        j.current_price, None,
        "a cleared judgment field is None, never 0"
    );
}

#[test]
fn snapshot_for_runs_the_engine_and_matches_build_snapshot() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);
    // Fill the load-bearing cells + judgment so the snapshot is computable.
    for y in 0..entry::YEAR_WINDOW {
        for (field, v) in [
            (entry::FIELD_HIGH, "100"),
            (entry::FIELD_LOW, "50"),
            (entry::FIELD_EPS, "5"),
        ] {
            state.edit_cell(id, y, field, Some(money(v))).unwrap();
        }
    }
    for (field, v) in [
        ("est_high_eps", "8"),
        ("est_low_eps", "3"),
        ("high_pe", "20"),
        ("low_pe", "10"),
        ("current_price", "60"),
    ] {
        state.set_judgment_field(id, field, Some(money(v))).unwrap();
    }

    let snap = state.snapshot_for(id).expect("snapshot computes");
    // No drift: the state-level snapshot equals the pure adapter snapshot on the same study.
    let study = state.get_study(id).unwrap();
    let direct = crate::viewmodel::engine::build_snapshot(&study).unwrap();
    assert_eq!(snap.outputs(), direct.outputs());
    assert_eq!(snap.verdict(), direct.verdict());
}

#[test]
fn an_edit_on_a_read_only_journal_is_refused_and_writes_nothing() {
    // A study created in a writable journal, then reopened read-only: the edit is refused with the
    // neutral notice and the on-disk value is unchanged.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    drop(
        Journal::create(
            &path,
            Uuid::from_u128(0xC),
            &Timestamp("2026-06-13T00:00:00Z".to_string()),
        )
        .unwrap(),
    );
    let id = {
        let mut state = open_state(&path);
        state.create_study("NESN", "CHF").unwrap()
    };
    // Force a read-only state by constructing one whose `read_only` flag is set.
    let mut state = open_state(&path);
    state.read_only = true;
    assert_eq!(
        state.edit_cell(id, 0, entry::FIELD_HIGH, Some(money("1"))),
        Err(MSG_READ_ONLY_WRITE.to_string())
    );
    // Nothing was written: the cell is still absent/empty.
    let back = open_state(&path).get_study(id).unwrap();
    assert!(back.years.is_empty(), "a refused edit materialized nothing");
}

// ── Story 2.12 — dashboard archive (soft) & delete (hard) state wrappers ──

fn status_in_list(state: &JournalState, id: Uuid) -> Option<String> {
    state
        .list_studies()
        .into_iter()
        .find(|s| s.id == id)
        .map(|s| s.status)
}

#[test]
fn archive_then_unarchive_flips_status_reversibly() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);
    assert_eq!(status_in_list(&state, id).as_deref(), Some("active"));

    state.archive_study(id).expect("archive");
    assert_eq!(status_in_list(&state, id).as_deref(), Some("archived"));

    state.unarchive_study(id).expect("un-archive");
    assert_eq!(status_in_list(&state, id).as_deref(), Some("active"));
}

#[test]
fn delete_removes_the_study_from_the_list() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let (mut state, id) = study_with_entry(&path);
    assert!(
        status_in_list(&state, id).is_some(),
        "present before delete"
    );

    state.delete_study(id).expect("delete");
    assert!(
        status_in_list(&state, id).is_none(),
        "the deleted study is gone from the list"
    );
    assert!(
        state.get_study(id).is_none(),
        "the deleted study is unreadable"
    );
}

#[test]
fn archive_and_delete_are_refused_on_a_read_only_journal() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let id = study_with_entry(&path).1;

    let mut state = open_state(&path);
    state.read_only = true;
    assert_eq!(
        state.archive_study(id),
        Err(MSG_READ_ONLY_WRITE.to_string())
    );
    assert_eq!(state.delete_study(id), Err(MSG_READ_ONLY_WRITE.to_string()));
    // Nothing changed on disk: the study is still present and active.
    assert_eq!(
        status_in_list(&open_state(&path), id).as_deref(),
        Some("active"),
        "a refused archive/delete mutated nothing"
    );
}

// ── Story 6.5 — FX acquisition: dated, source-aware rates (FR28) ──

#[test]
fn a_manual_fx_rate_records_dated_and_sourced_and_reupserts_in_place() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x650);
    state
        .upsert_manual_fx_rate("eur", "0.93", "2026-06-26", "CHF")
        .expect("the manual rate records");
    let rates = state.list_fx_rates();
    assert_eq!(rates.len(), 1);
    assert_eq!(rates[0].base_currency, "EUR", "uppercased");
    assert_eq!(rates[0].quote_currency, "CHF");
    assert_eq!(rates[0].rate, "0.93");
    assert_eq!(rates[0].rate_date, "2026-06-26");
    assert_eq!(rates[0].source, "manuel");

    // Same (pair, date, source) with a corrected rate → update in place, no duplicate.
    state
        .upsert_manual_fx_rate("EUR", "0.94", "2026-06-26", "CHF")
        .unwrap();
    let rates = state.list_fx_rates();
    assert_eq!(rates.len(), 1, "no duplicate row");
    assert_eq!(rates[0].rate, "0.94");
}

#[test]
fn deleting_a_fx_rate_by_id_removes_it_and_a_stale_id_is_a_benign_no_op() {
    // Issue #90: the panel's repair path. A real removal reports true; an unparseable/absent id is
    // Ok(false) (no error) so a stale UI row never blows up.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x653);
    state
        .upsert_manual_fx_rate("EUR", "0.94", "2026-06-26", "CHF")
        .unwrap();
    state
        .upsert_manual_fx_rate("USD", "0.88", "2026-06-26", "CHF")
        .unwrap();
    let eur_id = state
        .list_fx_rates()
        .into_iter()
        .find(|r| r.base_currency == "EUR")
        .expect("the EUR rate exists")
        .id
        .to_string();

    assert_eq!(
        state.delete_fx_rate(&eur_id),
        Ok(true),
        "deleting a present id reports removed"
    );
    let rates = state.list_fx_rates();
    assert_eq!(rates.len(), 1, "only the targeted row is gone");
    assert_eq!(rates[0].base_currency, "USD", "the other row is untouched");

    // A garbage id (a stale UI row) is a benign no-op, never an error.
    assert_eq!(
        state.delete_fx_rate("not-a-uuid"),
        Ok(false),
        "an unparseable id is a benign no-op"
    );
    assert_eq!(state.list_fx_rates().len(), 1, "nothing else removed");
}

#[test]
fn manual_fx_validation_refuses_neutrally() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x651);
    assert_eq!(
        state.upsert_manual_fx_rate("CHF", "0.93", "", "CHF"),
        Err(MSG_FX_SAME_CURRENCY.to_string()),
        "base == reference refused"
    );
    assert_eq!(
        state.upsert_manual_fx_rate("EUR", "0", "", "CHF"),
        Err(MSG_FX_INVALID_RATE.to_string())
    );
    assert_eq!(
        state.upsert_manual_fx_rate("EUR", "-1", "", "CHF"),
        Err(MSG_FX_INVALID_RATE.to_string())
    );
    assert_eq!(
        state.upsert_manual_fx_rate("SEK", "0.93", "", "CHF"),
        Err(MSG_FX_INVALID_CURRENCY.to_string()),
        "an off-allow-list base is a CURRENCY refusal, not a rate one (review)"
    );
    assert_eq!(
        state.upsert_manual_fx_rate("EUR", "0.93", "2027-01-01", "CHF"),
        Err(MSG_FX_FUTURE_DATE.to_string()),
        "a future-dated rate would win the latest arbitration until that day (review)"
    );
    assert_eq!(
        state.upsert_manual_fx_rate("EUR", "0.93", "2026-02-30", "CHF"),
        Err(MSG_LEDGER_INVALID_DATE.to_string()),
        "the 6.3 real-calendar validation is reused"
    );
    assert!(state.list_fx_rates().is_empty(), "nothing written");
}

#[test]
fn foreign_currencies_in_use_covers_all_portfolio_holdings_minus_the_reference() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x652);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    state.add_holding("AAPL", "5", "150", "USD").unwrap();
    state.add_holding("ASML", "2", "600", "EUR").unwrap();
    // A SOLD USD holding still counts (its history feeds the 6.6 consolidation).
    let usd_id = state
        .list_holdings()
        .iter()
        .find(|h| h.security_ticker == "AAPL")
        .unwrap()
        .id;
    state.sell_holding(usd_id, "", "", "CHF").unwrap();

    assert_eq!(
        state.foreign_currencies_in_use("CHF"),
        vec!["EUR".to_string(), "USD".to_string()],
        "deterministic, deduplicated, reference excluded, sold holdings counted"
    );
}

#[test]
fn apply_fx_fetch_stamps_the_day_and_the_provider_source() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x653);
    state
        .apply_fx_fetch(
            "USD",
            "CHF",
            Decimal::from_str_exact("0.8850").unwrap(),
            None, // Twelve Data's bare `/price` is undated (issue #90 part 3) → the fetch-day fallback.
            "twelvedata",
        )
        .expect("the fetched rate lands");
    let rates = state.list_fx_rates();
    assert_eq!(rates.len(), 1);
    assert_eq!(rates[0].rate, "0.885", "normalized spelling");
    assert_eq!(
        rates[0].rate_date, "2026-06-27",
        "no session date supplied → the fetch DAY (the fixed test clock)"
    );
    assert_eq!(rates[0].source, "twelvedata");
}

/// Issue #90 (part 3): a provider-supplied session date is used verbatim (a Friday close fetched
/// on the following Monday stays dated Friday — no weekend phantom row), instead of always the
/// fetch day.
#[test]
fn apply_fx_fetch_uses_the_provider_session_date_over_the_fetch_day() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x654);
    state
        .apply_fx_fetch(
            "EUR",
            "CHF",
            Decimal::from_str_exact("0.9312").unwrap(),
            Some("2026-06-26"), // a Friday; the fixed test clock's "today" is 2026-06-27 (Saturday).
            "eodhd",
        )
        .expect("the fetched rate lands");
    let rates = state.list_fx_rates();
    assert_eq!(rates.len(), 1);
    assert_eq!(
        rates[0].rate_date, "2026-06-26",
        "the real session date wins over the (later) fetch day"
    );
}

#[test]
fn a_corrected_manual_rate_wins_the_same_day_tie_over_an_earlier_provider_row() {
    // Review HIGH: the in-place update refreshes created_at, so the LATEST write always wins
    // the same-day arbitration — a user's evening correction outranks the mid-day provider row.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x655);
    state
        .upsert_manual_fx_rate("EUR", "0.93", "2026-06-27", "CHF")
        .unwrap();
    state
        .apply_fx_fetch(
            "EUR",
            "CHF",
            Decimal::from_str_exact("0.94").unwrap(),
            Some("2026-06-27"),
            "eodhd",
        )
        .unwrap();
    // The user corrects the manual rate LAST (same natural key → in-place update).
    state
        .upsert_manual_fx_rate("EUR", "0.95", "2026-06-27", "CHF")
        .unwrap();

    let latest = state
        .journal
        .as_ref()
        .unwrap()
        .latest_fx_rate("EUR", "CHF", None)
        .unwrap()
        .expect("a rate exists");
    assert_eq!(latest.rate, "0.95", "the corrected manual rate wins");
    assert_eq!(latest.source, "manuel");
}

#[test]
fn an_off_list_holding_currency_never_poisons_the_fetch_pair_set() {
    // Review MED: an imported "SEK" holding must not put an unfetchable pair into the set.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x656);
    state.add_holding("AAPL", "5", "150", "USD").unwrap();
    // Plant an off-list currency straight at persistence (the import path's freedom).
    let pid = state.active_portfolio().unwrap().id;
    state
        .journal
        .as_mut()
        .unwrap()
        .add_holding(
            Uuid::from_u128(0x5EC),
            pid,
            "ERIC",
            "10",
            "50",
            "SEK",
            &Timestamp("2026-06-27T15:00:00Z".to_string()),
        )
        .unwrap();
    assert_eq!(
        state.foreign_currencies_in_use("CHF"),
        vec!["USD".to_string()],
        "the off-list SEK is excluded from the fetch set"
    );
}

#[test]
fn fx_writes_are_refused_on_a_read_only_journal() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x654);
    state.read_only = true;
    assert_eq!(
        state.upsert_manual_fx_rate("EUR", "0.93", "", "CHF"),
        Err(MSG_READ_ONLY_WRITE.to_string())
    );
}

// ── Story 6.6 — capital-at-risk per currency → per bank → global (FR44) ──

#[test]
fn the_consolidation_converts_per_bank_and_globally_with_exact_rates() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x660);
    // Bank 1 (the default): CHF 10@100 stop 85 → CaR 150; USD 4@50 stop 40 → CaR 40.
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    state.add_holding("AAPL", "4", "50", "USD").unwrap();
    let ids: Vec<_> = state.list_holdings().iter().map(|h| h.id).collect();
    state.set_holding_trailing_stop(ids[0], "15").unwrap();
    state.set_holding_trailing_stop(ids[1], "20").unwrap();
    // Bank 2: EUR 10@20 stop 15 → CaR 50.
    let bank2 = state.add_portfolio("PostFinance").unwrap();
    state.add_holding("ASML", "10", "20", "EUR").unwrap();
    let eur_id = state.list_holdings()[0].id;
    state.set_holding_trailing_stop(eur_id, "25").unwrap();
    let _ = bank2;
    // Rates: USD→CHF 0.5 (CaR 40 → 20), EUR→CHF 2 (CaR 50 → 100).
    state
        .upsert_manual_fx_rate("USD", "0.5", "2026-06-27", "CHF")
        .unwrap();
    state
        .upsert_manual_fx_rate("EUR", "2", "2026-06-27", "CHF")
        .unwrap();

    let view = state.journal_capital_at_risk_consolidation("CHF");
    assert_eq!(view.banks.len(), 2);
    let bank1 = &view.banks[0];
    assert_eq!(
        bank1.converted.unwrap().0,
        Decimal::from(170),
        "150 CHF + 40 USD × 0.5 = 170 CHF"
    );
    let bank2 = &view.banks[1];
    assert_eq!(
        bank2.converted.unwrap().0,
        Decimal::from(100),
        "50 EUR × 2 = 100 CHF"
    );
    assert_eq!(
        view.global.unwrap().0,
        Decimal::from(270),
        "the global total"
    );
    assert_eq!(
        view.rates_used.len(),
        2,
        "both rates named for the footnote"
    );
    assert!(view.missing_pairs.is_empty());
}

#[test]
fn a_missing_pair_absents_the_bank_and_the_global_by_name() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x661);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    state.add_holding("AAPL", "4", "50", "USD").unwrap();
    let ids: Vec<_> = state.list_holdings().iter().map(|h| h.id).collect();
    state.set_holding_trailing_stop(ids[1], "20").unwrap();
    // NO USD→CHF rate stored.
    let view = state.journal_capital_at_risk_consolidation("CHF");
    assert!(
        view.banks[0].converted.is_none(),
        "the bank cannot consolidate"
    );
    assert_eq!(view.banks[0].missing_pairs, vec!["USD → CHF".to_string()]);
    assert!(
        view.global.is_none(),
        "never a partial sum passed off as total"
    );
    assert_eq!(view.missing_pairs, vec!["USD → CHF".to_string()]);

    // The rate arrives → the next read consolidates.
    state
        .upsert_manual_fx_rate("USD", "0.5", "2026-06-27", "CHF")
        .unwrap();
    let view = state.journal_capital_at_risk_consolidation("CHF");
    assert!(view.global.is_some());
    assert!(view.missing_pairs.is_empty());
}

#[test]
fn reference_buckets_convert_at_identity_and_sold_holdings_stay_excluded() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x662);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    state.set_holding_trailing_stop(id, "15").unwrap();
    // A sold USD holding: no rate stored for USD, but a sold position carries no risk — it must
    // neither require the pair nor block the global.
    state.add_holding("AAPL", "4", "50", "USD").unwrap();
    let usd_id = state
        .list_holdings()
        .iter()
        .find(|h| h.security_ticker == "AAPL")
        .unwrap()
        .id;
    state.sell_holding(usd_id, "", "", "CHF").unwrap();

    let view = state.journal_capital_at_risk_consolidation("CHF");
    assert_eq!(
        view.global.unwrap().0,
        Decimal::from(150),
        "identity conversion for CHF; the sold USD position is not position risk"
    );
    assert!(
        view.missing_pairs.is_empty(),
        "no pair required for a sold holding"
    );
    assert!(view.rates_used.is_empty(), "no rate looked up at all");
}

/// Inject a holding straight into the journal, bypassing `add_holding`'s issue-#60 magnitude bound —
/// to simulate a PRE-EXISTING / externally-edited absurd row that the render-side overflow guards
/// must still handle gracefully (the write bound only stops NEW absurd input).
fn inject_raw_holding(
    state: &mut JournalState,
    ticker: &str,
    qty: &str,
    price: &str,
    currency: &str,
) {
    let portfolio_id = match state.active_portfolio() {
        Some(p) => p.id,
        None => state.add_portfolio("Portefeuille").unwrap(),
    };
    let id = state.idgen.new_id();
    let created_at = state.clock.now();
    state
        .journal
        .as_mut()
        .unwrap()
        .add_holding(id, portfolio_id, ticker, qty, price, currency, &created_at)
        .unwrap();
}

#[test]
fn a_checked_overflow_absents_the_bank_plainly_never_a_wrong_figure() {
    // AC5/review: an overflowing conversion is an ABSENT subtotal marked `unavailable` (no
    // dangling empty line, no partial global) — never a corrupt number.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x663);
    // A USD position whose saturated CaR is Decimal::MAX-scale: converting at 2 overflows. Injected
    // raw (issue #60): an absurd magnitude can only reach the render path as pre-existing data now.
    inject_raw_holding(
        &mut state,
        "HUGE",
        "79228162514264337593543950335",
        "1",
        "USD",
    );
    let id = state.list_holdings()[0].id;
    state.set_holding_trailing_stop(id, "15").unwrap();
    state
        .upsert_manual_fx_rate("USD", "2", "2026-06-27", "CHF")
        .unwrap();

    let view = state.journal_capital_at_risk_consolidation("CHF");
    let bank = &view.banks[0];
    assert!(bank.converted.is_none(), "absent, never wrong");
    assert!(
        bank.missing_pairs.is_empty(),
        "no pair is missing — the rate exists"
    );
    assert!(bank.unavailable, "marked plainly unavailable (overflow)");
    assert!(view.global.is_none(), "the global never sums a broken bank");
}

#[test]
fn unstopped_exposure_counts_holdings_without_a_stop_per_currency() {
    // Issue #61: capital-at-risk counts only stop-protected downside, so a stop-less portfolio reads
    // "0 % at risk". This neutral fact names the un-protected exposure per currency instead.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x610);
    state.add_holding("NESN", "10", "100", "CHF").unwrap(); // 1000 CHF, no stop
    state.add_holding("ABBN", "5", "40", "CHF").unwrap(); // 200 CHF, will be stopped
    state.add_holding("AAPL", "2", "150", "USD").unwrap(); // 300 USD, no stop
    let abbn = state
        .list_holdings()
        .into_iter()
        .find(|h| h.security_ticker == "ABBN")
        .unwrap()
        .id;
    state.set_holding_trailing_stop(abbn, "15").unwrap();
    // CHF: only NESN is un-stopped now (ABBN protected) → 1 position, 1000; USD: AAPL → 1, 300.
    assert_eq!(
        state.portfolio_unstopped_exposure_by_currency("CHF"),
        vec![
            ("CHF".to_string(), 1, Decimal::from(1000)),
            ("USD".to_string(), 1, Decimal::from(300)),
        ]
    );
    // The composed line pluralizes the count and carries the uncovered value.
    assert_eq!(
        unstopped_exposure_notice(1, "1 000,00 CHF"),
        "1 position sans seuil suiveur (1 000,00 CHF non couverts)"
    );
    assert_eq!(
        unstopped_exposure_notice(3, "5 000,00 CHF"),
        "3 positions sans seuil suiveur (5 000,00 CHF non couverts)"
    );
    // Protect every remaining holding → the exposure is empty (nothing to warn about).
    for t in ["NESN", "AAPL"] {
        let id = state
            .list_holdings()
            .into_iter()
            .find(|h| h.security_ticker == t)
            .unwrap()
            .id;
        state.set_holding_trailing_stop(id, "10").unwrap();
    }
    assert!(
        state
            .portfolio_unstopped_exposure_by_currency("CHF")
            .is_empty(),
        "every holding protected → no un-stopped exposure line"
    );
}

// ── Story 6.7 — concentration on total capital + diversify-by-size (FR45) ──

/// Bank 1: NESN 10@100 CHF (1000) + AAPL 4@50 USD (200 USD); Bank 2: NESN 5@100 CHF (500).
/// USD→CHF 0.5 → AAPL = 100 CHF; NESN = 1500 CHF across banks; global = 1600 CHF.
fn diversification_fixture(dir: &TempDir, seed: u128) -> JournalState {
    let mut state = watch_state(dir, seed);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    state.add_holding("AAPL", "4", "50", "USD").unwrap();
    state.add_portfolio("PostFinance").unwrap();
    state.add_holding("NESN", "5", "100", "CHF").unwrap();
    state
        .upsert_manual_fx_rate("USD", "0.5", "2026-06-27", "CHF")
        .unwrap();
    state
}

fn bounds() -> (Decimal, Decimal) {
    (
        Decimal::from(1_000_000_000i64),
        Decimal::from(10_000_000_000i64),
    )
}

#[test]
fn a_ticker_held_at_two_banks_is_one_concentration_line_with_an_exact_share() {
    // THE FR45 point (PRD Journey 3): concentration is against the TOTAL capital, regardless of
    // which bank or currency holds the security — NESN's two positions are ONE line.
    let dir = TempDir::new().unwrap();
    let state = diversification_fixture(&dir, 0x670);
    let (small, medium) = bounds();

    let view = state.journal_diversification("CHF", small, medium);
    assert!(!view.unavailable);
    assert_eq!(view.rows.len(), 2, "two securities, not three positions");
    assert_eq!(view.global_invested, Some(Decimal::from(1600)));
    // Largest share first.
    assert_eq!(view.rows[0].ticker, "NESN");
    assert_eq!(view.rows[0].invested, Some(Decimal::from(1500)));
    assert_eq!(
        view.rows[0].share_pct,
        Some(Decimal::from_str_exact("93.75").unwrap()),
        "1500 / 1600 — exact decimal, no rounding"
    );
    assert_eq!(view.rows[1].ticker, "AAPL");
    assert_eq!(
        view.rows[1].share_pct,
        Some(Decimal::from_str_exact("6.25").unwrap())
    );
    assert_eq!(view.rates_used.len(), 1, "USD→CHF named for the footnote");
    assert!(view.missing_pairs.is_empty());
}

#[test]
fn a_missing_rate_absents_the_security_and_the_denominator_by_name() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x671);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    state.add_holding("AAPL", "4", "50", "USD").unwrap();
    // No USD→CHF rate stored.
    let (small, medium) = bounds();
    let view = state.journal_diversification("CHF", small, medium);

    let aapl = view.rows.iter().find(|r| r.ticker == "AAPL").unwrap();
    assert_eq!(aapl.invested, None, "absent, never a partial figure");
    assert_eq!(aapl.missing_pairs, vec!["USD → CHF".to_string()]);
    assert_eq!(
        view.global_invested, None,
        "the denominator refuses — never a partial total passed off as the whole"
    );
    let nesn = view.rows.iter().find(|r| r.ticker == "NESN").unwrap();
    assert_eq!(
        nesn.share_pct, None,
        "even a fully-converted security has no share against an absent total"
    );
    assert_eq!(nesn.invested, Some(Decimal::from(1000)), "its figure stays");
    assert_eq!(view.missing_pairs, vec!["USD → CHF".to_string()]);
    // A rate arriving makes the next read consolidate (the 6.6 rule).
    state
        .upsert_manual_fx_rate("USD", "0.5", "2026-06-27", "CHF")
        .unwrap();
    let view = state.journal_diversification("CHF", small, medium);
    assert_eq!(view.global_invested, Some(Decimal::from(1100)));
}

#[test]
fn size_classification_joins_the_study_converts_sales_and_fills_the_mix() {
    let dir = TempDir::new().unwrap();
    let mut state = diversification_fixture(&dir, 0x672);
    // NESN's study (CHF): latest sales 2 000 000 000 → Medium (1e9 ≤ s ≤ 1e10).
    let nesn_study = state.create_study("NESN", "CHF").unwrap();
    state
        .edit_cell(nesn_study, 4, entry::FIELD_SALES, Some(money("2000000000")))
        .unwrap();
    // AAPL's study (USD): latest sales 500 000 000 USD × 0.5 = 250 000 000 CHF → Small.
    let aapl_study = state.create_study("AAPL", "USD").unwrap();
    state
        .edit_cell(aapl_study, 4, entry::FIELD_SALES, Some(money("500000000")))
        .unwrap();

    let (small, medium) = bounds();
    let view = state.journal_diversification("CHF", small, medium);
    assert!(view.unclassified.is_empty());
    assert_eq!(
        view.medium.share_pct,
        Some(Decimal::from_str_exact("93.75").unwrap()),
        "NESN — 1500 / 1600"
    );
    assert_eq!(
        view.small.share_pct,
        Some(Decimal::from_str_exact("6.25").unwrap()),
        "AAPL — 100 / 1600"
    );
    assert_eq!(
        view.large.share_pct,
        Some(Decimal::ZERO),
        "0 % of a present total — a fact, not an absence"
    );
}

#[test]
fn an_unclassifiable_security_lands_in_the_honest_bucket_with_its_reason() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x673);
    // NOSTUDY: held, no study at all. NOSALES: a study with no sales value. EURSALES: a study
    // whose sales are in EUR — and no EUR→CHF rate stored (the holding itself is CHF, so its
    // INVESTED still converts at identity; only the CLASSIFICATION refuses).
    state.add_holding("NOSTUDY", "1", "100", "CHF").unwrap();
    state.add_holding("NOSALES", "1", "100", "CHF").unwrap();
    state.add_holding("EURSALES", "1", "100", "CHF").unwrap();
    state.create_study("NOSALES", "CHF").unwrap();
    let eur_study = state.create_study("EURSALES", "EUR").unwrap();
    state
        .edit_cell(eur_study, 4, entry::FIELD_SALES, Some(money("5000000000")))
        .unwrap();

    let (small, medium) = bounds();
    let view = state.journal_diversification("CHF", small, medium);
    assert_eq!(
        view.global_invested,
        Some(Decimal::from(300)),
        "every holding is CHF — the denominator is whole"
    );
    assert_eq!(view.unclassified.len(), 3);
    let reason_of = |ticker: &str| {
        &view
            .unclassified
            .iter()
            .find(|u| u.ticker == ticker)
            .unwrap()
            .reason
    };
    assert!(matches!(reason_of("NOSTUDY"), UnclassifiedReason::NoStudy));
    assert!(matches!(reason_of("NOSALES"), UnclassifiedReason::NoSales));
    match reason_of("EURSALES") {
        UnclassifiedReason::MissingRate(pair) => assert_eq!(pair, "EUR → CHF"),
        other => panic!(
            "expected MissingRate, got {}",
            match other {
                UnclassifiedReason::NoStudy => "NoStudy",
                UnclassifiedReason::NoSales => "NoSales",
                UnclassifiedReason::Unconvertible => "Unconvertible",
                UnclassifiedReason::StudyUnavailable => "StudyUnavailable",
                UnclassifiedReason::MissingRate(_) => unreachable!(),
            }
        ),
    }
    // No class received them — never a default class (0 % of a present total).
    assert_eq!(view.small.share_pct, Some(Decimal::ZERO));
    assert_eq!(view.medium.share_pct, Some(Decimal::ZERO));
    assert_eq!(view.large.share_pct, Some(Decimal::ZERO));
    // 2026-07-03 review: EUR → CHF blocks only the CLASSIFICATION — named on its « non classé »
    // row, never blamed for the shares' denominator (which is whole here).
    assert!(
        view.missing_pairs.is_empty(),
        "classification-only pairs stay off the denominator set"
    );
}

#[test]
fn a_nonpositive_latest_sales_refuses_to_classify() {
    // 2026-07-03 review: a negative (or zero) latest sales figure must not classify confidently
    // as Small — it is not a usable classification input, so the security lands in the honest
    // « non classé » bucket.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x677);
    state.add_holding("NEG", "1", "100", "CHF").unwrap();
    let study = state.create_study("NEG", "CHF").unwrap();
    state
        .edit_cell(study, 4, entry::FIELD_SALES, Some(money("-5")))
        .unwrap();

    let (small, medium) = bounds();
    let view = state.journal_diversification("CHF", small, medium);
    assert_eq!(view.unclassified.len(), 1);
    assert!(matches!(
        view.unclassified[0].reason,
        UnclassifiedReason::NoSales
    ));
    assert_eq!(view.small.share_pct, Some(Decimal::ZERO), "never Small");
}

#[test]
fn a_sold_holding_is_excluded_from_concentration() {
    // Position facts — the unchanged 4.6/6.2 semantics: a sold position carries no share of the
    // invested capital.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x674);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    state.add_holding("GONE", "5", "100", "CHF").unwrap();
    let gone = state
        .list_holdings()
        .iter()
        .find(|h| h.security_ticker == "GONE")
        .unwrap()
        .id;
    state.sell_holding(gone, "", "", "CHF").unwrap();

    let (small, medium) = bounds();
    let view = state.journal_diversification("CHF", small, medium);
    assert_eq!(view.rows.len(), 1, "the sold position left the view");
    assert_eq!(view.rows[0].ticker, "NESN");
    assert_eq!(view.global_invested, Some(Decimal::from(1000)));
}

#[test]
fn a_checked_overflow_absents_the_share_and_the_total_never_corrupts() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x675);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    // cost × qty overflows Decimal — the checked product is absent, never saturated into a share.
    // Injected raw (issue #60): the write bound now refuses such input, so overflow at render only
    // arises from pre-existing / externally-edited data — which is exactly what this guards.
    inject_raw_holding(
        &mut state,
        "HUGE",
        "2",
        "79228162514264337593543950335",
        "CHF",
    );

    let (small, medium) = bounds();
    let view = state.journal_diversification("CHF", small, medium);
    let huge = view.rows.iter().find(|r| r.ticker == "HUGE").unwrap();
    assert_eq!(huge.invested, None, "absent, never wrong");
    assert!(huge.missing_pairs.is_empty(), "no pair to blame — overflow");
    assert_eq!(view.global_invested, None, "the total never sums a break");
    assert_eq!(
        view.rows.last().unwrap().ticker,
        "HUGE",
        "absent rows sink below priced ones"
    );
}

#[test]
fn diversification_without_a_journal_is_unavailable_never_a_zero_state() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x676);
    state.journal = None;
    let (small, medium) = bounds();
    let view = state.journal_diversification("CHF", small, medium);
    assert!(view.unavailable, "an absence, never an empty-looking zero");
    assert!(view.rows.is_empty());
    assert_eq!(view.global_invested, None);
}

// ── Story 6.8 — replacement candidates on a sell (FR48) ──

/// A study with a §4 band (provider-filled years + complete judgment): high = 8×20 = 160,
/// low = 6×10 = 60 → buy_top = 60 + (160−60)/3. `current_price` positions it in/above the band.
fn banded_study(
    state: &mut JournalState,
    ticker: &str,
    currency: &str,
    current_price: i64,
) -> Uuid {
    let id = state.create_study(ticker, currency).unwrap();
    state
        .apply_provider_refresh(id, &fetched_for(&[2020, 2021, 2022, 2023, 2024]))
        .unwrap();
    for (field, v) in [
        ("est_high_eps", 8),
        ("est_low_eps", 6),
        ("high_pe", 20),
        ("low_pe", 10),
        ("current_price", current_price),
    ] {
        state
            .set_judgment_field(id, field, Some(und_money(v)))
            .unwrap();
    }
    id
}

#[test]
fn candidates_order_in_zone_then_distance_then_insufficient_then_unlinked() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x680);
    // Watchlist positions deliberately REVERSED vs the expected surfacing order.
    state.add_watch_item("EUNLINKED", None).unwrap(); // no study at all
    state.add_watch_item("CFAR", None).unwrap(); // far above the buy zone
    state.add_watch_item("BNEAR", None).unwrap(); // just above the buy zone
    state.add_watch_item("AIN", None).unwrap(); // inside the buy zone
    state.add_watch_item("DINSUF", None).unwrap(); // study without a band
    banded_study(&mut state, "CFAR", "CHF", 150);
    banded_study(&mut state, "BNEAR", "CHF", 100);
    banded_study(&mut state, "AIN", "CHF", 70);
    state.create_study("DINSUF", "CHF").unwrap(); // no judgment → no band

    let candidates = state.replacement_candidates("CHF").unwrap();
    let order: Vec<&str> = candidates.iter().map(|c| c.ticker.as_str()).collect();
    assert_eq!(
        order,
        vec!["AIN", "BNEAR", "CFAR", "DINSUF", "EUNLINKED"],
        "in-zone → ascending distance → insufficient → unlinked"
    );
    assert!(candidates[0].in_buy_zone);
    assert_eq!(candidates[0].zone_key, "buy");
    assert_eq!(candidates[0].data, CandidateData::Ok);
    assert_eq!(candidates[3].data, CandidateData::Insufficient);
    assert_eq!(candidates[4].data, CandidateData::NoStudy);
    assert_eq!(candidates[4].study_id, None, "an honest row, never dropped");
    // The link fell back to `study_id_for_ticker` (items were added with NO explicit link).
    assert!(candidates[0].study_id.is_some());
}

#[test]
fn candidate_distance_and_ud_are_exact_and_absent_when_undefined() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x681);
    state.add_watch_item("NEAR", None).unwrap();
    state.add_watch_item("BELOW", None).unwrap();
    let near = banded_study(&mut state, "NEAR", "CHF", 100);
    banded_study(&mut state, "BELOW", "CHF", 50); // below forecast_low → U/D Undefined

    // The expected relative distance derives from the SAME snapshot the read uses.
    let snapshot = engine::build_snapshot(&state.get_study(near).unwrap()).unwrap();
    let buy_top = snapshot
        .outputs()
        .risk_reward
        .zones
        .as_ref()
        .unwrap()
        .buy_top;
    let expected = (Decimal::from(100) - buy_top) / buy_top * Decimal::from(100);

    let candidates = state.replacement_candidates("CHF").unwrap();
    let near_c = candidates.iter().find(|c| c.ticker == "NEAR").unwrap();
    assert_eq!(near_c.distance_above_buy_pct, Some(expected));
    assert_eq!(
        near_c.ud_ratio,
        Some(Decimal::from_str_exact("1.5").unwrap()),
        "(160 − 100) / (100 − 60)"
    );
    let below_c = candidates.iter().find(|c| c.ticker == "BELOW").unwrap();
    assert_eq!(
        below_c.ud_ratio, None,
        "Undefined is an absence, never a number"
    );
    assert_eq!(below_c.distance_above_buy_pct, None, "below the band");
    assert_eq!(
        below_c.data,
        CandidateData::Insufficient,
        "the §4 zone is undefined below the band — an honest bucket"
    );
}

#[test]
fn held_share_and_currency_exposure_facts_join_the_candidate() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x682);
    // Held: NESN 6@100 CHF (600) + AAPL 8@100 USD × 0.5 = 400 CHF → global 1000.
    state.add_holding("NESN", "6", "100", "CHF").unwrap();
    state.add_holding("AAPL", "8", "100", "USD").unwrap();
    state
        .upsert_manual_fx_rate("USD", "0.5", "2026-06-27", "CHF")
        .unwrap();
    // Watched: NESN (held, CHF study), XYZ (unheld, USD study), EUR-study candidate (unheld ccy).
    state.add_watch_item("NESN", None).unwrap();
    state.add_watch_item("XYZ", None).unwrap();
    state.add_watch_item("EURC", None).unwrap();
    banded_study(&mut state, "NESN", "CHF", 70);
    banded_study(&mut state, "XYZ", "USD", 70);
    banded_study(&mut state, "EURC", "EUR", 70);

    let candidates = state.replacement_candidates("CHF").unwrap();
    let by = |t: &str| candidates.iter().find(|c| c.ticker == t).unwrap();
    assert_eq!(
        by("NESN").held_share_pct,
        Some(Decimal::from(60)),
        "600 / 1000 — the already-held fact"
    );
    assert_eq!(by("XYZ").held_share_pct, None, "not held → no fact");
    assert_eq!(
        by("XYZ").currency_share_pct,
        Some(Decimal::from(40)),
        "USD already carries 400 / 1000 of the capital"
    );
    assert_eq!(
        by("NESN").currency_share_pct,
        Some(Decimal::from(60)),
        "CHF exposure"
    );
    assert_eq!(
        by("EURC").currency_share_pct,
        Some(Decimal::ZERO),
        "an unheld currency is an honest 0 % when the total is known"
    );
    assert_eq!(by("EURC").currency_missing_pair, None);
}

#[test]
fn currency_exposure_refuses_honestly_on_a_missing_pair() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x683);
    state.add_holding("NESN", "6", "100", "CHF").unwrap();
    state.add_holding("AAPL", "8", "100", "USD").unwrap();
    // No USD→CHF rate stored.
    let exposure = state.journal_currency_exposure("CHF").unwrap();
    assert!(!exposure.global_positive, "the total could not be formed");
    let usd = exposure.rows.iter().find(|r| r.currency == "USD").unwrap();
    assert_eq!(usd.share_pct, None);
    assert_eq!(usd.missing_pair, Some("USD → CHF".to_string()));
    let chf = exposure.rows.iter().find(|r| r.currency == "CHF").unwrap();
    assert_eq!(
        chf.share_pct, None,
        "no share against an absent total — never a partial"
    );
    assert_eq!(
        exposure.share_for("EUR"),
        (None, None),
        "an absent total never yields an honest zero either"
    );
    // The rate arriving makes the next read consolidate.
    state
        .upsert_manual_fx_rate("USD", "0.5", "2026-06-27", "CHF")
        .unwrap();
    let exposure = state.journal_currency_exposure("CHF").unwrap();
    assert!(exposure.global_positive);
    assert_eq!(exposure.share_for("USD").0, Some(Decimal::from(40)));
    assert_eq!(exposure.share_for("CHF").0, Some(Decimal::from(60)));
}

#[test]
fn exposure_excludes_sold_holdings_and_is_none_without_a_journal() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x684);
    state.add_holding("NESN", "6", "100", "CHF").unwrap();
    state.add_holding("GONE", "4", "100", "CHF").unwrap();
    let gone = state
        .list_holdings()
        .iter()
        .find(|h| h.security_ticker == "GONE")
        .unwrap()
        .id;
    state.sell_holding(gone, "", "", "CHF").unwrap();
    let exposure = state.journal_currency_exposure("CHF").unwrap();
    assert_eq!(
        exposure.share_for("CHF").0,
        Some(Decimal::from(100)),
        "position facts — the sold holding carries no exposure (4.6/6.2 semantics)"
    );
    state.journal = None;
    assert!(
        state.journal_currency_exposure("CHF").is_none(),
        "an absence, never an empty-looking zero state"
    );
    assert!(
        state.replacement_candidates("CHF").is_none(),
        "the candidates read refuses too — « indisponible », never « liste vide »"
    );
}

// ── Issue #95 — a study READ FAILURE is « indisponible », never « n'existe pas » ──

/// Make a saved study present-but-unreadable: bump its stored `schema_version` past this build's
/// (the #63 vehicle — `list_studies` reads only indexed columns so the row stays listed, but any
/// payload read fails with `NewerRowSchema`).
fn make_study_unreadable(state: &mut JournalState, id: Uuid) {
    let mut future = state.get_study(id).expect("the study exists");
    future.schema_version = steadyinvest_contract::SCHEMA_VERSION + 1;
    state.journal.as_mut().unwrap().put_study(&future).unwrap();
}

#[test]
fn try_get_study_distinguishes_a_read_failure_from_a_true_absence() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x951);
    let id = state.create_study("NESN", "CHF").unwrap();

    assert_eq!(
        state.try_get_study(Uuid::from_u128(0xDEAD)),
        Ok(None),
        "a missing id is a TRUE absence"
    );
    make_study_unreadable(&mut state, id);
    assert!(
        state.try_get_study(id).is_err(),
        "a present-but-unreadable row is a FAILURE, never Ok(None)"
    );
    assert_eq!(
        state.get_study(id),
        None,
        "the absence-blind wrapper still flattens (its consumers state nothing)"
    );
    assert!(
        state.try_study_id_for_ticker("NESN").is_ok(),
        "the ticker match reads only the listing — still fine"
    );
    assert!(
        state
            .try_matched_study_in_currency("NESN", Some("CHF"))
            .is_err(),
        "the holding auto-match needs the payload — a failed read surfaces as Err"
    );
    assert!(
        state.try_matched_study_in_currency("NESN", None).is_err(),
        "the currency-less match resolves the id then reads the payload — Err too"
    );
}

#[test]
fn an_unreadable_study_is_unclassified_as_unavailable_never_no_study() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x952);
    state.add_holding("NESN", "1", "100", "CHF").unwrap();
    let id = state.create_study("NESN", "CHF").unwrap();
    make_study_unreadable(&mut state, id);

    let (small, medium) = bounds();
    let view = state.journal_diversification("CHF", small, medium);
    assert_eq!(view.unclassified.len(), 1);
    assert!(
        matches!(
            view.unclassified[0].reason,
            UnclassifiedReason::StudyUnavailable
        ),
        "the study EXISTS — « aucune étude » would be factually wrong"
    );
}

#[test]
fn an_unreadable_study_makes_the_candidate_unavailable_never_no_study() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x953);
    state.add_watch_item("NESN", None).unwrap();
    state.add_watch_item("ROG", None).unwrap();
    let id = state.create_study("NESN", "CHF").unwrap();
    make_study_unreadable(&mut state, id);

    let candidates = state.replacement_candidates("CHF").unwrap();
    let nesn = candidates.iter().find(|c| c.ticker == "NESN").unwrap();
    assert_eq!(
        nesn.data,
        CandidateData::StudyUnavailable,
        "the study EXISTS — « aucune étude » would be factually wrong"
    );
    assert_eq!(nesn.study_id, None, "no openable study is offered");
    let rog = candidates.iter().find(|c| c.ticker == "ROG").unwrap();
    assert_eq!(
        rog.data,
        CandidateData::NoStudy,
        "a TRUE absence keeps its honest bucket"
    );
}

#[test]
fn an_unreadable_study_makes_the_confront_unavailable_never_no_closes() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x954);
    let id = state.create_study("NESN", "CHF").unwrap();
    make_study_unreadable(&mut state, id);

    let view = state.confront(id);
    assert!(!view.available);
    assert!(
        view.unavailable,
        "a read failure names itself — never the « pas encore de cours » empty state"
    );
    let absent = state.confront(Uuid::from_u128(0xDEAD));
    assert!(!absent.available);
    assert!(!absent.unavailable, "a true absence stays the empty state");
}

// ── Issue #84 — « Positions vendues » : sold holdings stay reachable; a re-buy re-opens ──

#[test]
fn sold_holdings_lists_retired_positions_and_leaves_the_register_alone() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x841);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    state.add_holding("ROG", "5", "200", "CHF").unwrap();
    let nesn = state.list_holdings()[0].id;
    assert!(state.sold_holdings().is_empty(), "nothing sold yet");

    assert_eq!(
        state.sell_holding(nesn, "", "", "CHF"),
        Ok(MSG_HOLDING_SOLD)
    );
    let sold = state.sold_holdings();
    assert_eq!(sold.len(), 1);
    assert_eq!(sold[0].id, nesn);
    assert_eq!(
        sold[0].sold_at.as_deref().map(|s| &s[..10]),
        Some("2026-06-27"),
        "the sold DAY comes from the injected clock"
    );
    assert_eq!(
        state.list_holdings().len(),
        1,
        "the register keeps only the active position"
    );
    assert!(
        !state.holding_ledger(nesn).is_empty(),
        "the retired ledger stays readable (the #84 surface reads it)"
    );
}

#[test]
fn a_buy_on_a_retired_holding_reopens_the_position_with_a_fresh_wac() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x842);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    assert_eq!(state.sell_holding(id, "", "", "CHF"), Ok(MSG_HOLDING_SOLD));
    assert!(state.list_holdings().is_empty());

    // The re-buy rail (product decision 2026-07-03): a buy on the retired holding re-opens it.
    state
        .record_buy_for(id, "2026-06-28", "5", "120", "0", "retour", "CHF")
        .unwrap();
    let holdings = state.list_holdings();
    assert_eq!(holdings.len(), 1, "the position is back in the register");
    assert_eq!(holdings[0].id, id, "the SAME holding — not a new row");
    assert_eq!(holdings[0].quantity, "5");
    assert_eq!(
        holdings[0].purchase_price, "120",
        "the WAC restarts from the re-buy (Appendix A through a zero position)"
    );
    assert!(
        state.sold_holdings().is_empty(),
        "no longer a sold position"
    );
    // The one ledger keeps the FULL history: opening buy, retiring sell, re-buy.
    let kinds: Vec<Option<String>> = state
        .holding_ledger(id)
        .iter()
        .map(|t| t.kind.clone())
        .collect();
    assert_eq!(
        kinds,
        vec![
            Some("buy".to_string()),
            Some("sell".to_string()),
            Some("buy".to_string())
        ],
        "opening → retiring sell → re-buy, on one ledger"
    );
}

#[test]
fn a_rebuy_is_still_guarded_read_only_and_validated() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x843);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    assert_eq!(state.sell_holding(id, "", "", "CHF"), Ok(MSG_HOLDING_SOLD));

    state.read_only = true;
    assert_eq!(
        state.record_buy_for(id, "", "5", "120", "", "", "CHF"),
        Err(MSG_READ_ONLY_WRITE.to_string())
    );
    state.read_only = false;
    assert_eq!(
        state.record_buy_for(id, "", "0", "120", "", "", "CHF"),
        Err(MSG_HOLDING_INVALID_NUMBER.to_string()),
        "the ledger validations apply to a re-buy unchanged"
    );
    assert!(
        state.list_holdings().is_empty(),
        "a refused re-buy re-opens nothing"
    );
}

// ── Issue #67 — a raw copy of a live journal (non-empty sibling -wal) refuses to restore ──

#[test]
fn request_restore_refuses_an_uncheckpointed_backup_and_parks_nothing() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x670);
    make_backup(&dir, "rawcopy.db", 0xC0FFEE, false);
    // Simulate the hand-rolled raw copy of a LIVE journal: committed frames still in a sibling
    // -wal the `.db` file does not contain.
    std::fs::write(dir.path().join("rawcopy.db-wal"), b"uncheckpointed frames").unwrap();

    assert_eq!(
        state.request_restore(dir.path().join("rawcopy.db").to_str().unwrap()),
        Err(MSG_RESTORE_UNCHECKPOINTED.to_string()),
        "the honest cause is named — never a silent partial restore"
    );
    assert!(!state.has_pending_restore(), "a hard refusal parks nothing");
    assert!(
        state.confirm_restore().is_err(),
        "confirm cannot fire on nothing"
    );
}

#[test]
fn confirm_restore_recheck_catches_a_wal_that_appeared_after_the_assessment() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x671);
    let live_version = state.logical_version_or_zero();
    make_backup(&dir, "src.db", 0xC0FFEE, false);
    state
        .request_restore(dir.path().join("src.db").to_str().unwrap())
        .unwrap();
    // TOCTOU: between the assessment and the confirm, the file becomes a live journal's raw copy.
    std::fs::write(
        dir.path().join("src.db-wal"),
        b"frames written after parking",
    )
    .unwrap();

    assert_eq!(
        state.confirm_restore(),
        Err(MSG_RESTORE_UNCHECKPOINTED.to_string()),
        "the confirm-time re-validation refuses too"
    );
    assert_eq!(
        state.logical_version_or_zero(),
        live_version,
        "the live journal was never touched"
    );
}

// ── Issue #65 — import d'un journal plus ancien : arbitrage de version + agrégats re-dérivés ──

#[test]
fn an_older_same_journal_import_parks_behind_a_confirm_and_applies_nothing() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x650);
    let envelope = state.export_journal().unwrap();
    // Advance the live journal past the envelope's version.
    state.create_study("NESN", "CHF").unwrap();

    let request = state.request_import_journal(&envelope).unwrap();
    let ImportRequest::NeedsConfirm { source, current } = request else {
        panic!("an older same-journal envelope must ask for a confirm, got {request:?}");
    };
    assert!(
        source < current,
        "the regression is stated: {source} < {current}"
    );
    assert_eq!(
        state.list_studies().len(),
        1,
        "nothing was applied while parked"
    );

    state.cancel_import_journal();
    assert!(
        state.confirm_import_journal().is_err(),
        "cancel discarded the parked envelope — confirm has nothing to apply"
    );
}

#[test]
fn a_confirmed_older_import_does_not_resurrect_a_sold_holding() {
    // THE issue scenario: export → sell locally → re-import the older envelope. The upsert alone
    // would blank `sold_at` (a resurrected register row beside a surviving SELL transaction); the
    // arbitration + post-import re-derivation must keep the position retired.
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x651);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let id = state.list_holdings()[0].id;
    let envelope = state.export_journal().unwrap(); // sold_at NULL, quantity 10, no transactions
    assert_eq!(state.sell_holding(id, "", "", "CHF"), Ok(MSG_HOLDING_SOLD));

    let request = state.request_import_journal(&envelope).unwrap();
    assert!(matches!(request, ImportRequest::NeedsConfirm { .. }));
    let summary = state.confirm_import_journal().unwrap();
    assert_eq!(summary.holdings, 1, "the envelope's holding was merged");

    assert!(
        state.list_holdings().is_empty(),
        "the sold position must NOT resurrect into the register"
    );
    let sold = state.sold_holdings();
    assert_eq!(sold.len(), 1, "it stays a sold position");
    assert_eq!(
        sold[0].quantity, "0",
        "the aggregate was re-derived from the surviving ledger, not left at the imported 10"
    );
    let kinds: Vec<Option<String>> = state
        .holding_ledger(id)
        .iter()
        .map(|t| t.kind.clone())
        .collect();
    assert_eq!(
        kinds,
        vec![Some("buy".to_string()), Some("sell".to_string())],
        "the local ledger survived the merge untouched"
    );
}

#[test]
fn a_same_version_or_foreign_envelope_applies_without_a_confirm() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x652);
    state.create_study("NESN", "CHF").unwrap();

    // Same journal, same version — no regression, applied straight away.
    let envelope = state.export_journal().unwrap();
    assert!(matches!(
        state.request_import_journal(&envelope).unwrap(),
        ImportRequest::Applied(_)
    ));

    // A FOREIGN journal's envelope (the FR60 seed case) has no version axis to compare — applied.
    let foreign_dir = TempDir::new().unwrap();
    let mut foreign = watch_state_with_journal_id(&foreign_dir, 0x653, 0xFEED);
    foreign.create_study("ROG", "CHF").unwrap();
    let foreign_envelope = foreign.export_journal().unwrap();
    assert!(matches!(
        state.request_import_journal(&foreign_envelope).unwrap(),
        ImportRequest::Applied(_)
    ));
    assert_eq!(state.list_studies().len(), 2, "the foreign study merged in");
}

#[test]
fn request_import_is_guarded_and_maps_envelope_rejections() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x654);
    let envelope = state.export_journal().unwrap();
    state.read_only = true;
    assert_eq!(
        state.request_import_journal(&envelope).unwrap_err(),
        MSG_READ_ONLY_WRITE.to_string()
    );
    state.read_only = false;
    assert_eq!(
        state.request_import_journal("not json at all").unwrap_err(),
        MSG_IMPORT_MALFORMED.to_string(),
        "the peek refuses exactly what the import would"
    );
}

// ── Issue #34 (FR51, PR 1) — every effective save lands in the durable history ──

#[test]
fn every_effective_save_lands_in_the_durable_history_cross_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let mut state = watch_state(&dir, 0x340);
    let id = state.create_study("NESN", "CHF").unwrap();
    let count = |s: &JournalState| {
        s.journal
            .as_ref()
            .unwrap()
            .list_judgment_snapshots(id)
            .unwrap()
            .len()
    };
    assert_eq!(count(&state), 1, "the creation opens the timeline");

    state
        .edit_cell(id, 4, entry::FIELD_SALES, Some(money("1000")))
        .unwrap();
    assert_eq!(count(&state), 2, "a cell edit appends");
    state
        .edit_cell(id, 4, entry::FIELD_SALES, Some(money("1000")))
        .unwrap();
    assert_eq!(
        count(&state),
        2,
        "a value-identical re-save is deduplicated"
    );

    state.undo(id).unwrap();
    assert_eq!(
        count(&state),
        3,
        "an undo is a real state change — it lands in the history (cadrage decision)"
    );

    // Durable ACROSS reopen — the whole point of FR51 (the 2.9 undo stack resets here).
    drop(state);
    let reopened = open_state(&path);
    assert_eq!(
        reopened
            .journal
            .as_ref()
            .unwrap()
            .list_judgment_snapshots(id)
            .unwrap()
            .len(),
        3,
        "the history survives the reopen"
    );
}
