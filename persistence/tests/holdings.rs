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
        .add_holding(id, pid, ticker, qty, price, "CHF", &created)
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

    // No-op edit (identical values, same currency) bumps nothing.
    journal
        .update_holding(id, "NESN", "10", "95.40", "CHF")
        .unwrap();
    assert_eq!(
        journal.logical_version().unwrap(),
        v_after_add,
        "no-op edit is silent"
    );
    // Editing an absent id bumps nothing.
    journal
        .update_holding(Uuid::from_u128(0xDEAD), "X", "1", "1", "CHF")
        .unwrap();
    assert_eq!(
        journal.logical_version().unwrap(),
        v_after_add,
        "absent edit is silent"
    );

    // A real edit changes the row and bumps.
    journal
        .update_holding(id, "NESN.SW", "12", "96.00", "CHF")
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
                "CHF",
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

#[test]
fn changing_a_holdings_ticker_clears_its_trailing_stop_but_qty_price_edits_keep_it() {
    // Story 4.5 review: a stop level is seeded from a security's price/cost; on a TICKER change it
    // would persist (ratchet-up-only) against the new security → a false breach. So a ticker edit
    // clears the stop; editing only quantity/price leaves it intact.
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let id = add_at(&mut journal, "NESN", "10", "100", 0);
    journal
        .set_trailing_stop(id, Some("15"), Some("85"))
        .unwrap();

    // Edit only quantity + price (same ticker) → the stop is kept.
    journal
        .update_holding(id, "NESN", "12", "110", "CHF")
        .unwrap();
    let h = &journal.list_holdings(portfolio_id()).unwrap()[0];
    assert_eq!(
        h.trailing_stop_pct.as_deref(),
        Some("15"),
        "qty/price edit keeps the stop"
    );
    assert_eq!(h.trailing_stop_level.as_deref(), Some("85"));

    // Change the ticker → the stop clears (both fields NULL).
    journal
        .update_holding(id, "ROG", "12", "110", "CHF")
        .unwrap();
    let h = &journal.list_holdings(portfolio_id()).unwrap()[0];
    assert_eq!(h.security_ticker, "ROG");
    assert!(
        h.trailing_stop_pct.is_none() && h.trailing_stop_level.is_none(),
        "a ticker change clears the now-stale stop"
    );
}

// ── Story 6.2 — multi-currency holdings (FR38) ──

#[test]
fn holding_stores_and_reads_back_its_currency() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let pid = portfolio_id();
    journal
        .ensure_portfolio(pid, "Portefeuille", &ts("2026-07-02T09:00:00Z"))
        .unwrap();
    journal
        .add_holding(
            Uuid::from_u128(0xE04),
            pid,
            "ASML",
            "3",
            "620.00",
            "EUR",
            &ts("2026-07-02T10:00:00Z"),
        )
        .unwrap();
    let h = &journal.list_holdings(pid).unwrap()[0];
    assert_eq!(
        h.currency.as_deref(),
        Some("EUR"),
        "the holding's native currency is stored and read back"
    );
}

#[test]
fn editing_only_the_currency_bumps_once_and_an_identical_currency_edit_is_a_no_op() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let id = add_at(&mut journal, "NESN", "10", "100", 0); // add_at stores "CHF"
    let v0 = journal.logical_version().unwrap();

    // Same ticker/qty/price, a NEW currency → a real change that bumps once.
    journal
        .update_holding(id, "NESN", "10", "100", "EUR")
        .unwrap();
    assert_eq!(
        journal.logical_version().unwrap(),
        v0 + 1,
        "a currency-only edit is a real change"
    );
    assert_eq!(
        journal.list_holdings(portfolio_id()).unwrap()[0]
            .currency
            .as_deref(),
        Some("EUR")
    );

    // Repeat with the identical currency → a true no-op (no bump).
    journal
        .update_holding(id, "NESN", "10", "100", "EUR")
        .unwrap();
    assert_eq!(
        journal.logical_version().unwrap(),
        v0 + 1,
        "an identical-currency edit bumps nothing (C4)"
    );
}

// ── Story 6.1 — multiple portfolios (FR37) ──

use steadyinvest_persistence::DeletePortfolioOutcome;

fn pid(n: u128) -> Uuid {
    Uuid::from_u128(0x6100 + n)
}

#[test]
fn add_and_list_portfolios_in_deterministic_order() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    journal
        .add_portfolio(pid(1), "UBS — compte titres", &ts("2026-06-30T09:00:00Z"))
        .unwrap();
    journal
        .add_portfolio(pid(2), "PostFinance", &ts("2026-06-30T09:01:00Z"))
        .unwrap();
    let names: Vec<_> = journal
        .list_portfolios()
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(
        names,
        ["UBS — compte titres", "PostFinance"],
        "ordered by id"
    );
}

#[test]
fn rename_bumps_once_identical_rename_is_a_no_op() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    journal
        .add_portfolio(pid(1), "UBS", &ts("2026-06-30T09:00:00Z"))
        .unwrap();
    let v0 = journal.logical_version().unwrap();
    assert!(journal.rename_portfolio(pid(1), "UBS Switzerland").unwrap());
    let v1 = journal.logical_version().unwrap();
    assert_eq!(v1, v0 + 1, "a real rename bumps once");
    // Identical name → no-op, no bump (C4).
    assert!(!journal.rename_portfolio(pid(1), "UBS Switzerland").unwrap());
    assert_eq!(
        journal.logical_version().unwrap(),
        v1,
        "identical rename is a true no-op"
    );
    assert_eq!(
        journal.list_portfolios().unwrap()[0].name,
        "UBS Switzerland"
    );
}

#[test]
fn delete_is_guarded_against_holdings_and_the_last_portfolio() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    // `add_at` ensures the default `portfolio_id()` and puts a holding in it.
    add(&mut journal, "NESN", "10", "95.40");
    let default_pid = portfolio_id();
    journal
        .add_portfolio(pid(1), "PostFinance", &ts("2026-06-30T09:00:00Z"))
        .unwrap();

    // The default portfolio has a holding → refused (FK never orphaned).
    assert_eq!(
        journal.delete_portfolio(default_pid).unwrap(),
        DeletePortfolioOutcome::HasHoldings
    );
    assert_eq!(
        journal.list_portfolios().unwrap().len(),
        2,
        "nothing deleted"
    );

    // The empty, non-last portfolio deletes + bumps.
    let v0 = journal.logical_version().unwrap();
    assert_eq!(
        journal.delete_portfolio(pid(1)).unwrap(),
        DeletePortfolioOutcome::Deleted
    );
    assert_eq!(
        journal.logical_version().unwrap(),
        v0 + 1,
        "a real delete bumps once"
    );
    assert_eq!(journal.list_portfolios().unwrap().len(), 1);

    // Now only the (holding-bearing) default remains → deleting it is refused as HasHoldings
    // BEFORE the last-portfolio check (the holding guard runs first).
    assert_eq!(
        journal.delete_portfolio(default_pid).unwrap(),
        DeletePortfolioOutcome::HasHoldings
    );
}

#[test]
fn delete_last_empty_portfolio_is_refused() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    // One empty portfolio, no holdings.
    journal
        .add_portfolio(pid(1), "Only", &ts("2026-06-30T09:00:00Z"))
        .unwrap();
    assert_eq!(
        journal.delete_portfolio(pid(1)).unwrap(),
        DeletePortfolioOutcome::LastPortfolio,
        "the register keeps at least one portfolio"
    );
    assert_eq!(journal.list_portfolios().unwrap().len(), 1);
}
