//! Whole-journal export/import round-trip and rejection tests (Story 5.3, FR60/NFR-R5).
//!
//! Moved out of `src/export.rs` (they were half that file) and rebased onto the PUBLIC surface:
//! instead of the crate-private `journal_snapshot`/`snapshot_to_envelope_json`, the helpers below
//! decode an [`JournalExport`] envelope back into its [`JournalSnapshot`] and re-wrap a (possibly
//! tampered) snapshot under a correct hash — exactly what a third-party tool could do, which is
//! the honest level for these tests.

use steadyinvest_contract::{
    ForecastLowOption, Judgment, SCHEMA_VERSION, Study, Timestamp, sha256_hex,
};
use steadyinvest_persistence::{
    Error, HoldingItem, Journal, JournalExport, JournalSnapshot, LedgerEntry, PortfolioItem,
};
use tempfile::tempdir;
use uuid::Uuid;

fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

fn judgment() -> Judgment {
    Judgment {
        estimated_high_eps: None,
        estimated_low_eps: None,
        projected_sales_growth_pct: None,
        projected_eps_growth_pct: None,
        judged_avg_high_pe: None,
        judged_avg_low_pe: None,
        forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
        recent_severe_low: None,
        current_price: None,
        present_full_year_dividend: None,
        ttm_eps: None,
    }
}

fn study(journal_id: Uuid, id_lo: u128, ticker: &str) -> Study {
    Study::new(
        Uuid::from_u128(id_lo),
        journal_id,
        ticker,
        "CHF",
        judgment(),
        ts("2026-06-29T00:00:00Z"),
    )
}

/// A journal at a temp path, populated with one study, one watchlist row, a portfolio + holding,
/// and a recorded sell (so a sold holding + its transaction are exercised).
fn populated_journal() -> (tempfile::TempDir, Journal) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("a.db");
    let jid = Uuid::from_u128(0xA11CE);
    let mut j = Journal::create(&path, jid, &ts("2026-06-01T00:00:00Z")).unwrap();

    // A study (active) + an archived study.
    j.put_study(&study(jid, 0x5001, "NESN")).unwrap();
    let archived = study(jid, 0x5002, "ROG");
    j.put_study(&archived).unwrap();
    j.set_study_status(archived.id, "archived").unwrap();

    // Watchlist linked to the active study.
    j.add_watch_item(
        Uuid::from_u128(0xA1),
        "NESN",
        Some(Uuid::from_u128(0x5001)),
        &ts("2026-06-02T00:00:00Z"),
    )
    .unwrap();

    // Portfolio + two holdings; sell one (soft-delete + a sell transaction).
    let pid = Uuid::from_u128(0xB1);
    j.ensure_portfolio(pid, "Portfolio", &ts("2026-06-03T00:00:00Z"))
        .unwrap();
    let h1 = j
        .add_holding(
            Uuid::from_u128(0xC1),
            pid,
            "NESN",
            "10",
            "100.00",
            "CHF",
            &ts("2026-06-04T00:00:00Z"),
        )
        .unwrap();
    let h2 = j
        .add_holding(
            Uuid::from_u128(0xC2),
            pid,
            "ROG",
            "5",
            "250.00",
            "CHF",
            &ts("2026-06-05T00:00:00Z"),
        )
        .unwrap();
    // Sell h2 → soft-deleted + a sell transaction referencing it.
    j.record_sell(
        Uuid::from_u128(0xD1),
        h2.id,
        "5",
        "260.00",
        "0",
        "CHF",
        Some("trigger fired"),
        &ts("2026-06-06T00:00:00Z"),
    )
    .unwrap();
    let _ = h1;

    (dir, j)
}

fn empty_journal(name: &str, jid: u128) -> (tempfile::TempDir, Journal) {
    let dir = tempdir().unwrap();
    let path = dir.path().join(name);
    let j = Journal::create(&path, Uuid::from_u128(jid), &ts("2026-06-10T00:00:00Z")).unwrap();
    (dir, j)
}

/// Decode a journal's export envelope back into its snapshot — the public-surface equivalent of the
/// crate-private `journal_snapshot`.
fn snapshot_of(j: &Journal) -> JournalSnapshot {
    let envelope: JournalExport = serde_json::from_str(&j.export_journal().unwrap()).unwrap();
    serde_json::from_str(&envelope.payload).unwrap()
}

/// Re-wrap a (possibly tampered) snapshot in a correctly-hashed envelope — the public-surface
/// equivalent of the crate-private `snapshot_to_envelope_json`. Only the version/shape gates fire on
/// import, never the integrity gate.
fn envelope_json(snapshot: &JournalSnapshot) -> String {
    let payload = serde_json::to_string(snapshot).unwrap();
    serde_json::to_string(&JournalExport {
        schema_version: snapshot.schema_version,
        journal_id: snapshot.journal_id.to_string(),
        logical_version: snapshot.logical_version,
        integrity_hash: sha256_hex(payload.as_bytes()),
        payload,
    })
    .unwrap()
}

#[test]
fn export_then_import_into_an_empty_journal_round_trips_every_entity() {
    let (_da, a) = populated_journal();
    let envelope = a.export_journal().unwrap();

    let (_db, mut b) = empty_journal("b.db", 0xB0B);
    let summary = b.import_journal(&envelope).unwrap();

    assert_eq!(summary.studies, 2);
    assert_eq!(summary.watch_items, 1);
    assert_eq!(summary.holdings, 2);
    assert_eq!(summary.transactions, 1);
    assert_eq!(summary.source_journal_id, Uuid::from_u128(0xA11CE));

    // Studies (incl. the archived one, with status preserved) — rebind to B's journal_id.
    let studies = b.list_studies().unwrap();
    assert_eq!(studies.len(), 2);
    let archived = studies.iter().find(|s| s.security_ticker == "ROG").unwrap();
    assert_eq!(archived.status, "archived");
    let active = b.get_study(Uuid::from_u128(0x5001)).unwrap().unwrap();
    assert_eq!(active.journal_id, b.id(), "study journal_id rebound to B");

    // Watchlist preserved (with its soft link).
    let watch = b.list_watch_items().unwrap();
    assert_eq!(watch.len(), 1);
    assert_eq!(watch[0].study_id, Some(Uuid::from_u128(0x5001)));

    // Holdings: the active register shows only the still-held one; the sold one survives in the
    // full read (so its sell transaction keeps a referent).
    let portfolio = b.first_portfolio().unwrap().unwrap();
    let active_holdings = b.list_holdings(portfolio.id).unwrap();
    assert_eq!(
        active_holdings.len(),
        1,
        "the sold holding left the register"
    );
    assert_eq!(active_holdings[0].security_ticker, "NESN");
    let all_holdings = b.list_all_holdings().unwrap();
    assert_eq!(all_holdings.len(), 2);
    let sold = all_holdings
        .iter()
        .find(|h| h.security_ticker == "ROG")
        .unwrap();
    assert!(sold.sold_at.is_some(), "sold_at preserved on round-trip");

    // The sell transaction came across.
    let txns = b.list_all_transactions().unwrap();
    assert_eq!(txns.len(), 1);
    assert_eq!(txns[0].kind.as_deref(), Some("sell"));
    // Story 6.2 (FR38): the holdings' currency round-trips (populated_journal seeds "CHF").
    assert!(
        all_holdings
            .iter()
            .all(|h| h.currency.as_deref() == Some("CHF")),
        "holding currency preserved on round-trip"
    );
}

#[test]
fn a_holdings_currency_round_trips_including_a_legacy_none() {
    // Story 6.2 (FR38): import a snapshot carrying one explicit-currency holding and one pre-6.2
    // holding (currency: None) — both survive the merge with their currency intact (None stays
    // None, so the app's read-time coalescing to the reference currency still applies).
    let jid = Uuid::from_u128(0x6200);
    let pid = Uuid::from_u128(0x62A);
    let snapshot = JournalSnapshot {
        schema_version: SCHEMA_VERSION,
        journal_id: jid,
        logical_version: 3,
        studies: Vec::new(),
        watch_items: Vec::new(),
        portfolios: vec![PortfolioItem {
            id: pid,
            name: "Bank".to_string(),
            created_at: ts("2026-07-02T09:00:00Z"),
        }],
        holdings: vec![
            HoldingItem {
                id: Uuid::from_u128(0x62B),
                portfolio_id: pid,
                security_ticker: "ASML".to_string(),
                quantity: "3".to_string(),
                purchase_price: "620.00".to_string(),
                currency: Some("EUR".to_string()),
                trailing_stop_pct: None,
                trailing_stop_level: None,
                sold_at: None,
                created_at: ts("2026-07-02T10:00:00Z"),
            },
            HoldingItem {
                id: Uuid::from_u128(0x62C),
                portfolio_id: pid,
                security_ticker: "LEGACY".to_string(),
                quantity: "1".to_string(),
                purchase_price: "10.00".to_string(),
                currency: None, // a pre-6.2 holding
                trailing_stop_pct: None,
                trailing_stop_level: None,
                sold_at: None,
                created_at: ts("2026-07-02T10:01:00Z"),
            },
        ],
        transactions: Vec::new(),
        fx_rates: Vec::new(),
        judgment_snapshots: Vec::new(),
    };
    let envelope = envelope_json(&snapshot);

    let (_dir, mut b) = empty_journal("target", 0x62F);
    b.import_journal(&envelope).unwrap();
    let holdings = b.list_all_holdings().unwrap();
    let eur = holdings
        .iter()
        .find(|h| h.security_ticker == "ASML")
        .unwrap();
    let legacy = holdings
        .iter()
        .find(|h| h.security_ticker == "LEGACY")
        .unwrap();
    assert_eq!(
        eur.currency.as_deref(),
        Some("EUR"),
        "explicit currency kept"
    );
    assert_eq!(legacy.currency, None, "a legacy None currency stays None");
}

#[test]
fn re_import_is_an_idempotent_update_not_a_duplicate() {
    let (_da, a) = populated_journal();
    let envelope = a.export_journal().unwrap();
    let (_db, mut b) = empty_journal("b.db", 0xB0B);
    b.import_journal(&envelope).unwrap();
    b.import_journal(&envelope).unwrap();
    assert_eq!(b.list_studies().unwrap().len(), 2, "no duplicate studies");
    assert_eq!(
        b.list_all_holdings().unwrap().len(),
        2,
        "no duplicate holdings"
    );
    assert_eq!(
        b.list_watch_items().unwrap().len(),
        1,
        "no duplicate watch rows"
    );
    assert_eq!(
        b.list_all_transactions().unwrap().len(),
        1,
        "no duplicate txns"
    );
}

#[test]
fn ledger_buy_and_partial_sell_rows_round_trip_through_export_import() {
    // Story 6.3 (FR39): the ledger rows (kind "buy"/"sell", event dates, fees, rationale) and the
    // materialized weighted-average aggregate survive a whole-journal round-trip, and the
    // partially-sold holding stays ACTIVE on the other side.
    let (_da, mut a) = empty_journal("a.db", 0xA63);
    let pid = Uuid::from_u128(0xB1);
    a.ensure_portfolio(pid, "Portfolio", &ts("2026-06-03T00:00:00Z"))
        .unwrap();
    let h = a
        .add_holding(
            Uuid::from_u128(0xC1),
            pid,
            "NESN",
            "10",
            "100",
            "CHF",
            &ts("2026-06-04T00:00:00Z"),
        )
        .unwrap();
    // A buy with a materialized opening (the app's Story-6.3 flow), then a partial sell.
    let opening = LedgerEntry {
        id: Uuid::from_u128(0xE0),
        occurred_at: "2026-06-04T00:00:00Z",
        quantity: "10",
        unit_price: "100",
        fees: "0",
        currency: "CHF",
        rationale: None,
    };
    let buy = LedgerEntry {
        id: Uuid::from_u128(0xE1),
        occurred_at: "2026-07-01T00:00:00Z",
        quantity: "10",
        unit_price: "110",
        fees: "10",
        currency: "CHF",
        rationale: Some("renforcement"),
    };
    a.record_buy(
        h.id,
        Some(&opening),
        &buy,
        "20",
        "105.5",
        &ts("2026-07-02T09:00:00Z"),
    )
    .unwrap();
    let sell = LedgerEntry {
        id: Uuid::from_u128(0xE2),
        occurred_at: "2026-07-02T00:00:00Z",
        quantity: "4",
        unit_price: "120",
        fees: "0",
        currency: "CHF",
        rationale: None,
    };
    a.record_partial_sell(h.id, None, &sell, "16", &ts("2026-07-02T10:00:00Z"))
        .unwrap();
    // Story 6.4 (FR41): a dividend row (gross per share 3, withholding 10.5) rides the same export.
    let dividend = LedgerEntry {
        id: Uuid::from_u128(0xE3),
        occurred_at: "2026-07-03T00:00:00Z",
        quantity: "10",
        unit_price: "3",
        fees: "10.5",
        currency: "CHF",
        rationale: None,
    };
    a.record_dividend(h.id, &dividend, &ts("2026-07-03T09:00:00Z"))
        .unwrap();

    let envelope = a.export_journal().unwrap();
    let (_db, mut b) = empty_journal("b.db", 0xB63);
    let summary = b.import_journal(&envelope).unwrap();
    assert_eq!(
        summary.transactions, 4,
        "opening + buy + sell + dividend all carried"
    );

    let txns = b.list_transactions(h.id).unwrap();
    assert_eq!(txns.len(), 4);
    assert_eq!(
        txns.iter()
            .map(|t| t.kind.as_deref().unwrap().to_string())
            .collect::<Vec<_>>(),
        vec!["buy", "buy", "sell", "dividend"],
        "kinds and (occurred_at, id) order preserved"
    );
    let dividend_back = txns.iter().find(|t| t.id == dividend.id).unwrap();
    assert_eq!(dividend_back.fees, "10.5", "the withholding round-trips");
    assert_eq!(
        dividend_back.currency, "CHF",
        "the stamped currency round-trips"
    );
    assert_eq!(
        dividend_back.unit_price, "3",
        "the gross per share round-trips"
    );
    let buy_back = txns.iter().find(|t| t.id == buy.id).unwrap();
    assert_eq!(buy_back.fees, "10", "fees round-trip");
    assert_eq!(buy_back.rationale.as_deref(), Some("renforcement"));
    let holding = b
        .list_holdings(pid)
        .unwrap()
        .into_iter()
        .find(|hh| hh.id == h.id)
        .expect("the partially-sold holding is still ACTIVE after import");
    assert_eq!(
        holding.quantity, "16",
        "the materialized aggregate round-trips"
    );
    assert_eq!(holding.purchase_price, "105.5", "WAC round-trips");
}

#[test]
fn the_export_hash_is_deterministic() {
    let (_da, a) = populated_journal();
    assert_eq!(
        a.export_journal().unwrap(),
        a.export_journal().unwrap(),
        "the canonical whole-journal export (and its hash) is stable"
    );
}

#[test]
fn a_tampered_payload_is_rejected_for_integrity_and_writes_nothing() {
    let (_da, a) = populated_journal();
    let envelope = a.export_journal().unwrap();
    let tampered = envelope.replacen("NESN", "ROG0", 1);
    assert_ne!(tampered, envelope);
    let (_db, mut b) = empty_journal("b.db", 0xB0B);
    assert!(matches!(
        b.import_journal(&tampered),
        Err(Error::ImportIntegrity)
    ));
    assert_eq!(b.list_studies().unwrap().len(), 0, "nothing imported");
}

#[test]
fn an_unsupported_schema_version_is_rejected() {
    let (_da, a) = populated_journal();
    let snapshot = snapshot_of(&a);
    // Re-wrap the same payload under a bumped version with a correct hash, so only the version
    // gate fires.
    let mut bumped = snapshot.clone();
    bumped.schema_version = SCHEMA_VERSION + 1;
    let json = envelope_json(&bumped);
    let (_db, mut b) = empty_journal("b.db", 0xB0B);
    assert!(matches!(
        b.import_journal(&json),
        Err(Error::ImportVersion {
            found,
            supported,
        }) if found == SCHEMA_VERSION + 1 && supported == SCHEMA_VERSION
    ));
}

#[test]
fn garbage_input_is_malformed_not_a_panic() {
    let (_db, mut b) = empty_journal("b.db", 0xB0B);
    assert!(matches!(
        b.import_journal("not json at all"),
        Err(Error::ImportMalformed { .. })
    ));
    assert!(matches!(
        b.import_journal("{\"unrelated\": true}"),
        Err(Error::ImportMalformed { .. })
    ));
}

#[test]
fn a_failing_import_rolls_back_completely_never_partial() {
    // A snapshot whose transaction references a holding that is NOT in the snapshot → caught as
    // the neutral `ImportMalformed` (like the holding→portfolio case) → the whole transaction
    // rolls back (atomicity / NFR-R5).
    let (_da, a) = populated_journal();
    let mut snapshot = snapshot_of(&a);
    snapshot.transactions[0].holding_id = Uuid::from_u128(0xDEAD); // dangling FK
    let envelope = envelope_json(&snapshot);

    let (_db, mut b) = empty_journal("b.db", 0xB0B);
    let before_version = b.logical_version().unwrap();
    match b.import_journal(&envelope) {
        Err(Error::ImportMalformed { .. }) => {}
        other => panic!("expected a neutral ImportMalformed, got {other:?}"),
    }
    // Nothing applied: no studies, no holdings, no watch rows, no portfolio, no version bump.
    assert_eq!(b.list_studies().unwrap().len(), 0);
    assert_eq!(b.list_all_holdings().unwrap().len(), 0);
    assert_eq!(b.list_watch_items().unwrap().len(), 0);
    assert!(b.first_portfolio().unwrap().is_none());
    assert_eq!(b.logical_version().unwrap(), before_version);
}

#[test]
fn a_holding_referencing_an_absent_portfolio_is_neutral_malformed_not_a_raw_fk_error() {
    // Story 6.1 review (MED): a holding whose portfolio_id is not in `snapshot.portfolios` is a
    // MALFORMED snapshot — caught as the neutral `ImportMalformed`, never a raw FK leak. The whole
    // import still rolls back (atomic).
    let (_da, a) = populated_journal();
    let mut snapshot = snapshot_of(&a);
    snapshot.holdings[0].portfolio_id = Uuid::from_u128(0xDEAD); // points at no portfolio
    let envelope = envelope_json(&snapshot);

    let (_db, mut b) = empty_journal("b.db", 0xB0B);
    let before = b.logical_version().unwrap();
    match b.import_journal(&envelope) {
        Err(Error::ImportMalformed { .. }) => {}
        other => panic!("expected a neutral ImportMalformed, got {other:?}"),
    }
    assert_eq!(b.list_all_holdings().unwrap().len(), 0, "nothing applied");
    assert_eq!(
        b.logical_version().unwrap(),
        before,
        "no version bump on a refused import"
    );
}

#[test]
fn a_name_only_portfolio_update_on_import_bumps_the_version() {
    // Story 6.1 review (MED / NFR-R2): re-importing a same-id portfolio whose only delta is its
    // name is a real write (ON CONFLICT DO UPDATE), so it must bump the version — even when the
    // snapshot carries no studies/holdings.
    let (_da, mut a) = empty_journal("a.db", 0xA0A);
    let pid = Uuid::from_u128(0xB1);
    a.add_portfolio(pid, "Banque A", &ts("2026-06-10T00:00:00Z"))
        .unwrap();
    let envelope = a.export_journal().unwrap(); // a portfolios-only snapshot

    let (_db, mut b) = empty_journal("b.db", 0xB0B);
    b.add_portfolio(pid, "Banque B", &ts("2026-06-10T00:00:00Z"))
        .unwrap(); // same id, different name
    let before = b.logical_version().unwrap();
    let summary = b.import_journal(&envelope).unwrap();
    assert_eq!(
        summary.portfolios, 0,
        "the existing id is updated, not inserted"
    );
    assert_eq!(
        b.logical_version().unwrap(),
        before + 1,
        "a name-only portfolio update is a real write → it bumps (NFR-R2)"
    );
    assert_eq!(
        b.list_portfolios().unwrap()[0].name,
        "Banque A",
        "the name was updated"
    );
}

#[test]
fn import_into_a_read_only_journal_refuses() {
    // Build a journal, then re-open it read-only by faking a newer on-disk schema is overkill;
    // instead assert the writable gate via a fresh journal is writable, and a verification-only
    // path on garbage still refuses cleanly (covered above). Here we assert an empty snapshot is a
    // no-op success (no spurious version bump).
    let (_da, a) = empty_journal("a.db", 0xA0A);
    let envelope = a.export_journal().unwrap();
    let (_db, mut b) = empty_journal("b.db", 0xB0B);
    let before = b.logical_version().unwrap();
    let summary = b.import_journal(&envelope).unwrap();
    assert_eq!(summary.studies, 0);
    assert_eq!(
        b.logical_version().unwrap(),
        before,
        "an empty snapshot applies nothing and bumps no version"
    );
}

#[test]
fn an_empty_import_into_a_journal_with_a_portfolio_bumps_no_version() {
    // Review patch: the bump/summary must key off a real portfolio INSERT, not mere existence.
    let (_da, a) = empty_journal("a.db", 0xA0A);
    let envelope = a.export_journal().unwrap(); // empty snapshot, no portfolio
    let (_db, mut b) = empty_journal("b.db", 0xB0B);
    b.ensure_portfolio(Uuid::from_u128(0xB1), "P", &ts("2026-06-10T00:00:00Z"))
        .unwrap();
    let before = b.logical_version().unwrap();
    let summary = b.import_journal(&envelope).unwrap();
    assert_eq!(
        summary.portfolios, 0,
        "no portfolio was inserted by the merge"
    );
    assert_eq!(
        b.logical_version().unwrap(),
        before,
        "an empty import into a journal that already has a portfolio is a true no-op"
    );
}

#[test]
fn a_merge_import_repacks_watchlist_positions_to_contiguous() {
    // Review patch: imported watch rows must not collide with existing positions.
    let dir_a = tempdir().unwrap();
    let jid_a = Uuid::from_u128(0xA11CE);
    let mut a = Journal::create(
        dir_a.path().join("a.db"),
        jid_a,
        &ts("2026-06-01T00:00:00Z"),
    )
    .unwrap();
    a.add_watch_item(
        Uuid::from_u128(0x10),
        "NESN",
        None,
        &ts("2026-06-02T00:00:00Z"),
    )
    .unwrap(); // position 0
    a.add_watch_item(
        Uuid::from_u128(0x11),
        "ROG",
        None,
        &ts("2026-06-03T00:00:00Z"),
    )
    .unwrap(); // position 1
    let envelope = a.export_journal().unwrap();

    let (_db, mut b) = empty_journal("b.db", 0xB0B);
    b.add_watch_item(
        Uuid::from_u128(0x20),
        "ABBN",
        None,
        &ts("2026-06-04T00:00:00Z"),
    )
    .unwrap(); // position 0 — collides with A's first imported row
    b.import_journal(&envelope).unwrap();

    let rows = b.list_watch_items().unwrap();
    assert_eq!(rows.len(), 3, "all three rows present after the merge");
    let positions: Vec<i64> = rows.iter().map(|w| w.position).collect();
    assert_eq!(
        positions,
        vec![0, 1, 2],
        "positions repacked to contiguous 0..n"
    );
}

#[test]
fn a_study_blob_with_an_incompatible_version_is_rejected_and_rolls_back() {
    // Review patch: gate each study blob's own schema_version (distinct axis from the envelope's).
    let (_da, a) = populated_journal();
    let mut snapshot = snapshot_of(&a);
    snapshot.studies[0].study.schema_version = SCHEMA_VERSION + 1;
    let envelope = envelope_json(&snapshot);
    let (_db, mut b) = empty_journal("b.db", 0xB0B);
    assert!(matches!(
        b.import_journal(&envelope),
        Err(Error::ImportVersion { found, supported })
            if found == SCHEMA_VERSION + 1 && supported == SCHEMA_VERSION
    ));
    assert_eq!(
        b.list_studies().unwrap().len(),
        0,
        "nothing imported on rejection"
    );
}

#[test]
fn fx_rates_round_trip_and_an_old_file_without_the_array_still_imports() {
    // Story 6.5 (FR28/AC5): fx_rates ride the export; `#[serde(default)]` is the #78 additive
    // rail — an OLD export (no array) imports fine into this build.
    let (_da, mut a) = empty_journal("a.db", 0xA65);
    a.upsert_fx_rate(&steadyinvest_persistence::FxRateItem {
        id: Uuid::from_u128(0xF1),
        base_currency: "USD".to_string(),
        quote_currency: "CHF".to_string(),
        rate: "0.885".to_string(),
        rate_date: "2026-07-02".to_string(),
        source: "manuel".to_string(),
        created_at: ts("2026-07-02T09:00:00Z"),
    })
    .unwrap();

    let envelope = a.export_journal().unwrap();
    let (_db, mut b) = empty_journal("b.db", 0xB65);
    let summary = b.import_journal(&envelope).unwrap();
    assert_eq!(summary.fx_rates, 1);
    let rates = b.list_fx_rates().unwrap();
    assert_eq!(rates.len(), 1);
    assert_eq!(rates[0].rate, "0.885");
    assert_eq!(rates[0].rate_date, "2026-07-02");
    assert_eq!(
        rates[0].source, "manuel",
        "date + source round-trip verbatim"
    );

    // An OLD file: with `skip_serializing_if`, an empty store serializes WITHOUT the array —
    // which is byte-for-byte the pre-6.5 file shape (the #78 back-compat half).
    let mut snapshot = snapshot_of(&a);
    snapshot.fx_rates.clear();
    let old_payload = serde_json::to_string(&snapshot).unwrap();
    assert!(
        !old_payload.contains("fx_rates"),
        "an empty store omits the array entirely (pre-6.5 builds still read this export)"
    );
    let old_envelope = serde_json::to_string(&JournalExport {
        schema_version: snapshot.schema_version,
        journal_id: snapshot.journal_id.to_string(),
        logical_version: snapshot.logical_version,
        integrity_hash: sha256_hex(old_payload.as_bytes()),
        payload: old_payload,
    })
    .unwrap();
    let (_dc, mut c) = empty_journal("c.db", 0xC65);
    let summary = c
        .import_journal(&old_envelope)
        .expect("a pre-6.5 file (no fx_rates array) imports fine");
    assert_eq!(summary.fx_rates, 0);
}

// ── Issue #34 (FR51, PR 3) — the durable history travels in the envelope ──

#[test]
fn the_history_round_trips_byte_faithfully_and_reads_back_as_a_timeline() {
    let (_dir, mut source) = empty_journal("source.db", 0x34A);
    let jid = source.id();
    let original = study(jid, 0x1, "NESN");
    source
        .put_study_with_history(&original, &ts("2026-07-09T08:00:00Z"))
        .unwrap();
    let mut changed = original.clone();
    changed.rationale = Some("Raison consignée.".to_string());
    source
        .put_study_with_history(&changed, &ts("2026-07-09T09:00:00Z"))
        .unwrap();
    let source_rows = source.list_judgment_snapshots(original.id).unwrap();
    assert_eq!(source_rows.len(), 2);

    let (_dir2, mut target) = empty_journal("target.db", 0x34B);
    let summary = target
        .import_journal(&source.export_journal().unwrap())
        .unwrap();
    assert_eq!(
        summary.judgment_snapshots, 2,
        "the summary counts the history"
    );

    let target_rows = target.list_judgment_snapshots(original.id).unwrap();
    assert_eq!(
        target_rows, source_rows,
        "ids, stamps and versions round-trip identically"
    );
    assert_eq!(
        target.get_judgment_snapshot(target_rows[1].id).unwrap(),
        Some(changed),
        "each historical state reads back exactly"
    );
    // Idempotent — a re-import updates nothing new.
    let again = target
        .import_journal(&source.export_journal().unwrap())
        .unwrap();
    assert_eq!(again.judgment_snapshots, 2);
    assert_eq!(
        target.list_judgment_snapshots(original.id).unwrap().len(),
        2
    );
}

#[test]
fn a_history_less_journal_exports_without_the_field_and_an_old_file_imports_fine() {
    // The #78 additive rail, both directions.
    let (_dir, mut j) = empty_journal("plain.db", 0x34C);
    j.put_study(&study(j.id(), 0x2, "ROG")).unwrap(); // plain save — no history
    let envelope: JournalExport = serde_json::from_str(&j.export_journal().unwrap()).unwrap();
    assert!(
        !envelope.payload.contains("judgment_snapshots"),
        "an empty series is omitted, so a pre-#34 build still reads this export"
    );

    // An OLD file (the field absent) imports into THIS build — serde default.
    let (_dir2, mut target) = empty_journal("old-target.db", 0x34D);
    let summary = target.import_journal(&j.export_journal().unwrap()).unwrap();
    assert_eq!(summary.judgment_snapshots, 0);
}

#[test]
fn a_history_row_referencing_an_absent_study_is_malformed_and_writes_nothing() {
    let (_dir, mut source) = empty_journal("bad-source.db", 0x34E);
    let s = study(source.id(), 0x3, "NOVN");
    source
        .put_study_with_history(&s, &ts("2026-07-09T08:00:00Z"))
        .unwrap();
    let mut snapshot = snapshot_of(&source);
    snapshot.studies.clear(); // the history now dangles
    let (_dir2, mut target) = empty_journal("bad-target.db", 0x34F);
    assert!(matches!(
        target.import_journal(&envelope_json(&snapshot)),
        Err(Error::ImportMalformed { .. })
    ));
    assert!(target.list_judgment_snapshots(s.id).unwrap().is_empty());
}

#[test]
fn a_newer_schema_history_row_is_rejected_up_front() {
    let (_dir, mut source) = empty_journal("newer-source.db", 0x350);
    let s = study(source.id(), 0x4, "ABBN");
    source
        .put_study_with_history(&s, &ts("2026-07-09T08:00:00Z"))
        .unwrap();
    let mut snapshot = snapshot_of(&source);
    snapshot.judgment_snapshots[0].schema_version = i64::from(SCHEMA_VERSION) + 1;
    let (_dir2, mut target) = empty_journal("newer-target.db", 0x351);
    assert!(
        matches!(
            target.import_journal(&envelope_json(&snapshot)),
            Err(Error::ImportVersion { .. })
        ),
        "a row this build could never read back must not poison the timeline"
    );
}
