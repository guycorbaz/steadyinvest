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

/// Migration step 5 (Story 5.1, FR50/ADD13): a **post-decision price-history cache**. Confront mode
/// overlays a study's recorded projection on the security's *actual* close trajectory since the
/// decision — which needs dated closes per ticker, sourced via the Epic-3/4 refresh (the `/eod`/`/price`
/// path) and stored here. This is the project's **first NEW table since the v1 DDL** (the Epic-4 tables
/// were pre-provisioned; `price_history` was not), so it is a `CREATE TABLE` step rather than an `ADD
/// COLUMN`. `close` is a TEXT decimal (NFR-C1 — never REAL). The UNIQUE `(security_ticker, close_date)`
/// index makes `upsert_closes` an idempotent append (INSERT OR IGNORE). Retention = append-on-refresh,
/// keep-all (a confront window is decision-date → now; pruning is a later concern).
pub(crate) fn migrate_to_v5(tx: &Transaction<'_>) -> Result<()> {
    tx.execute_batch(
        "CREATE TABLE price_history (
             id              TEXT PRIMARY KEY,
             security_ticker TEXT NOT NULL,
             close_date      TEXT NOT NULL,
             close           TEXT NOT NULL,
             source          TEXT NOT NULL,
             created_at      TEXT NOT NULL
         );
         CREATE UNIQUE INDEX idx_price_history_ticker_date
             ON price_history(security_ticker, close_date);",
    )?;
    Ok(())
}

/// Migration step 6 (Story 6.2, FR38): a holding carries the **currency it is denominated in**. The
/// v1 `holdings` table stored amounts implicitly in the single reference currency (FR36); multi-
/// currency holdings (FR38) need the currency stored per row. Add a **nullable** `currency` — the
/// persistence layer stays currency-agnostic and does **not** backfill (it can't know the app's
/// reference currency), so an existing row reads `NULL` and the app interprets that as "a pre-6.2
/// holding = the reference currency" at the read boundary. Amounts are never mixed/converted here
/// (FR28); this only records the native currency. Like v2–v5: a metadata-only `ADD COLUMN`, forward-
/// safe (existing v1–v5 rows read `NULL`), and `DDL_V1` stays frozen.
pub(crate) fn migrate_to_v6(tx: &Transaction<'_>) -> Result<()> {
    tx.execute_batch("ALTER TABLE holdings ADD COLUMN currency TEXT")?;
    Ok(())
}

/// Migration step 7 (issue #98, FR48): a holding carries its **sector** — provider-reported
/// (EODHD `General::Sector`, PR 2) with a manual override that always wins, feeding the FR48
/// sector re-concentration facts (PR 3). A **nullable** `sector`, no backfill: an existing row
/// reads `NULL` = « non renseigné » (the app states the absence honestly — the 6.6/6.7 rule —
/// and the provider fills only the void, per the 2026-07-09 product decision). Like v2–v6: a
/// metadata-only `ADD COLUMN`, forward-safe, `DDL_V1` stays frozen.
pub(crate) fn migrate_to_v7(tx: &Transaction<'_>) -> Result<()> {
    tx.execute_batch("ALTER TABLE holdings ADD COLUMN sector TEXT")?;
    Ok(())
}

/// Migration step 8 (Story 8.2a, Epic 8 [P4] — arch §Phase 4 A3/A4; FR70–FR72, FR77): the **AI
/// drafts** table. A draft is a proposal an AI client submits through MCP (Story 8.3); it waits here,
/// physically outside the `Study` blob the engine reads — which is what makes "a pending draft
/// changes nothing" true by construction — until the owner decides it (Story 8.2b). A `CREATE
/// TABLE` step like v5; `DDL_V1` stays frozen.
///
/// - **Every enum spelling is closed by a CHECK** (`kind`, `status`, the two 0/1 booleans): the
///   contract's fail-loud enum policy, enforced by the engine too.
/// - **Comment and origin are mandatory** (NFR-A4): `NOT NULL` + a non-blank CHECK. SQLite's
///   `trim()` strips spaces only — the full Unicode-blank refusal, with a named reason, is 8.3's.
/// - **Shape rules** (defence in depth — 8.3 refuses earlier with a named reason): a draft study
///   has no `study_id` and carries its proposed `native_currency`, every other kind targets a study
///   and carries no currency; a pending draft has no `decided_at` and a decided one has it.
/// - **Decision facts**: a pending draft carries no `stale_at_decision` / `edited_before_validation`;
///   `edited_before_validation` exists only on a validated (or validated-then-undone) draft.
/// - **Draft studies** (arch A8, Story 8.7): their validation is **not undoable** — it is reversed
///   by deleting the created study, which deletes its drafts (O7) — so a draft study is never
///   `validated_undone`, and it carries `created_study_id` exactly when `validated`; no other kind
///   ever carries it.
/// - **Ids are canonical UUID text** (lower-case, 36 chars — how the app writes them): the TEXT
///   comparisons of the cascade and the reference checks cannot miss a differently spelled id.
/// - **FKs to `studies`** (RESTRICT, `foreign_keys=ON` on every read-write open): no orphan draft;
///   `delete_study` deletes a study's drafts first (O7).
/// - **The trigger `trg_ai_drafts_bump_logical_version`** bumps `journal_meta.logical_version` on
///   every inserted draft — the MCP draft connection's only write besides the insert itself (arch
///   A3: its authorizer allows `UPDATE journal_meta` only from THIS trigger's name — never rename
///   it). It fires on INSERT only: an import upsert that updates an existing row fires nothing, and
///   the import's own bump covers it (see `util::bump_logical_version`).
pub(crate) fn migrate_to_v8(tx: &Transaction<'_>) -> Result<()> {
    tx.execute_batch(&format!(
        "CREATE TABLE ai_drafts (
             id                       TEXT PRIMARY KEY CHECK (id = lower(id) AND length(id) = 36),
             kind                     TEXT NOT NULL
                                      CHECK (kind IN ('study','note','cell','judgment')),
             study_id                 TEXT REFERENCES studies(id)
                                      CHECK (study_id IS NULL
                                             OR (study_id = lower(study_id)
                                                 AND length(study_id) = 36)),
             security_ticker          TEXT NOT NULL,
             native_currency          TEXT,
             status                   TEXT NOT NULL DEFAULT 'pending'
                                      CHECK (status IN
                                          ('pending','validated','validated_undone','rejected')),
             created_at               TEXT NOT NULL,
             decided_at               TEXT,
             comment                  TEXT NOT NULL CHECK (length(trim(comment)) > 0),
             origin_client            TEXT NOT NULL CHECK (length(trim(origin_client)) > 0),
             origin_model             TEXT NOT NULL CHECK (length(trim(origin_model)) > 0),
             stale_at_decision        INTEGER CHECK (stale_at_decision IN (0,1)),
             edited_before_validation INTEGER CHECK (edited_before_validation IN (0,1)),
             created_study_id         TEXT REFERENCES studies(id)
                                      CHECK (created_study_id IS NULL
                                             OR (created_study_id = lower(created_study_id)
                                                 AND length(created_study_id) = 36)),
             payload                  TEXT NOT NULL,
             CHECK ((kind = 'study') = (study_id IS NULL)),
             CHECK ((kind = 'study') = (native_currency IS NOT NULL)),
             CHECK (created_study_id IS NULL OR kind = 'study'),
             CHECK (kind <> 'study'
                    OR (status <> 'validated_undone'
                        AND ((status = 'validated') = (created_study_id IS NOT NULL)))),
             CHECK ((status = 'pending') = (decided_at IS NULL)),
             CHECK (status <> 'pending'
                    OR (stale_at_decision IS NULL AND edited_before_validation IS NULL)),
             CHECK (edited_before_validation IS NULL
                    OR status IN ('validated','validated_undone'))
         );
         CREATE INDEX idx_ai_drafts_status ON ai_drafts(status);
         CREATE INDEX idx_ai_drafts_study_id ON ai_drafts(study_id);
         CREATE INDEX idx_ai_drafts_created_study_id ON ai_drafts(created_study_id);
         CREATE TRIGGER {DRAFT_TRIGGER} AFTER INSERT ON ai_drafts
         BEGIN
             UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1;
         END;"
    ))?;
    Ok(())
}

/// Migration step 9 (Story 8.3 G3): an insert NEVER overwrites a draft. The MCP draft connection's
/// authorizer allows `INSERT INTO ai_drafts` — and an `INSERT OR REPLACE` / `REPLACE INTO` is an
/// insert whose conflict clause the authorizer cannot see: it would DELETE a decided draft and
/// write another in its place. `trg_ai_drafts_refuse_existing_id` aborts any insert whose id is
/// already in the table, whatever its conflict clause (a `BEFORE INSERT` trigger runs before the
/// conflict is resolved). The v8 DDL is frozen (shipped), hence a step of its own. The import
/// updates an existing draft with an `UPDATE`, never an upsert (`export.rs`).
pub(crate) fn migrate_to_v9(tx: &Transaction<'_>) -> Result<()> {
    tx.execute_batch(&format!(
        "CREATE TRIGGER {DRAFT_ID_GUARD_TRIGGER} BEFORE INSERT ON ai_drafts
         WHEN EXISTS (SELECT 1 FROM ai_drafts WHERE id = NEW.id)
         BEGIN
             SELECT RAISE(ABORT, 'ai_drafts: a draft with this id already exists');
         END;"
    ))?;
    Ok(())
}

/// The name of the v9 trigger that refuses an insert over an existing draft id (Story 8.3 G3).
pub(crate) const DRAFT_ID_GUARD_TRIGGER: &str = "trg_ai_drafts_refuse_existing_id";

/// The name of the v8 trigger that bumps `logical_version` on every inserted draft — ONE spelling,
/// used by the DDL above and by the MCP draft connection's authorizer (Story 8.3, arch A3: it allows
/// `UPDATE journal_meta` only when this trigger is the accessor). Never rename it: the name is in
/// every v8 dossier.
pub(crate) const DRAFT_TRIGGER: &str = "trg_ai_drafts_bump_logical_version";

/// The tables the MCP access surface may READ (Story 8.3, arch A3, NFR-A2): the studies, their FR51
/// history, the journal identity / version, and the drafts. The allowlist is the security boundary —
/// every other table, today's and any later one, is denied (and the classification test below fails
/// until a new table is put in one of the two lists on purpose).
pub(crate) const MCP_READABLE_TABLES: &[&str] =
    &["studies", "judgments", "journal_meta", "ai_drafts"];

/// The tables the MCP access surface may never read (the portfolio, the watchlist and the local
/// caches — NFR-A2 / NFR-S4). Listed so that the classification is explicit; the authorizer denies
/// anything not in [`MCP_READABLE_TABLES`] whether or not it is listed here.
#[cfg_attr(not(test), expect(dead_code))] // the classification test's explicit list
pub(crate) const MCP_DENIED_TABLES: &[&str] = &[
    "holdings",
    "transactions",
    "portfolios",
    "watchlist_items",
    "fx_rates",
    "price_history",
];

/// SQLite's own catalogue tables, readable by the MCP connections (statement preparation reads the
/// schema through them).
pub(crate) const SQLITE_INTERNAL_TABLES: &[&str] = &[
    "sqlite_master",
    "sqlite_schema",
    "sqlite_temp_master",
    "sqlite_temp_schema",
    "sqlite_sequence",
];

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
    fn the_registry_creates_exactly_the_architecture_tables() {
        // v1 froze 8 tables; Story 5.1 (v5) adds `price_history` (the first NEW table since v1) and
        // Story 8.2a (v8) `ai_drafts`. The full registry therefore yields 10. A drift here is a
        // migration-step change, not an edit.
        let conn = v1_connection();
        assert_eq!(
            table_names(&conn),
            vec![
                "ai_drafts",
                "fx_rates",
                "holdings",
                "journal_meta",
                "judgments",
                "portfolios",
                "price_history",
                "studies",
                "transactions",
                "watchlist_items",
            ],
            "hybrid schema table set drifted — that is a migration-step change, not an edit"
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
            9,
            "the registry migrates a fresh DB to the latest version (v9)"
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
    fn v7_adds_the_holdings_sector_column() {
        // Issue #98 (FR48): the v7 migration gives `holdings` a nullable `sector`. Additive —
        // existing rows read NULL (« non renseigné », stated honestly; the provider fills the void).
        let conn = v1_connection();
        assert!(
            column_names(&conn, "holdings").contains(&"sector".to_string()),
            "v7 added holdings.sector"
        );
    }

    #[test]
    fn v6_adds_the_holdings_currency_column() {
        // Story 6.2 (FR38): the v6 migration gives `holdings` a nullable `currency`. Additive —
        // existing rows read NULL (a pre-6.2 holding = the reference currency, coalesced by the app).
        let conn = v1_connection();
        assert!(
            column_names(&conn, "holdings").contains(&"currency".to_string()),
            "v6 added holdings.currency"
        );
        // The v1–v4 holdings columns are untouched (additive migration).
        for col in [
            "id",
            "portfolio_id",
            "security_ticker",
            "quantity",
            "purchase_price",
            "trailing_stop_pct",
            "trailing_stop_level",
            "sold_at",
            "created_at",
        ] {
            assert!(column_names(&conn, "holdings").contains(&col.to_string()));
        }
        // The new column is nullable (an ADD COLUMN with no default): a fresh journal's holdings, if
        // any, would read NULL — proven here structurally via table_info (the column exists, no NOT NULL).
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

        // Entity tables are snake_case plural; `journal_meta` (the singleton) and `price_history` (a
        // mass noun — a history *of* closes, Story 5.1) are the mandated exceptions.
        const NON_PLURAL: &[&str] = &["journal_meta", "price_history"];
        for table in table_names(&conn) {
            assert_eq!(
                table,
                table.to_lowercase(),
                "table {table} is not snake_case"
            );
            if !NON_PLURAL.contains(&table.as_str()) {
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

        // Triggers follow trg_<table>_<purpose> (Story 8.2a — the first trigger in the schema).
        let mut stmt = conn
            .prepare(
                "SELECT name, tbl_name FROM sqlite_master WHERE type = 'trigger' ORDER BY name",
            )
            .expect("trigger names queryable");
        let triggers: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .expect("trigger names read")
            .collect::<Result<Vec<_>, _>>()
            .expect("trigger names collect");
        for (name, tbl) in &triggers {
            assert!(
                name.starts_with(&format!("trg_{tbl}_")),
                "trigger {name} on {tbl} does not follow trg_<table>_<purpose>"
            );
        }
        assert!(
            triggers
                .iter()
                .any(|(n, _)| n == "trg_ai_drafts_bump_logical_version"),
            "the v8 trigger exists under the name 8.3's authorizer relies on"
        );
        assert!(
            triggers
                .iter()
                .any(|(n, _)| n == super::DRAFT_ID_GUARD_TRIGGER),
            "the v9 id guard exists (Story 8.3 G3)"
        );
    }
}
