//! Integration tests — the FR28 FX-rate store (Story 6.5, AC1): natural-key upsert semantics
//! (insert / in-place update / identical no-op, exact bump counts), the deterministic list order,
//! and `latest_fx_rate`'s dated arbitration (≤-date window, created_at tie-break, never the
//! inverted pair).

use steadyinvest_contract::Timestamp;
use steadyinvest_persistence::{FxRateItem, Journal};
use tempfile::TempDir;
use uuid::Uuid;

fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

fn fresh(dir: &TempDir) -> Journal {
    let path = dir.path().join("journal.db");
    let jid = Uuid::from_u128(0xC0FFEE);
    Journal::create(&path, jid, &ts("2026-07-01T00:00:00Z")).expect("a fresh journal creates")
}

fn version(journal: &Journal) -> u64 {
    journal.logical_version().expect("logical_version reads")
}

/// A rate row BASE→QUOTE with a caller-minted id and clock stamp (ADD15).
fn rate(
    id: u128,
    base: &str,
    quote: &str,
    rate: &str,
    rate_date: &str,
    source: &str,
    created_at: &str,
) -> FxRateItem {
    FxRateItem {
        id: Uuid::from_u128(id),
        base_currency: base.to_string(),
        quote_currency: quote.to_string(),
        rate: rate.to_string(),
        rate_date: rate_date.to_string(),
        source: source.to_string(),
        created_at: ts(created_at),
    }
}

#[test]
fn insert_writes_the_row_verbatim_and_bumps_exactly_once() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let before = version(&journal);

    let item = rate(
        0xF001,
        "EUR",
        "CHF",
        "0.9412",
        "2026-07-01",
        "manuel",
        "2026-07-01T09:00:00Z",
    );
    assert!(
        journal.upsert_fx_rate(&item).expect("the insert applies"),
        "a fresh insert reports applied"
    );
    assert_eq!(version(&journal), before + 1, "exactly one version bump");

    let rows = journal.list_fx_rates().expect("list reads");
    assert_eq!(
        rows,
        vec![item],
        "the row round-trips verbatim (TEXT exact)"
    );
}

#[test]
fn identical_re_upsert_is_a_true_no_op() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    let item = rate(
        0xF001,
        "EUR",
        "CHF",
        "0.9412",
        "2026-07-01",
        "manuel",
        "2026-07-01T09:00:00Z",
    );
    journal.upsert_fx_rate(&item).expect("the insert applies");
    let before = version(&journal);

    // The same natural key with the identical rate — even under a DIFFERENT caller id and clock
    // stamp — writes nothing and bumps nothing (Epic-3 C4).
    let replay = rate(
        0xF999,
        "EUR",
        "CHF",
        "0.9412",
        "2026-07-01",
        "manuel",
        "2026-07-02T09:00:00Z",
    );
    assert!(
        !journal.upsert_fx_rate(&replay).expect("the no-op succeeds"),
        "an identical-values re-upsert reports not-applied"
    );
    assert_eq!(version(&journal), before, "no bump on a no-op");
    let rows = journal.list_fx_rates().expect("list reads");
    assert_eq!(rows.len(), 1, "no duplicate row");
    assert_eq!(rows[0].id, Uuid::from_u128(0xF001), "the original id kept");
    assert_eq!(rows[0].created_at, ts("2026-07-01T09:00:00Z"));
}

#[test]
fn changed_rate_updates_in_place_keeping_the_id_and_refreshing_created_at() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    journal
        .upsert_fx_rate(&rate(
            0xF001,
            "EUR",
            "CHF",
            "0.9412",
            "2026-07-01",
            "twelvedata",
            "2026-07-01T09:00:00Z",
        ))
        .expect("the insert applies");
    let before = version(&journal);

    // The same-day re-fetch came back with a corrected rate: UPDATE in place, no duplicate.
    let refetch = rate(
        0xF777,
        "EUR",
        "CHF",
        "0.9425",
        "2026-07-01",
        "twelvedata",
        "2026-07-01T18:00:00Z",
    );
    assert!(
        journal
            .upsert_fx_rate(&refetch)
            .expect("the update applies"),
        "a changed rate reports applied"
    );
    assert_eq!(version(&journal), before + 1, "exactly one version bump");

    let rows = journal.list_fx_rates().expect("list reads");
    assert_eq!(rows.len(), 1, "updated in place — no duplicate row");
    assert_eq!(rows[0].rate, "0.9425", "the rate changed");
    assert_eq!(
        rows[0].id,
        Uuid::from_u128(0xF001),
        "the existing id is kept (not the re-fetch caller's)"
    );
    assert_eq!(
        rows[0].created_at,
        ts("2026-07-01T18:00:00Z"),
        "created_at refreshes to the correcting write (2026-07-02 review: the later \
         write must win the same-day arbitration tie)"
    );
}

#[test]
fn list_order_is_deterministic_pair_then_date_desc_then_source() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    // Inserted deliberately out of order across pairs, dates and sources.
    for item in [
        rate(
            1,
            "USD",
            "CHF",
            "0.88",
            "2026-07-01",
            "manuel",
            "2026-07-01T09:00:00Z",
        ),
        rate(
            2,
            "EUR",
            "CHF",
            "0.94",
            "2026-06-30",
            "manuel",
            "2026-07-01T09:01:00Z",
        ),
        rate(
            3,
            "EUR",
            "CHF",
            "0.95",
            "2026-07-01",
            "twelvedata",
            "2026-07-01T09:02:00Z",
        ),
        rate(
            4,
            "EUR",
            "CHF",
            "0.96",
            "2026-07-01",
            "eodhd",
            "2026-07-01T09:03:00Z",
        ),
        rate(
            5,
            "EUR",
            "USD",
            "1.07",
            "2026-07-01",
            "manuel",
            "2026-07-01T09:04:00Z",
        ),
    ] {
        journal.upsert_fx_rate(&item).expect("the insert applies");
    }

    let keys: Vec<(String, String, String, String)> = journal
        .list_fx_rates()
        .expect("list reads")
        .into_iter()
        .map(|r| (r.base_currency, r.quote_currency, r.rate_date, r.source))
        .collect();
    let expected: Vec<(String, String, String, String)> = [
        // Pair ASC; within a pair the most recent date first; a same-date tie by source.
        ("EUR", "CHF", "2026-07-01", "eodhd"),
        ("EUR", "CHF", "2026-07-01", "twelvedata"),
        ("EUR", "CHF", "2026-06-30", "manuel"),
        ("EUR", "USD", "2026-07-01", "manuel"),
        ("USD", "CHF", "2026-07-01", "manuel"),
    ]
    .into_iter()
    .map(|(b, q, d, s)| (b.to_string(), q.to_string(), d.to_string(), s.to_string()))
    .collect();
    assert_eq!(keys, expected, "pair ASC, rate_date DESC, source ASC");
}

#[test]
fn latest_picks_the_greatest_dated_row_and_respects_on_or_before() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    for item in [
        rate(
            1,
            "EUR",
            "CHF",
            "0.93",
            "2026-06-15",
            "manuel",
            "2026-06-15T09:00:00Z",
        ),
        rate(
            2,
            "EUR",
            "CHF",
            "0.94",
            "2026-06-30",
            "manuel",
            "2026-06-30T09:00:00Z",
        ),
        rate(
            3,
            "EUR",
            "CHF",
            "0.95",
            "2026-07-01",
            "manuel",
            "2026-07-01T09:00:00Z",
        ),
    ] {
        journal.upsert_fx_rate(&item).expect("the insert applies");
    }

    // No bound → the absolute latest.
    let latest = journal
        .latest_fx_rate("EUR", "CHF", None)
        .expect("latest reads")
        .expect("a rate exists");
    assert_eq!(
        (latest.rate.as_str(), latest.rate_date.as_str()),
        ("0.95", "2026-07-01")
    );

    // Bounded → the greatest rate_date ≤ the asked day (an exact hit counts).
    let bounded = journal
        .latest_fx_rate("EUR", "CHF", Some("2026-06-30"))
        .expect("latest reads")
        .expect("a rate exists on or before");
    assert_eq!(
        (bounded.rate.as_str(), bounded.rate_date.as_str()),
        ("0.94", "2026-06-30")
    );
    let between = journal
        .latest_fx_rate("EUR", "CHF", Some("2026-06-20"))
        .expect("latest reads")
        .expect("the earlier rate is found");
    assert_eq!(
        between.rate_date, "2026-06-15",
        "the ≤-window walks back to the prior date"
    );

    // A bound before every stored date → honestly absent.
    assert_eq!(
        journal
            .latest_fx_rate("EUR", "CHF", Some("2026-06-01"))
            .expect("latest reads"),
        None,
        "no rate on or before the bound"
    );
}

#[test]
fn latest_never_consults_the_inverted_pair_and_is_none_when_absent() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    // Empty store → None (not an error).
    assert_eq!(
        journal
            .latest_fx_rate("EUR", "CHF", None)
            .expect("latest reads"),
        None,
        "an empty store is honestly absent"
    );

    // Only the INVERTED pair exists: asking for EUR→CHF must never guess 1/rate from CHF→EUR.
    journal
        .upsert_fx_rate(&rate(
            1,
            "CHF",
            "EUR",
            "1.0625",
            "2026-07-01",
            "manuel",
            "2026-07-01T09:00:00Z",
        ))
        .expect("the insert applies");
    assert_eq!(
        journal
            .latest_fx_rate("EUR", "CHF", None)
            .expect("latest reads"),
        None,
        "the inverted pair is never consulted"
    );
    assert!(
        journal
            .latest_fx_rate("CHF", "EUR", None)
            .expect("latest reads")
            .is_some(),
        "the exact stored direction is found"
    );
}

#[test]
fn manual_and_provider_rows_coexist_on_a_date_and_the_later_created_at_wins_the_tie() {
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    // A provider fetch in the morning, a manual override in the evening — same pair, same day,
    // DIFFERENT sources: two rows by design (the natural key keeps them apart).
    journal
        .upsert_fx_rate(&rate(
            1,
            "EUR",
            "CHF",
            "0.9412",
            "2026-07-01",
            "twelvedata",
            "2026-07-01T09:00:00Z",
        ))
        .expect("the provider row applies");
    journal
        .upsert_fx_rate(&rate(
            2,
            "EUR",
            "CHF",
            "0.9400",
            "2026-07-01",
            "manuel",
            "2026-07-01T18:00:00Z",
        ))
        .expect("the manual row applies");

    let rows = journal.list_fx_rates().expect("list reads");
    assert_eq!(
        rows.len(),
        2,
        "distinct sources on the same (pair, date) coexist"
    );

    // The rate_date tie is broken by created_at DESC: the later (manual) write wins, and the
    // full item names its source so the caller can show it (FR28).
    let latest = journal
        .latest_fx_rate("EUR", "CHF", None)
        .expect("latest reads")
        .expect("a rate exists");
    assert_eq!(
        latest.source, "manuel",
        "the later created_at wins the same-day tie"
    );
    assert_eq!(latest.rate, "0.9400");
}

#[test]
fn delete_removes_the_row_bumps_once_and_an_absent_id_is_a_no_op() {
    // Issue #90: the Réglages panel's repair path — retract a mis-entered/stranded rate by id.
    let dir = TempDir::new().unwrap();
    let mut journal = fresh(&dir);
    journal
        .upsert_fx_rate(&rate(
            1,
            "EUR",
            "CHF",
            "0.94",
            "2026-07-01",
            "manuel",
            "2026-07-01T09:00:00Z",
        ))
        .expect("the insert applies");
    journal
        .upsert_fx_rate(&rate(
            2,
            "USD",
            "CHF",
            "0.88",
            "2026-07-01",
            "manuel",
            "2026-07-01T09:01:00Z",
        ))
        .expect("the insert applies");
    let before = version(&journal);

    // Deleting a present id removes exactly that row and bumps once.
    assert!(
        journal
            .delete_fx_rate(Uuid::from_u128(1))
            .expect("the delete succeeds"),
        "deleting a present id reports removed"
    );
    assert_eq!(version(&journal), before + 1, "exactly one version bump");
    let rows = journal.list_fx_rates().expect("list reads");
    assert_eq!(rows.len(), 1, "only the targeted row is gone");
    assert_eq!(rows[0].id, Uuid::from_u128(2), "the other row is untouched");
    assert_eq!(
        journal.latest_fx_rate("EUR", "CHF", None).expect("reads"),
        None,
        "the deleted pair is honestly absent again"
    );

    // Deleting an absent id is an idempotent no-op: no removal, no bump.
    let after_delete = version(&journal);
    assert!(
        !journal
            .delete_fx_rate(Uuid::from_u128(0xDEAD))
            .expect("the no-op succeeds"),
        "an absent id reports not-removed"
    );
    assert_eq!(version(&journal), after_delete, "no bump on a no-op delete");
}
