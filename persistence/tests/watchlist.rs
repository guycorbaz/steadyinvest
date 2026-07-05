//! Integration tests — Story 4.1 watchlist CRUD (FR34) on a v2 journal.

use steadyinvest_contract::Timestamp;
use steadyinvest_persistence::Journal;
use tempfile::TempDir;
use uuid::Uuid;

fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

fn fresh(dir: &TempDir) -> (Journal, Uuid) {
    let path = dir.path().join("journal.db");
    let jid = Uuid::from_u128(0xC0FFEE);
    let journal =
        Journal::create(&path, jid, &ts("2026-06-27T00:00:00Z")).expect("a fresh journal creates");
    (journal, jid)
}

/// Add a watch item with a deterministic id derived from the ticker (avoids the `uuid/v4` feature).
fn add(journal: &mut Journal, ticker: &str, study: Option<Uuid>) -> Uuid {
    let id = Uuid::from_u128(
        ticker
            .bytes()
            .fold(1u128, |acc, b| acc * 131 + u128::from(b)),
    );
    journal
        .add_watch_item(id, ticker, study, &ts("2026-06-27T10:00:00Z"))
        .expect("add watch item");
    id
}

#[test]
fn add_list_orders_by_position_and_assigns_contiguous_positions() {
    let dir = TempDir::new().unwrap();
    let (mut journal, _) = fresh(&dir);
    add(&mut journal, "NESN", None);
    add(&mut journal, "ROG", None);
    add(&mut journal, "NOVN", None);

    let items = journal.list_watch_items().unwrap();
    let tickers: Vec<_> = items.iter().map(|i| i.security_ticker.as_str()).collect();
    assert_eq!(
        tickers,
        ["NESN", "ROG", "NOVN"],
        "insertion order = position order"
    );
    assert_eq!(
        items.iter().map(|i| i.position).collect::<Vec<_>>(),
        [0, 1, 2],
        "positions are contiguous 0-based"
    );
}

#[test]
fn study_link_round_trips_and_can_be_set_and_cleared() {
    let dir = TempDir::new().unwrap();
    let (mut journal, _) = fresh(&dir);
    let study = Uuid::from_u128(0x57);
    let id = add(&mut journal, "NESN", Some(study));

    assert_eq!(journal.list_watch_items().unwrap()[0].study_id, Some(study));
    // Edit: change ticker, clear the link.
    journal.update_watch_item(id, "NESN.SW", None).unwrap();
    let item = &journal.list_watch_items().unwrap()[0];
    assert_eq!(item.security_ticker, "NESN.SW");
    assert_eq!(item.study_id, None, "the study link was cleared");
}

#[test]
fn reorder_moves_positions_and_relist_reflects_it() {
    let dir = TempDir::new().unwrap();
    let (mut journal, _) = fresh(&dir);
    let a = add(&mut journal, "A", None);
    let b = add(&mut journal, "B", None);
    let c = add(&mut journal, "C", None);

    // Move C to the front: C=0, A=1, B=2.
    journal
        .set_watch_positions(&[(c, 0), (a, 1), (b, 2)])
        .unwrap();
    let tickers: Vec<_> = journal
        .list_watch_items()
        .unwrap()
        .into_iter()
        .map(|i| i.security_ticker)
        .collect();
    assert_eq!(tickers, ["C", "A", "B"]);
}

#[test]
fn delete_repacks_positions_to_contiguous() {
    let dir = TempDir::new().unwrap();
    let (mut journal, _) = fresh(&dir);
    add(&mut journal, "A", None);
    let b = add(&mut journal, "B", None);
    add(&mut journal, "C", None);

    journal.delete_watch_item(b).unwrap();
    let items = journal.list_watch_items().unwrap();
    assert_eq!(
        items
            .iter()
            .map(|i| i.security_ticker.as_str())
            .collect::<Vec<_>>(),
        ["A", "C"]
    );
    assert_eq!(
        items.iter().map(|i| i.position).collect::<Vec<_>>(),
        [0, 1],
        "positions re-packed contiguous after the middle delete"
    );
}

#[test]
fn mutations_bump_logical_version_but_no_ops_do_not() {
    let dir = TempDir::new().unwrap();
    let (mut journal, _) = fresh(&dir);
    let v0 = journal.logical_version().unwrap();
    let id = add(&mut journal, "NESN", None);
    let v1 = journal.logical_version().unwrap();
    assert!(v1 > v0, "add bumps the version");

    // A no-op update (identical ticker + link) bumps nothing.
    journal.update_watch_item(id, "NESN", None).unwrap();
    assert_eq!(
        journal.logical_version().unwrap(),
        v1,
        "no-op update is silent"
    );
    // A reorder to the current order bumps nothing.
    journal.set_watch_positions(&[(id, 0)]).unwrap();
    assert_eq!(
        journal.logical_version().unwrap(),
        v1,
        "no-op reorder is silent"
    );
    // Deleting an absent id bumps nothing.
    journal.delete_watch_item(Uuid::from_u128(0xDEAD)).unwrap();
    assert_eq!(
        journal.logical_version().unwrap(),
        v1,
        "absent delete is silent"
    );

    // A real edit bumps.
    journal.update_watch_item(id, "ROG", None).unwrap();
    assert!(journal.logical_version().unwrap() > v1, "a real edit bumps");
}

#[test]
fn watchlist_survives_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.db");
    let jid = Uuid::from_u128(0xC0FFEE);
    {
        let mut journal = Journal::create(&path, jid, &ts("2026-06-27T00:00:00Z")).unwrap();
        add(&mut journal, "NESN", Some(Uuid::from_u128(0x9)));
        add(&mut journal, "ROG", None);
    }
    let reopened = Journal::open(&path).expect("reopen migrates/opens at v2");
    let items = reopened.list_watch_items().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].security_ticker, "NESN");
    assert_eq!(items[0].study_id, Some(Uuid::from_u128(0x9)));
}

#[test]
fn deleting_a_linked_study_clears_the_watchlist_link() {
    use steadyinvest_contract::{ForecastLowOption, Judgment, SCHEMA_VERSION, Study};
    let dir = TempDir::new().unwrap();
    let (mut journal, jid) = fresh(&dir);
    let study_id = Uuid::from_u128(0x1234);
    let study = Study {
        id: study_id,
        journal_id: jid,
        security_ticker: "NESN".to_string(),
        native_currency: "CHF".to_string(),
        years: Vec::new(),
        judgment: Judgment {
            estimated_high_eps: None,
            estimated_low_eps: None,
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: None,
            forecast_low_option: ForecastLowOption::AvgLowPriceLast5y,
            recent_severe_low: None,
            current_price: None,
            present_full_year_dividend: None,
            ttm_eps: None,
        },
        rationale: None,
        created_at: ts("2026-06-27T00:00:00Z"),
        schema_version: SCHEMA_VERSION,
    };
    journal.put_study(&study).unwrap();
    add(&mut journal, "NESN", Some(study_id));

    // Deleting the study must clear the watchlist soft link (no orphan), entry survives.
    journal.delete_study(study_id).unwrap();
    let items = journal.list_watch_items().unwrap();
    assert_eq!(
        items.len(),
        1,
        "the watchlist entry survives the study delete"
    );
    assert_eq!(items[0].study_id, None, "the dangling link was cleared");
}
