//! Integration tests — Story 4.3 holdings CRUD (FR36) on the pre-provisioned v1 tables.

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
    Journal::create(&path, jid, &ts("2026-06-27T00:00:00Z")).expect("a fresh journal creates")
}

/// A deterministic portfolio id (avoids the `uuid/v4` feature in tests).
fn portfolio_id() -> Uuid {
    Uuid::from_u128(0x9001)
}

/// Ensure the single portfolio and add a holding with a deterministic id derived from the ticker.
/// `seq` advances the second-of-minute so `ORDER BY created_at` reflects insertion order in tests.
fn add_at(journal: &mut Journal, ticker: &str, qty: &str, price: &str, seq: u8) -> Uuid {
    let pid = portfolio_id();
    journal
        .ensure_portfolio(pid, "Portefeuille", &ts("2026-06-27T09:00:00Z"))
        .expect("ensure portfolio");
    let id = Uuid::from_u128(ticker.bytes().fold(7u128, |acc, b| {
        acc.wrapping_mul(131).wrapping_add(u128::from(b))
    }));
    let created = ts(&format!("2026-06-27T10:00:{seq:02}Z"));
    journal
        .add_holding(id, pid, ticker, qty, price, &created)
        .expect("add holding");
    id
}

/// Convenience: append at the next free second, derived from the current holding count.
fn add(journal: &mut Journal, ticker: &str, qty: &str, price: &str) -> Uuid {
    let seq = journal
        .list_holdings(portfolio_id())
        .map(|h| h.len() as u8)
        .unwrap_or(0);
    add_at(journal, ticker, qty, price, seq)
}

#[test]
fn add_and_list_returns_holdings_in_creation_order() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    add(&mut journal, "NESN", "10", "95.40");
    add(&mut journal, "ROG", "5", "248.10");

    let items = journal.list_holdings(portfolio_id()).unwrap();
    let tickers: Vec<_> = items.iter().map(|h| h.security_ticker.as_str()).collect();
    assert_eq!(tickers, ["NESN", "ROG"], "creation order is preserved");
    assert_eq!(items[0].quantity, "10");
    assert_eq!(items[0].purchase_price, "95.40");
    assert!(
        items.iter().all(|h| h.trailing_stop_pct.is_none()),
        "trailing stop is NULL in 4.3 (Story 4.5 owns it)"
    );
}

#[test]
fn decimals_round_trip_byte_exact_as_text() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    // A high-precision spelling must survive untouched (no REAL, no rounding).
    add(
        &mut journal,
        "ABC",
        "0.000000000000000001",
        "1234.5678901234567890",
    );
    let h = &journal.list_holdings(portfolio_id()).unwrap()[0];
    assert_eq!(h.quantity, "0.000000000000000001");
    assert_eq!(h.purchase_price, "1234.5678901234567890");
}

#[test]
fn ensure_portfolio_is_idempotent_and_does_not_double_bump() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let pid = portfolio_id();
    let v0 = journal.logical_version().unwrap();
    let first = journal
        .ensure_portfolio(pid, "Portefeuille", &ts("2026-06-27T09:00:00Z"))
        .unwrap();
    let v1 = journal.logical_version().unwrap();
    assert!(v1 > v0, "creating the portfolio bumps once");

    // A second ensure (even with a different id/name) must return the existing row, write nothing.
    let again = journal
        .ensure_portfolio(
            Uuid::from_u128(0xBEEF),
            "Autre",
            &ts("2026-06-27T11:00:00Z"),
        )
        .unwrap();
    assert_eq!(again, first, "the singleton is returned unchanged");
    assert_eq!(
        journal.logical_version().unwrap(),
        v1,
        "ensuring an existing portfolio is silent"
    );
    assert_eq!(
        journal.first_portfolio().unwrap(),
        Some(first),
        "still exactly one portfolio"
    );
}

#[test]
fn edit_changes_values_and_bumps_only_on_a_real_change() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let id = add(&mut journal, "NESN", "10", "95.40");
    let v_after_add = journal.logical_version().unwrap();

    // No-op edit (identical values) bumps nothing.
    journal.update_holding(id, "NESN", "10", "95.40").unwrap();
    assert_eq!(
        journal.logical_version().unwrap(),
        v_after_add,
        "no-op edit is silent"
    );
    // Editing an absent id bumps nothing.
    journal
        .update_holding(Uuid::from_u128(0xDEAD), "X", "1", "1")
        .unwrap();
    assert_eq!(
        journal.logical_version().unwrap(),
        v_after_add,
        "absent edit is silent"
    );

    // A real edit changes the row and bumps.
    journal
        .update_holding(id, "NESN.SW", "12", "96.00")
        .unwrap();
    let h = &journal.list_holdings(portfolio_id()).unwrap()[0];
    assert_eq!(h.security_ticker, "NESN.SW");
    assert_eq!(h.quantity, "12");
    assert_eq!(h.purchase_price, "96.00");
    assert!(
        journal.logical_version().unwrap() > v_after_add,
        "a real edit bumps"
    );
}

#[test]
fn delete_removes_and_absent_delete_is_silent() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let a = add(&mut journal, "A", "1", "1");
    add(&mut journal, "B", "2", "2");
    let v = journal.logical_version().unwrap();

    // Absent delete: no-op.
    journal.delete_holding(Uuid::from_u128(0xDEAD)).unwrap();
    assert_eq!(
        journal.logical_version().unwrap(),
        v,
        "absent delete is silent"
    );

    journal.delete_holding(a).unwrap();
    let items = journal.list_holdings(portfolio_id()).unwrap();
    assert_eq!(
        items
            .iter()
            .map(|h| h.security_ticker.as_str())
            .collect::<Vec<_>>(),
        ["B"],
        "the deleted holding is gone, the other survives"
    );
    assert!(
        journal.logical_version().unwrap() > v,
        "a real delete bumps"
    );
}

#[test]
fn holdings_survive_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let jid = Uuid::from_u128(0xC0FFEE);
    let pid = portfolio_id();
    {
        let mut journal = Journal::create(&path, jid, &ts("2026-06-27T00:00:00Z")).unwrap();
        journal
            .ensure_portfolio(pid, "Portefeuille", &ts("2026-06-27T09:00:00Z"))
            .unwrap();
        journal
            .add_holding(
                Uuid::from_u128(0x1),
                pid,
                "NESN",
                "10",
                "95.40",
                &ts("2026-06-27T10:00:00Z"),
            )
            .unwrap();
    }
    let reopened = Journal::open(&path).expect("reopen opens at the current schema");
    let items = reopened.list_holdings(pid).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].security_ticker, "NESN");
    assert_eq!(items[0].quantity, "10");
    assert_eq!(items[0].purchase_price, "95.40");
}

#[test]
fn set_trailing_stop_round_trips_is_idempotent_and_clears() {
    // Story 4.5 (FR42): the trailing-stop pct + ratcheted level persist together, a no-op set bumps
    // no version (C4), the level can ratchet up, and None/None clears the stop.
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let id = add_at(&mut journal, "NESN", "10", "100", 0);
    let v0 = journal.logical_version().unwrap();

    journal
        .set_trailing_stop(id, Some("15"), Some("85"))
        .unwrap();
    let v1 = journal.logical_version().unwrap();
    assert!(v1 > v0, "setting a stop bumps the version");
    let h = &journal.list_holdings(portfolio_id()).unwrap()[0];
    assert_eq!(h.trailing_stop_pct.as_deref(), Some("15"));
    assert_eq!(h.trailing_stop_level.as_deref(), Some("85"));

    journal
        .set_trailing_stop(id, Some("15"), Some("85"))
        .unwrap();
    assert_eq!(
        journal.logical_version().unwrap(),
        v1,
        "an identical set is a no-op (no version bump)"
    );

    journal
        .set_trailing_stop(id, Some("15"), Some("90"))
        .unwrap();
    assert!(journal.logical_version().unwrap() > v1, "a ratchet bumps");
    assert_eq!(
        journal.list_holdings(portfolio_id()).unwrap()[0]
            .trailing_stop_level
            .as_deref(),
        Some("90")
    );

    journal.set_trailing_stop(id, None, None).unwrap();
    let h = &journal.list_holdings(portfolio_id()).unwrap()[0];
    assert!(
        h.trailing_stop_pct.is_none() && h.trailing_stop_level.is_none(),
        "None/None clears the stop"
    );

    let v = journal.logical_version().unwrap();
    journal
        .set_trailing_stop(Uuid::from_u128(0xDEAD), Some("10"), Some("9"))
        .unwrap();
    assert_eq!(
        journal.logical_version().unwrap(),
        v,
        "an absent id is an idempotent no-op"
    );
}
