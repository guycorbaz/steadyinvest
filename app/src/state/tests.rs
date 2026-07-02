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
    use steadyinvest_core::normalize::{normalize, RawAmount, RawFinancials, RawYear};
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
        .apply_holding_price(id, Decimal::from_str_exact("104.50").unwrap())
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
        .apply_holding_price(id, Decimal::from_str_exact("104.50").unwrap())
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

/// A divergent refresh of a **validated** provider cell auto-demotes `✓ → ?` (FR20, AC3); a
/// non-divergent re-fetch keeps the human `✓`.
#[test]
fn refresh_demotes_a_divergent_validated_provider_cell() {
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

    // A divergent re-fetch (100 → 250) auto-demotes the ✓ to ?.
    state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 250, 50, "beadfeed"))
        .unwrap();
    let high = &state.get_study(id).unwrap().years[0].high_price;
    assert_eq!(
        high.review,
        Review::ToReview,
        "a divergent provider value auto-tags ✓ → ?"
    );
    assert_eq!(high.value, Some(und_money(250)));
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

/// End-to-end (the Story-3.3 invariant 2b through a REAL refresh, not a hand-set freshness):
/// a fully-validated provider study reads `Full`; a divergent refresh of a load-bearing provider
/// cell auto-demotes it and the verdict degrades to `Provisional` in the same frame. (AC3,
/// complements `seam_check.rs` SEAM 3 which sets the flag by hand.)
#[test]
fn a_divergent_refresh_degrades_a_full_verdict_to_provisional() {
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

    // A divergent refresh of the (validated, provider) high_price demotes ✓ → ? …
    state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 250, 50, "deg"))
        .unwrap();
    let study = state.get_study(id).unwrap();
    assert_eq!(
        study.years[0].high_price.review,
        Review::ToReview,
        "the divergent provider value auto-demotes the ✓"
    );
    assert!(
        matches!(
            engine::build_snapshot(&study)
                .expect("normalizes")
                .verdict(),
            Verdict::Provisional(_)
        ),
        "a demoted load-bearing input degrades Full → Provisional in the same frame"
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
/// preserved alongside (pending), and the `✓` demotes to `?` — never merged. (AC1, AC2, AC3)
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

    let high = &state.get_study(id).unwrap().years[0].high_price;
    assert_eq!(high.value, Some(und_money(999)), "manual value stands");
    assert_eq!(high.source, Source::Manual);
    assert_eq!(high.review, Review::ToReview, "the ✓ demotes on divergence");
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
    assert_eq!(high.review, Review::ToReview);
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

#[test]
fn revalidate_counts_only_demoted_validated_cells() {
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
    // Refresh: high_price 100 → 200 diverges (demotes the 5 validated ✓); eps/sales/low unchanged.
    let report = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 200, 50, "annual"))
        .unwrap();
    assert_eq!(
        report.revalidate, 5,
        "the 5 validated high_price cells that diverged are the re-validation scope"
    );
    // A second identical refresh demotes nothing (already ? + value agrees) → revalidate 0.
    let again = state
        .apply_provider_refresh(id, &fetched_custom(&years, 1000, 5, 200, 50, "annual"))
        .unwrap();
    assert_eq!(again.revalidate, 0, "an agreeing re-fetch demotes nothing");
}

#[test]
fn refresh_summary_appends_the_revalidate_clause_only_when_needed() {
    let no_demote = RefreshReport {
        updated: 1,
        cause: crate::viewmodel::refresh::RefreshCause {
            price: true,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        refresh_summary(no_demote),
        refresh_notice(no_demote),
        "with no demotions the summary is exactly the cause notice (no regression)"
    );
    let with_demote = RefreshReport {
        revalidate: 3,
        ..no_demote
    };
    let summary = refresh_summary(with_demote);
    assert!(summary.starts_with(refresh_notice(with_demote)));
    assert!(
        summary.contains("3 cellule(s) à revérifier"),
        "the re-validation scope is named: {summary}"
    );
}

/// The Journey-2b ritual end-to-end through the real rails: reopen a saved validated study, re-fetch
/// new annual data, and confirm manual + judgment preserved, changed ✓ → ?, unchanged ✓ kept, the
/// re-validation count correct, and the projection extends. (AC1, AC2, AC3, AC4)
#[test]
fn the_annual_update_journey_preserves_manual_and_judgment_and_demotes_only_what_moved() {
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
    // AC2 — a changed validated provider cell is now ?; an unchanged one keeps ✓.
    assert_eq!(y0.high_price.review, Review::ToReview, "changed high ✓ → ?");
    assert_eq!(y0.high_price.value, Some(und_money(200)));
    assert_eq!(y0.eps.review, Review::Validated, "unchanged eps keeps ✓");
    assert_eq!(
        y0.sales.review,
        Review::ToReview,
        "diverged manual sales → ?"
    );
    // AC3 — the re-validation scope: 5 high_price + the 1 manual sales = 6.
    assert_eq!(report.revalidate, 6, "only what moved needs re-validation");
    assert!(refresh_summary(report).contains("6 cellule(s) à revérifier"));

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
        report.revalidate, 0,
        "nothing was ✓ after unlock → no demotions to re-validate"
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
        .apply_holding_price(id, rust_decimal::Decimal::new(70, 0))
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
        .sell_holding(ids[0], "  stop touché  ", "CHF")
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

    state.sell_holding(id, "", "CHF").expect("the sell records");

    let transactions = state
        .journal
        .as_ref()
        .unwrap()
        .list_all_transactions()
        .unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(
        transactions[0].currency, "USD",
        "the SELL row carries the holding's own currency, not the CHF reference"
    );
}

#[test]
fn sell_holding_refuses_an_absent_id() {
    let dir = TempDir::new().unwrap();
    let mut state = watch_state(&dir, 0x471);
    state.add_holding("NESN", "10", "100", "CHF").unwrap();
    let ghost = Uuid::from_u128(0xDEAD);
    assert!(
        state.sell_holding(ghost, "", "CHF").is_err(),
        "selling a non-existent holding is refused, nothing written"
    );
    assert_eq!(state.list_holdings().len(), 1, "the register is untouched");
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
    assert!(
        state.list_holdings().is_empty(),
        "no invalid input wrote a row"
    );
    // A free purchase (price 0) is allowed (e.g. a gift/spin-off).
    state.add_holding("FREE", "1", "0", "CHF").unwrap();
    assert_eq!(state.list_holdings().len(), 1);
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
    assert!(state.get_study(id).unwrap().years[0]
        .high_price
        .value
        .is_some());

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
    assert!(state.get_study(id).unwrap().years[0]
        .high_price
        .value
        .is_some());

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
    assert!(state
        .get_study(id)
        .unwrap()
        .judgment
        .estimated_high_eps
        .is_some());
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
        vec![2021, 2022, 2023, 2024, 2025, 2026, 2027],
        "each call appends the next year (oldest→newest, horizon re-bases off the new latest)"
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
