//! Hybrid schema v1 — the DDL applied by migration 1, never by ad-hoc statements at open.
//!
//! **Hybrid model** (architecture, Data Architecture): normalized tables for what is later
//! aggregated/queried (`portfolios`, `holdings`, `transactions`, `fx_rates`, `watchlist_items` —
//! DDL-only in v1, their contract types arrive with Epics 4/6) and versioned serde-JSON **blob
//! tables** for what is replayed whole (`studies`, `judgments`).
//!
//! **Binding conventions** (architecture, Naming Patterns): tables snake_case plural
//! (`journal_meta` is the mandated singleton exception), PK `id`, FKs `<entity>_id`, indexes
//! `idx_<table>_<cols>`, timestamps TEXT RFC3339 UTC, and **every monetary/decimal value is a
//! TEXT decimal string — `REAL` is forbidden anywhere in the schema** (NFR-C1; a test below
//! introspects every table and enforces it). No decimal arithmetic in SQL, ever — rows are pulled
//! and computed in Rust with `core`.
//!
//! The v1 columns of the normalized tables are deliberately **minimal, grounded in the FRs**
//! (FR36 holding = security/quantity/purchase price; FR39 transaction = date/quantity/unit
//! price/fees/currency; FR28 fx_rate rows dated & source-aware; FR34 watchlist reorder ⇒
//! `position`; FR42 trailing stop). A v2 migration is EXPECTED when the Epic 4/6 contract types
//! land; the interpretation is recorded in the Story 1.10 GitHub issue.

use crate::error::Result;
use rusqlite::Transaction;

/// Migration step 1: create the whole v1 schema (all tables from birth — later epics fill rather
/// than migrate).
pub(crate) fn migrate_to_v1(tx: &Transaction<'_>) -> Result<()> {
    tx.execute_batch(DDL_V1)?;
    Ok(())
}

/// Migration step 2 (Story 4.1, the project's first real v2): a `watchlist_items` row can reference
/// the saved study whose buy zone it tracks (FR34). The v1 table had no such column, so add a
/// **nullable** `study_id` (a soft link cleared on study delete, never a hard FK — see
/// `studies::delete_study`). SQLite `ALTER TABLE … ADD COLUMN` is a metadata-only change (no table
/// rebuild) and is forward-safe: an existing v1 journal gains the column on open. `DDL_V1` stays
/// frozen — the column exists only via this step, so a fresh v2 DB and a migrated v1 DB are identical.
pub(crate) fn migrate_to_v2(tx: &Transaction<'_>) -> Result<()> {
    tx.execute_batch("ALTER TABLE watchlist_items ADD COLUMN study_id TEXT")?;
    Ok(())
}

/// Migration step 3 (Story 4.5, FR42): a holding carries a **ratcheted trailing-stop level** — the
/// stop price that ratchets up only. The v1 `holdings` table had `trailing_stop_pct` (the parameter)
/// but no level; the level cannot be re-derived from the latest price alone (it loses the high-water
/// mark), so it must be persisted. Add a **nullable** `trailing_stop_level` (NULL when no stop set).
/// Like v2: a metadata-only `ADD COLUMN`, forward-safe (an existing v2 journal gains the column on
/// open with NULL on every row), and `DDL_V1` stays frozen.
pub(crate) fn migrate_to_v3(tx: &Transaction<'_>) -> Result<()> {
    tx.execute_batch("ALTER TABLE holdings ADD COLUMN trailing_stop_level TEXT")?;
    Ok(())
}

/// Migration step 4 (Story 4.7, FR46/FR47): recording a sell. Two facets:
/// - `transactions.kind` (buy/sell discriminator) + `transactions.rationale` (a 4.7-specific
///   free-text reason, absent from FR39's fields) — the persisted **sell record**.
/// - `holdings.sold_at` — a **soft-delete** marker. A sold holding leaves the active register but is
///   **not** hard-deleted: its `transactions.holding_id` FK must keep pointing at a live row, so the
///   record survives. `list_holdings` (the register + capital-at-risk source) filters `sold_at IS
///   NULL`; a `NULL` here means still held.
///
/// All three are **nullable** `ADD COLUMN`s (existing v1–v3 rows read NULL), metadata-only,
/// forward-safe, and `DDL_V1` stays frozen. The full buy/sell ledger (partial sells, weighted-average
/// cost basis, edit/delete) remains Epic 6 / Story 6.3 — 4.7 writes a single SELL row + a soft delete.
pub(crate) fn migrate_to_v4(tx: &Transaction<'_>) -> Result<()> {
    tx.execute_batch(
        "ALTER TABLE transactions ADD COLUMN kind TEXT;
         ALTER TABLE transactions ADD COLUMN rationale TEXT;
         ALTER TABLE holdings ADD COLUMN sold_at TEXT;",
    )?;
    Ok(())
}

/// The complete v1 DDL. Frozen once shipped — schema changes go through new migration steps.
const DDL_V1: &str = "
    -- Journal identity (ADD6): one row, journal_id (UUID) + monotonic logical_version.
    -- logical_version starts at 0 = 'created, never mutated'; every mutating call increments it
    -- in the same transaction as the mutation (NFR-R2).
    CREATE TABLE journal_meta (
        id              INTEGER PRIMARY KEY CHECK (id = 1),
        journal_id      TEXT    NOT NULL,
        logical_version INTEGER NOT NULL,
        created_at      TEXT    NOT NULL
    );

    -- Blob table: whole serde-JSON contract::Study in payload; indexed columns ride alongside.
    -- status/method_version belong to Epic 2 features ('active' literal / NULL in v1).
    CREATE TABLE studies (
        id              TEXT PRIMARY KEY,
        journal_id      TEXT    NOT NULL,
        security_ticker TEXT    NOT NULL,
        created_at      TEXT    NOT NULL,
        status          TEXT    NOT NULL DEFAULT 'active',
        schema_version  INTEGER NOT NULL,
        method_version  TEXT,
        payload         TEXT    NOT NULL
    );
    CREATE INDEX idx_studies_security_ticker ON studies(security_ticker);
    CREATE INDEX idx_studies_status ON studies(status);

    -- Blob table: judgment-snapshot time-series (FR51) — written from Epic 2, DDL lands now.
    -- journal_id intentionally omitted (reachable via study_id).
    CREATE TABLE judgments (
        id             TEXT PRIMARY KEY,
        study_id       TEXT    NOT NULL REFERENCES studies(id),
        created_at     TEXT    NOT NULL,
        schema_version INTEGER NOT NULL,
        payload        TEXT    NOT NULL
    );
    CREATE INDEX idx_judgments_study_id ON judgments(study_id);

    -- Normalized tables (DDL-only in v1; typed CRUD arrives with Epics 4/6).
    CREATE TABLE portfolios (
        id         TEXT PRIMARY KEY,
        name       TEXT NOT NULL,
        created_at TEXT NOT NULL
    );

    -- FR36 (security/quantity/purchase price) + FR42 (trailing stop). Decimal columns are TEXT.
    CREATE TABLE holdings (
        id                TEXT PRIMARY KEY,
        portfolio_id      TEXT NOT NULL REFERENCES portfolios(id),
        security_ticker   TEXT NOT NULL,
        quantity          TEXT NOT NULL,
        purchase_price    TEXT NOT NULL,
        trailing_stop_pct TEXT,
        created_at        TEXT NOT NULL
    );
    CREATE INDEX idx_holdings_portfolio_id ON holdings(portfolio_id);

    -- FR39: date/quantity/unit price/fees/currency. Decimal columns are TEXT.
    CREATE TABLE transactions (
        id          TEXT PRIMARY KEY,
        holding_id  TEXT NOT NULL REFERENCES holdings(id),
        occurred_at TEXT NOT NULL,
        quantity    TEXT NOT NULL,
        unit_price  TEXT NOT NULL,
        fees        TEXT NOT NULL,
        currency    TEXT NOT NULL,
        created_at  TEXT NOT NULL
    );
    CREATE INDEX idx_transactions_holding_id ON transactions(holding_id);

    -- FR28 / architecture: fx_rate rows are dated & source-aware. rate is a TEXT decimal.
    CREATE TABLE fx_rates (
        id             TEXT PRIMARY KEY,
        base_currency  TEXT NOT NULL,
        quote_currency TEXT NOT NULL,
        rate           TEXT NOT NULL,
        rate_date      TEXT NOT NULL,
        source         TEXT NOT NULL,
        created_at     TEXT NOT NULL
    );

    -- FR34: user-ordered watchlist ⇒ a position column.
    CREATE TABLE watchlist_items (
        id              TEXT PRIMARY KEY,
        security_ticker TEXT    NOT NULL,
        position        INTEGER NOT NULL,
        created_at      TEXT    NOT NULL
    );
";

#[cfg(test)]
mod tests {
    use crate::migrations;
    use rusqlite::Connection;

    fn v1_connection() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory sqlite opens");
        migrations::run_pending(&mut conn, migrations::REGISTRY)
            .expect("migration to v1 applies on a fresh database");
        conn
    }

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .expect("sqlite_master is queryable");
        stmt.query_map([], |r| r.get::<_, String>(0))
            .expect("table names read")
            .collect::<Result<Vec<_>, _>>()
            .expect("table names collect")
    }

    #[test]
    fn v1_creates_exactly_the_eight_architecture_tables() {
        let conn = v1_connection();
        assert_eq!(
            table_names(&conn),
            vec![
                "fx_rates",
                "holdings",
                "journal_meta",
                "judgments",
                "portfolios",
                "studies",
                "transactions",
                "watchlist_items",
            ],
            "hybrid schema v1 table set drifted — that is a migration-step change, not an edit"
        );
    }

    fn column_names(conn: &Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table_info is queryable");
        stmt.query_map([], |r| r.get::<_, String>(1))
            .expect("column names read")
            .collect::<Result<Vec<_>, _>>()
            .expect("column names collect")
    }

    #[test]
    fn v2_adds_the_watchlist_study_id_column() {
        // Story 4.1: the v2 migration (in REGISTRY) gives `watchlist_items` a nullable `study_id`.
        let conn = v1_connection();
        assert_eq!(
            migrations::user_version(&conn).expect("user_version reads"),
            4,
            "the registry migrates a fresh DB to the latest version (v4)"
        );
        assert!(
            column_names(&conn, "watchlist_items").contains(&"study_id".to_string()),
            "v2 added watchlist_items.study_id"
        );
        // The v1 columns are untouched (additive migration).
        for col in ["id", "security_ticker", "position", "created_at"] {
            assert!(column_names(&conn, "watchlist_items").contains(&col.to_string()));
        }
    }

    #[test]
    fn v3_adds_the_holdings_trailing_stop_level_column() {
        // Story 4.5 (FR42): the v3 migration gives `holdings` a nullable `trailing_stop_level`.
        let conn = v1_connection();
        assert!(
            column_names(&conn, "holdings").contains(&"trailing_stop_level".to_string()),
            "v3 added holdings.trailing_stop_level"
        );
        // The v1 holdings columns are untouched (additive migration).
        for col in [
            "id",
            "portfolio_id",
            "security_ticker",
            "quantity",
            "purchase_price",
            "trailing_stop_pct",
            "created_at",
        ] {
            assert!(column_names(&conn, "holdings").contains(&col.to_string()));
        }
    }

    #[test]
    fn no_column_anywhere_is_real_money_is_text() {
        // NFR-C1: one REAL column would silently reintroduce floats into the decision chain.
        let conn = v1_connection();
        for table in table_names(&conn) {
            let mut stmt = conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .expect("table_info is queryable");
            let cols = stmt
                .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
                .expect("column info read")
                .collect::<Result<Vec<_>, _>>()
                .expect("column info collect");
            assert!(!cols.is_empty(), "table {table} has columns");
            for (name, decl_type) in cols {
                assert!(
                    !decl_type.eq_ignore_ascii_case("REAL")
                        && !decl_type.to_uppercase().contains("FLOA")
                        && !decl_type.to_uppercase().contains("DOUB"),
                    "column {table}.{name} has floating type {decl_type:?} — money/decimals are \
                     TEXT decimal strings, REAL is forbidden anywhere in the schema (NFR-C1)"
                );
            }
        }
    }

    #[test]
    fn naming_conventions_hold() {
        let conn = v1_connection();

        // Entity tables are snake_case plural; journal_meta is the mandated singleton exception.
        for table in table_names(&conn) {
            assert_eq!(
                table,
                table.to_lowercase(),
                "table {table} is not snake_case"
            );
            if table != "journal_meta" {
                assert!(
                    table.ends_with('s'),
                    "entity table {table} is not plural (Naming Patterns)"
                );
            }
        }

        // Every table's PK is `id`.
        for table in table_names(&conn) {
            let mut stmt = conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .expect("table_info is queryable");
            let pk_cols: Vec<String> = stmt
                .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, i64>(5)?)))
                .expect("pk info read")
                .filter_map(|row| {
                    let (name, pk) = row.expect("pk row");
                    (pk > 0).then_some(name)
                })
                .collect();
            assert_eq!(pk_cols, vec!["id"], "table {table} PK is the `id` column");
        }

        // Indexes follow idx_<table>_<cols>, and the two AC-mandated ones exist.
        let mut stmt = conn
            .prepare(
                "SELECT name, tbl_name FROM sqlite_master
                 WHERE type = 'index' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .expect("index names queryable");
        let indexes: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .expect("index names read")
            .collect::<Result<Vec<_>, _>>()
            .expect("index names collect");
        for (name, tbl) in &indexes {
            assert!(
                name.starts_with(&format!("idx_{tbl}_")),
                "index {name} on {tbl} does not follow idx_<table>_<cols>"
            );
        }
        let names: Vec<&str> = indexes.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"idx_studies_security_ticker"));
        assert!(names.contains(&"idx_studies_status"));
    }
}
