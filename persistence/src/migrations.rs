//! The migrations harness — versioned, ordered steps applied via `PRAGMA user_version`.
//!
//! This is the **permanent mechanism**: v2+ later just appends a step to [`REGISTRY`]. Each step
//! runs inside its **own transaction**, which also sets `user_version` to the step's number — a
//! step either fully applies (DDL + version stamp) or leaves the file untouched.
//!
//! **Version axes** (never conflated — see `contract::versioning`): `PRAGMA user_version` is the
//! SQL-schema axis and THE migration trigger; `contract::SCHEMA_VERSION` is the serde-blob axis
//! (lazy upgrade-on-save is the documented policy — with only v1 existing it needs no code yet);
//! `core::METHOD_VERSION` is not persistence's business.
//!
//! A file whose `user_version` is **greater** than the latest registered step is never migrated
//! or written here — the runner refuses and defers to the read-only open path (NFR-R3).

use crate::error::{Error, Result};
use rusqlite::{Connection, Transaction};

/// One migration step: applies its DDL/DML on the supplied transaction. The runner stamps
/// `user_version` and commits.
pub(crate) type MigrationStep = fn(&Transaction<'_>) -> Result<()>;

/// The ordered registry of all known migrations. Append-only; numbers are strictly ascending.
/// v2 (Story 4.1): `watchlist_items.study_id` — the watchlist→study soft link (FR34).
/// v3 (Story 4.5): `holdings.trailing_stop_level` — the ratcheted trailing-stop price (FR42).
/// v4 (Story 4.7): `transactions.kind` + `transactions.rationale` — the recorded-sell fields (FR46/47).
/// v5 (Story 5.1): `price_history` — the post-decision close cache (FR50/ADD13).
/// v6 (Story 6.2): `holdings.currency` — the per-holding denomination currency (FR38).
pub(crate) const REGISTRY: &[(u32, MigrationStep)] = &[
    (1, crate::schema::migrate_to_v1),
    (2, crate::schema::migrate_to_v2),
    (3, crate::schema::migrate_to_v3),
    (4, crate::schema::migrate_to_v4),
    (5, crate::schema::migrate_to_v5),
    (6, crate::schema::migrate_to_v6),
];

/// The newest schema version this build knows how to produce.
pub(crate) fn latest_version(registry: &[(u32, MigrationStep)]) -> u32 {
    registry.last().map(|(v, _)| *v).unwrap_or(0)
}

/// Read the file's current `PRAGMA user_version`.
pub(crate) fn user_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// Apply every registered step newer than the file's `user_version`, each in its own
/// transaction that also stamps the new version. Idempotent: a reopen at the latest version
/// applies nothing. Refuses (with the read-only cause) when the file is newer than the registry.
pub(crate) fn run_pending(conn: &mut Connection, registry: &[(u32, MigrationStep)]) -> Result<()> {
    let current = user_version(conn)?;
    let latest = latest_version(registry);
    if current > i64::from(latest) {
        return Err(Error::NewerJournalSchema {
            file_user_version: current,
            supported: latest,
        });
    }
    for (version, step) in registry {
        if i64::from(*version) <= current {
            continue;
        }
        let tx = conn.transaction()?;
        step(&tx).map_err(|e| Error::Migration {
            version: *version,
            source: Box::new(e),
        })?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        Connection::open_in_memory().expect("in-memory sqlite opens")
    }

    #[test]
    fn registry_versions_are_strictly_ascending() {
        for pair in REGISTRY.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "REGISTRY versions out of order: {} then {}",
                pair[0].0,
                pair[1].0
            );
        }
        assert_eq!(
            latest_version(REGISTRY),
            6,
            "v1 (1.10) + v2 (4.1 watchlist study_id) + v3 (4.5 holdings trailing_stop_level) + v4 (4.7 transactions kind/rationale) + v5 (5.1 price_history) + v6 (6.2 holdings currency)"
        );
    }

    #[test]
    fn fresh_database_migrates_to_latest() {
        let mut conn = mem();
        assert_eq!(user_version(&conn).expect("pragma reads"), 0);
        run_pending(&mut conn, REGISTRY).expect("v1 … v6 apply");
        assert_eq!(
            user_version(&conn).expect("pragma reads"),
            6,
            "a fresh DB migrates to the latest known version"
        );
    }

    #[test]
    fn rerun_is_idempotent_no_step_reruns() {
        let mut conn = mem();
        run_pending(&mut conn, REGISTRY).expect("first run applies v1 … v6");
        // A second run re-executing migrate_to_v1 would fail on CREATE TABLE (tables exist), and the
        // ALTER steps would fail on a duplicate ADD COLUMN: success here proves no step re-ran.
        run_pending(&mut conn, REGISTRY).expect("second run is a no-op");
        assert_eq!(user_version(&conn).expect("pragma reads"), 6);
    }

    #[test]
    fn v3_adds_the_holdings_trailing_stop_level_column() {
        // Forward-migration (NFR-R3): a migrate-to-latest exposes `holdings.trailing_stop_level`.
        // Selecting it succeeds only if the column exists (a missing column errors); SQLite's
        // `ADD COLUMN` defaults it to NULL on every existing row (Story 4.5 / FR42).
        let mut conn = mem();
        run_pending(&mut conn, REGISTRY).expect("v1 + v2 + v3 + v4 apply");
        conn.query_row("SELECT COUNT(trailing_stop_level) FROM holdings", [], |r| {
            r.get::<_, i64>(0)
        })
        .expect("trailing_stop_level column exists after v3");
    }

    #[test]
    fn v4_adds_the_transactions_kind_rationale_and_holdings_sold_at_columns() {
        // Forward-migration (NFR-R3): a migrate-to-latest exposes the new sell-recording columns.
        // Selecting them succeeds only if they exist; SQLite's `ADD COLUMN` defaults each to NULL on
        // every existing row (Story 4.7 / FR46/FR47).
        let mut conn = mem();
        run_pending(&mut conn, REGISTRY).expect("v1 + v2 + v3 + v4 apply");
        conn.query_row(
            "SELECT COUNT(kind), COUNT(rationale) FROM transactions",
            [],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .expect("transactions.kind + .rationale exist after v4");
        conn.query_row("SELECT COUNT(sold_at) FROM holdings", [], |r| {
            r.get::<_, i64>(0)
        })
        .expect("holdings.sold_at exists after v4");
    }

    // ── A fake FUTURE step (v7) on top of the real registry (latest = v6): ordering + per-step
    // stamping. The fake step shifts up by one each time a real migration lands (was v6 before 6.2). ──

    fn fake_v7(tx: &Transaction<'_>) -> Result<()> {
        // Writes into a table created by step 1 — fails loudly if steps ran out of order.
        tx.execute_batch(
            "INSERT INTO watchlist_items (id, security_ticker, position, created_at)
             VALUES ('migration-marker-v7', 'TEST', 0, '2026-01-01T00:00:00Z')",
        )?;
        Ok(())
    }

    const SEVEN_STEP_REGISTRY: &[(u32, MigrationStep)] = &[
        (1, crate::schema::migrate_to_v1),
        (2, crate::schema::migrate_to_v2),
        (3, crate::schema::migrate_to_v3),
        (4, crate::schema::migrate_to_v4),
        (5, crate::schema::migrate_to_v5),
        (6, crate::schema::migrate_to_v6),
        (7, fake_v7),
    ];

    fn marker_rows(conn: &Connection) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM watchlist_items WHERE id = 'migration-marker-v7'",
            [],
            |r| r.get(0),
        )
        .expect("marker count reads")
    }

    #[test]
    fn steps_apply_in_order_from_zero() {
        let mut conn = mem();
        run_pending(&mut conn, SEVEN_STEP_REGISTRY).expect("v1 → … → v6 → v7 apply in order");
        assert_eq!(user_version(&conn).expect("pragma reads"), 7);
        assert_eq!(marker_rows(&conn), 1);
    }

    #[test]
    fn only_pending_steps_apply_from_latest() {
        let mut conn = mem();
        run_pending(&mut conn, REGISTRY).expect("v1 … v6 apply");
        run_pending(&mut conn, SEVEN_STEP_REGISTRY).expect("only v7 applies on top");
        assert_eq!(user_version(&conn).expect("pragma reads"), 7);
        assert_eq!(
            marker_rows(&conn),
            1,
            "v7 ran exactly once; v1–v6 did not re-run (CREATE TABLE / duplicate ADD COLUMN would fail)"
        );
        // Idempotence at the new latest too.
        run_pending(&mut conn, SEVEN_STEP_REGISTRY).expect("no-op at latest");
        assert_eq!(marker_rows(&conn), 1, "no step re-ran");
    }

    #[test]
    fn newer_file_is_refused_not_migrated() {
        let mut conn = mem();
        run_pending(&mut conn, SEVEN_STEP_REGISTRY).expect("file at v7");
        let err = run_pending(&mut conn, REGISTRY)
            .expect_err("a build knowing only v1–v6 refuses a v7 file");
        match err {
            Error::NewerJournalSchema {
                file_user_version: 7,
                supported: 6,
            } => {}
            other => panic!("expected NewerJournalSchema, got {other:?}"),
        }
        assert_eq!(
            user_version(&conn).expect("pragma reads"),
            7,
            "refusal leaves the file untouched"
        );
    }

    #[test]
    fn failed_step_leaves_user_version_at_previous_step() {
        fn failing_v7(tx: &Transaction<'_>) -> Result<()> {
            tx.execute_batch("INSERT INTO no_such_table VALUES (1)")?;
            Ok(())
        }
        const FAILING: &[(u32, MigrationStep)] = &[
            (1, crate::schema::migrate_to_v1),
            (2, crate::schema::migrate_to_v2),
            (3, crate::schema::migrate_to_v3),
            (4, crate::schema::migrate_to_v4),
            (5, crate::schema::migrate_to_v5),
            (6, crate::schema::migrate_to_v6),
            (7, failing_v7),
        ];
        let mut conn = mem();
        let err = run_pending(&mut conn, FAILING).expect_err("v7 step fails");
        match err {
            Error::Migration { version: 7, .. } => {}
            other => panic!("expected Migration {{ version: 7 }}, got {other:?}"),
        }
        assert_eq!(
            user_version(&conn).expect("pragma reads"),
            6,
            "v1–v6 committed, the failing v7 rolled back wholly (own-transaction rule)"
        );
    }
}
