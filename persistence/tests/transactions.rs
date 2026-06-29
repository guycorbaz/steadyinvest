//! Integration tests — Story 4.7 recorded-sell (FR46/FR47) on the v4 `transactions` columns.

use steadyinvest_contract::Timestamp;
use steadyinvest_persistence::Journal;
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
        .add_holding(hid, pid, "NESN", "10", "100", &ts("2026-06-29T10:00:00Z"))
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
