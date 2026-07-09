//! Integration tests — the transaction ledger: Story 4.7 recorded-sell (FR46/FR47) and the
//! Story 6.3 compound writers (buys, partial sells, edit/delete — FR39) on the v4 columns.

use steadyinvest_contract::Timestamp;
use steadyinvest_persistence::{Journal, LedgerEntry};
use tempfile::TempDir;
use uuid::Uuid;

fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

fn fresh(dir: &TempDir) -> Journal {
    let path = dir.path().join("journal.db");
    let jid = Uuid::from_u128(0xC0FFEE);
    Journal::create(&path, jid, &ts("2026-06-29T00:00:00Z")).expect("a fresh journal creates")
}

fn seed_holding(journal: &mut Journal) -> Uuid {
    let pid = Uuid::from_u128(0x9001);
    journal
        .ensure_portfolio(pid, "Portefeuille", &ts("2026-06-29T09:00:00Z"))
        .expect("ensure portfolio");
    let hid = Uuid::from_u128(0x9101);
    journal
        .add_holding(
            hid,
            pid,
            "NESN",
            "10",
            "100",
            "CHF",
            None,
            &ts("2026-06-29T10:00:00Z"),
        )
        .expect("add holding");
    hid
}

#[test]
fn record_sell_writes_a_sell_row_with_its_rationale() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);

    journal
        .record_sell(
            Uuid::from_u128(0x9201),
            hid,
            "10",
            "85.50",
            "0",
            "CHF",
            Some("stop touché, je sécurise"),
            &ts("2026-06-29T11:00:00Z"),
        )
        .expect("the sell records");

    let rows = journal.list_transactions(hid).expect("list reads");
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.kind.as_deref(), Some("sell"));
    assert_eq!(row.quantity, "10");
    assert_eq!(
        row.unit_price, "85.50",
        "decimals round-trip exactly (TEXT)"
    );
    assert_eq!(row.fees, "0");
    assert_eq!(row.currency, "CHF");
    assert_eq!(row.rationale.as_deref(), Some("stop touché, je sécurise"));

    // The same `record_sell` transaction also retired the holding (atomic soft-delete) — it leaves
    // the active register but stays a live FK referent for the sell row above.
    assert!(
        journal
            .list_holdings(Uuid::from_u128(0x9001))
            .expect("list holdings")
            .is_empty(),
        "the sold holding no longer appears in the active register"
    );
}

#[test]
fn a_blank_rationale_persists_as_null() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);

    journal
        .record_sell(
            Uuid::from_u128(0x9202),
            hid,
            "10",
            "85",
            "0",
            "CHF",
            None,
            &ts("2026-06-29T11:00:00Z"),
        )
        .expect("the sell records");

    assert_eq!(
        journal.list_transactions(hid).expect("list reads")[0].rationale,
        None,
        "no rationale → NULL, not an empty string"
    );
}

// ── Story 6.3 — the FR39 ledger writers (buys, partial sells, edit/delete). Persistence performs
// no arithmetic: the aggregates below are the caller-computed values a real app derives via
// `core::risk::ledger`; the tests only assert they land atomically with the ledger row. ──

fn entry<'a>(
    id: u128,
    occurred_at: &'a str,
    quantity: &'a str,
    unit_price: &'a str,
    fees: &'a str,
) -> LedgerEntry<'a> {
    LedgerEntry {
        id: Uuid::from_u128(id),
        occurred_at,
        quantity,
        unit_price,
        fees,
        currency: "CHF",
        rationale: None,
    }
}

fn version(journal: &Journal) -> u64 {
    journal.logical_version().expect("logical_version reads")
}

/// The seeded holding's row (incl. a sold one), read back for aggregate assertions.
fn holding_row(journal: &Journal, hid: Uuid) -> steadyinvest_persistence::HoldingItem {
    journal
        .list_all_holdings()
        .expect("list_all_holdings reads")
        .into_iter()
        .find(|h| h.id == hid)
        .expect("the holding row exists")
}

#[test]
fn record_buy_inserts_the_row_and_lands_the_aggregate_atomically() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);
    let before = version(&journal);

    journal
        .record_buy(
            hid,
            None,
            &entry(0xB001, "2026-07-01T00:00:00Z", "5", "120", "9.95"),
            "15",
            "106.66",
            &ts("2026-07-02T08:00:00Z"),
        )
        .expect("the buy records");

    let rows = journal.list_transactions(hid).expect("list reads");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind.as_deref(), Some("buy"));
    assert_eq!(
        rows[0].occurred_at.0, "2026-07-01T00:00:00Z",
        "occurred_at is the caller's event date, not the clock stamp"
    );
    assert_eq!(rows[0].created_at.0, "2026-07-02T08:00:00Z");
    assert_eq!(rows[0].fees, "9.95", "decimals round-trip exactly (TEXT)");

    let holding = holding_row(&journal, hid);
    assert_eq!(holding.quantity, "15", "the aggregate landed with the row");
    assert_eq!(holding.purchase_price, "106.66");
    assert_eq!(version(&journal), before + 1, "exactly one version bump");
}

#[test]
fn record_buy_with_an_opening_materializes_both_rows_in_one_call() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);
    let before = version(&journal);

    journal
        .record_buy(
            hid,
            // The AC5 opening position: the pre-6.3 holding's own values, dated its created_at.
            Some(&entry(0xB010, "2026-06-29T10:00:00Z", "10", "100", "0")),
            &entry(0xB011, "2026-07-01T00:00:00Z", "5", "120", "0"),
            "15",
            "106.67",
            &ts("2026-07-02T08:00:00Z"),
        )
        .expect("the buy records with its opening");

    let rows = journal.list_transactions(hid).expect("list reads");
    assert_eq!(rows.len(), 2, "opening row + buy row in one call");
    assert_eq!(
        rows[0].id,
        Uuid::from_u128(0xB010),
        "the opening comes first (oldest occurred_at)"
    );
    assert_eq!(rows[0].kind.as_deref(), Some("buy"));
    assert_eq!(rows[0].quantity, "10");
    assert_eq!(rows[1].id, Uuid::from_u128(0xB011));
    assert_eq!(
        version(&journal),
        before + 1,
        "one bump for the whole compound write"
    );
}

#[test]
fn record_partial_sell_keeps_the_holding_active_with_the_reduced_quantity() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);
    let before = version(&journal);

    journal
        .record_partial_sell(
            hid,
            None,
            &entry(0xC001, "2026-07-01T00:00:00Z", "4", "130", "0"),
            "6",
            &ts("2026-07-02T08:00:00Z"),
        )
        .expect("the partial sell records");

    let rows = journal.list_transactions(hid).expect("list reads");
    assert_eq!(rows[0].kind.as_deref(), Some("sell"));

    let holding = holding_row(&journal, hid);
    assert_eq!(holding.quantity, "6");
    assert_eq!(
        holding.sold_at, None,
        "a partial sell does not retire the holding"
    );
    assert_eq!(
        holding.purchase_price, "100",
        "a sell never re-averages the cost basis"
    );
    assert_eq!(
        journal
            .list_holdings(Uuid::from_u128(0x9001))
            .expect("list holdings")
            .len(),
        1,
        "the holding stays in the active register"
    );
    assert_eq!(version(&journal), before + 1);
}

#[test]
fn record_partial_sell_to_zero_retires_the_holding_like_4_7() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);

    journal
        .record_partial_sell(
            hid,
            None,
            &entry(0xC002, "2026-07-01T00:00:00Z", "10", "130", "0"),
            "0",
            &ts("2026-07-02T08:00:00Z"),
        )
        .expect("the emptying sell records");

    let holding = holding_row(&journal, hid);
    assert_eq!(holding.quantity, "0", "the aggregate stays truthful");
    assert_eq!(
        holding.sold_at.as_deref(),
        Some("2026-07-02T08:00:00Z"),
        "an emptied position stamps sold_at (the 4.7 retire semantics)"
    );
    assert!(
        journal
            .list_holdings(Uuid::from_u128(0x9001))
            .expect("list holdings")
            .is_empty(),
        "the retired holding leaves the active register"
    );
}

#[test]
fn update_transaction_with_identical_values_is_a_true_no_op() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);
    journal
        .record_buy(
            hid,
            None,
            &entry(0xD001, "2026-07-01T00:00:00Z", "5", "120", "0"),
            "15",
            "106.67",
            &ts("2026-07-02T08:00:00Z"),
        )
        .expect("the buy records");
    let before = version(&journal);

    let applied = journal
        .update_transaction(
            Uuid::from_u128(0xD001),
            hid,
            None,
            "2026-07-01T00:00:00Z",
            "5",
            "120",
            "0",
            None,
            "15",
            "106.67",
            None,
            &ts("2026-07-02T09:00:00Z"),
        )
        .expect("the identical edit succeeds");

    assert!(applied, "the row exists → Ok(true)");
    assert_eq!(
        version(&journal),
        before,
        "identical values → no write, NO bump"
    );
}

#[test]
fn update_transaction_applies_the_edit_and_the_aggregate_with_one_bump() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);
    journal
        .record_buy(
            hid,
            None,
            &entry(0xD002, "2026-07-01T00:00:00Z", "5", "120", "0"),
            "15",
            "106.67",
            &ts("2026-07-02T08:00:00Z"),
        )
        .expect("the buy records");
    let before = version(&journal);

    let applied = journal
        .update_transaction(
            Uuid::from_u128(0xD002),
            hid,
            None,
            "2026-07-01T00:00:00Z",
            "8",
            "120",
            "12.50",
            Some("corrigé : 8 titres"),
            "18",
            "109.58",
            None,
            &ts("2026-07-02T09:00:00Z"),
        )
        .expect("the edit applies");

    assert!(applied);
    let row = &journal.list_transactions(hid).expect("list reads")[0];
    assert_eq!(row.quantity, "8");
    assert_eq!(row.fees, "12.50");
    assert_eq!(row.rationale.as_deref(), Some("corrigé : 8 titres"));
    assert_eq!(
        row.kind.as_deref(),
        Some("buy"),
        "kind is the row's identity — never editable"
    );
    assert_eq!(
        row.currency, "CHF",
        "currency is pinned to the holding's (FR28)"
    );

    let holding = holding_row(&journal, hid);
    assert_eq!(
        holding.quantity, "18",
        "the recomputed aggregate landed atomically"
    );
    assert_eq!(holding.purchase_price, "109.58");
    assert_eq!(
        version(&journal),
        before + 1,
        "exactly one bump for the compound edit"
    );
}

#[test]
fn update_transaction_on_an_absent_id_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);
    let before = version(&journal);

    let applied = journal
        .update_transaction(
            Uuid::from_u128(0xDEAD),
            hid,
            // Even with an opening passed, an absent id must not materialize it.
            Some(&entry(0xD010, "2026-06-29T10:00:00Z", "10", "100", "0")),
            "2026-07-01T00:00:00Z",
            "5",
            "120",
            "0",
            None,
            "15",
            "106.67",
            None,
            &ts("2026-07-02T09:00:00Z"),
        )
        .expect("the absent edit is a no-op success");

    assert!(!applied, "absent id → Ok(false)");
    assert!(
        journal
            .list_transactions(hid)
            .expect("list reads")
            .is_empty()
    );
    assert_eq!(
        holding_row(&journal, hid).quantity,
        "10",
        "the aggregate is untouched"
    );
    assert_eq!(version(&journal), before, "no bump");
}

#[test]
fn update_transaction_refuses_a_row_belonging_to_another_holding() {
    // 2026-07-02 review (HIGH): a txn id paired with the WRONG holding must be the typed no-op —
    // never "edit holding A's row while rewriting holding B's aggregate".
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);
    // A second holding with its own ledger row.
    let other = Uuid::from_u128(0x9102);
    journal
        .add_holding(
            other,
            Uuid::from_u128(0x9001),
            "ROG",
            "5",
            "250",
            "CHF",
            None,
            &ts("2026-06-29T09:30:00Z"),
        )
        .expect("second holding");
    let row = journal
        .record_buy(
            other,
            None,
            &entry(0xE100, "2026-06-30T00:00:00Z", "5", "250", "0"),
            "10",
            "250",
            &ts("2026-06-30T09:00:00Z"),
        )
        .expect("the other holding's buy");
    let before = version(&journal);

    // Edit `other`'s row while claiming it belongs to `hid` → Ok(false), NOTHING written.
    let applied = journal
        .update_transaction(
            row.id,
            hid,
            None,
            "2026-07-01T00:00:00Z",
            "5",
            "999",
            "0",
            None,
            "0",
            "0",
            None,
            &ts("2026-07-02T09:00:00Z"),
        )
        .expect("the mismatched pair is a typed no-op");
    assert!(!applied, "ownership mismatch → Ok(false)");
    assert_eq!(
        journal.list_transactions(other).unwrap()[0].unit_price,
        "250",
        "the row is untouched"
    );
    assert_eq!(
        holding_row(&journal, hid).quantity,
        "10",
        "the claimed holding's aggregate is untouched"
    );
    assert_eq!(version(&journal), before, "no bump");

    // Same guard on delete.
    let deleted = journal
        .delete_transaction(
            row.id,
            hid,
            None,
            "0",
            "0",
            None,
            &ts("2026-07-02T09:05:00Z"),
        )
        .expect("the mismatched delete is a typed no-op");
    assert!(!deleted, "ownership mismatch → Ok(false)");
    assert_eq!(journal.list_transactions(other).unwrap().len(), 1);
    assert_eq!(version(&journal), before, "still no bump");
}

#[test]
fn delete_transaction_rewrites_the_aggregate_with_one_bump() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);
    journal
        .record_buy(
            hid,
            None,
            &entry(0xE001, "2026-07-01T00:00:00Z", "5", "120", "0"),
            "15",
            "106.67",
            &ts("2026-07-02T08:00:00Z"),
        )
        .expect("the buy records");
    let before = version(&journal);

    let applied = journal
        .delete_transaction(
            Uuid::from_u128(0xE001),
            hid,
            None,
            "10",
            "100",
            None,
            &ts("2026-07-02T09:00:00Z"),
        )
        .expect("the delete applies");

    assert!(applied);
    assert!(
        journal
            .list_transactions(hid)
            .expect("list reads")
            .is_empty()
    );
    let holding = holding_row(&journal, hid);
    assert_eq!(
        holding.quantity, "10",
        "the restored aggregate landed atomically"
    );
    assert_eq!(holding.purchase_price, "100");
    assert_eq!(version(&journal), before + 1, "exactly one bump");
}

#[test]
fn deleting_the_retiring_sell_un_retires_the_holding() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);
    journal
        .record_partial_sell(
            hid,
            None,
            &entry(0xE002, "2026-07-01T00:00:00Z", "10", "130", "0"),
            "0",
            &ts("2026-07-02T08:00:00Z"),
        )
        .expect("the emptying sell records");
    assert!(
        holding_row(&journal, hid).sold_at.is_some(),
        "retired first"
    );

    journal
        .delete_transaction(
            Uuid::from_u128(0xE002),
            hid,
            None,
            "10",
            "100",
            None,
            &ts("2026-07-02T09:00:00Z"),
        )
        .expect("the delete applies");

    let holding = holding_row(&journal, hid);
    assert_eq!(
        holding.sold_at, None,
        "sold_at cleared — the holding un-retires"
    );
    assert_eq!(
        holding.quantity, "10",
        "the restored quantity landed with it"
    );
    assert_eq!(
        journal
            .list_holdings(Uuid::from_u128(0x9001))
            .expect("list holdings")
            .len(),
        1,
        "the holding is back in the active register"
    );
}

#[test]
fn delete_transaction_on_an_absent_id_writes_nothing_even_with_an_opening() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);
    let before = version(&journal);

    let applied = journal
        .delete_transaction(
            Uuid::from_u128(0xDEAD),
            hid,
            Some(&entry(0xE010, "2026-06-29T10:00:00Z", "10", "100", "0")),
            "10",
            "100",
            None,
            &ts("2026-07-02T09:00:00Z"),
        )
        .expect("the absent delete is a no-op success");

    assert!(!applied, "absent id → Ok(false)");
    assert!(
        journal
            .list_transactions(hid)
            .expect("list reads")
            .is_empty(),
        "the opening was NOT materialized"
    );
    assert_eq!(version(&journal), before, "nothing written, no bump");
}

#[test]
fn a_failing_fk_on_record_buy_applies_nothing() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    seed_holding(&mut journal);
    let before = version(&journal);

    let result = journal.record_buy(
        Uuid::from_u128(0xBAD0), // no such holding
        None,
        &entry(0xB0AD, "2026-07-01T00:00:00Z", "5", "120", "0"),
        "5",
        "120",
        &ts("2026-07-02T08:00:00Z"),
    );

    assert!(result.is_err(), "the FK violation surfaces as an error");
    assert!(
        journal
            .list_all_transactions()
            .expect("list reads")
            .is_empty(),
        "no partial row escaped the transaction"
    );
    assert_eq!(version(&journal), before, "no bump");
}

#[test]
fn record_dividend_inserts_one_row_one_bump_and_touches_nothing_else() {
    // Story 6.4 (FR41): a dividend is a CASH row — the aggregate (quantity/purchase_price) and the
    // retired state (sold_at) stay exactly as the buy/sell replay left them.
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);
    let before = version(&journal);

    let row = journal
        .record_dividend(
            hid,
            &entry(0xD1F, "2026-07-01T00:00:00Z", "10", "3", "10.5"),
            &ts("2026-07-02T09:00:00Z"),
        )
        .expect("the dividend records");
    assert_eq!(row.kind.as_deref(), Some("dividend"));

    let rows = journal.list_transactions(hid).expect("list reads");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind.as_deref(), Some("dividend"));
    assert_eq!(rows[0].quantity, "10", "shares paid on");
    assert_eq!(rows[0].unit_price, "3", "gross per share");
    assert_eq!(rows[0].fees, "10.5", "withholding retained");
    let holding = holding_row(&journal, hid);
    assert_eq!(holding.quantity, "10", "position untouched");
    assert_eq!(holding.purchase_price, "100", "basis untouched");
    assert!(holding.sold_at.is_none(), "retired state untouched");
    assert_eq!(version(&journal), before + 1, "exactly one bump");
}

#[test]
fn a_dividend_row_edits_and_deletes_through_the_generic_writers() {
    // The 6.3 update/delete writers are kind-agnostic: the kind survives an edit (not editable),
    // and each applied mutation bumps once.
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let hid = seed_holding(&mut journal);
    let row = journal
        .record_dividend(
            hid,
            &entry(0xD2F, "2026-07-01T00:00:00Z", "10", "3", "10.5"),
            &ts("2026-07-02T09:00:00Z"),
        )
        .unwrap();
    let before = version(&journal);

    // Edit the withholding (the aggregate passed back is the unchanged one — no-op on holdings).
    let applied = journal
        .update_transaction(
            row.id,
            hid,
            None,
            "2026-07-01T00:00:00Z",
            "10",
            "3",
            "0",
            Some("brut, pas de retenue"),
            "10",
            "100",
            None,
            &ts("2026-07-02T10:00:00Z"),
        )
        .expect("the dividend edit applies");
    assert!(applied);
    let rows = journal.list_transactions(hid).unwrap();
    assert_eq!(rows[0].kind.as_deref(), Some("dividend"), "kind survives");
    assert_eq!(rows[0].fees, "0");
    assert_eq!(version(&journal), before + 1, "one bump for the edit");

    // Delete it (aggregate unchanged).
    let deleted = journal
        .delete_transaction(
            row.id,
            hid,
            None,
            "10",
            "100",
            None,
            &ts("2026-07-02T11:00:00Z"),
        )
        .expect("the dividend delete applies");
    assert!(deleted);
    assert!(journal.list_transactions(hid).unwrap().is_empty());
    assert_eq!(version(&journal), before + 2, "one bump for the delete");
}
