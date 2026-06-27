//! Study storage — the blob side of the hybrid model.
//!
//! A [`Study`] is persisted **whole** as its serde-JSON payload (scale-preserving `Money`
//! strings included); indexed columns are extracted from the same struct. **Every mutating call
//! runs in a single transaction that also increments `journal_meta.logical_version`** — the
//! journal's heartbeat (NFR-R2; ADD6's stale-restore detection reads this counter): study row and
//! version bump commit together or not at all.

use crate::error::{Error, Result};
use crate::journal::Journal;
use rusqlite::OptionalExtension;
use steadyinvest_contract::{Study, Timestamp, SCHEMA_VERSION};
use uuid::Uuid;

/// One row of [`Journal::list_studies`]: the indexed columns only — no payload parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudySummary {
    pub id: Uuid,
    pub security_ticker: String,
    pub created_at: Timestamp,
    /// `'active'` in v1 (archive/delete are Epic 2 features).
    pub status: String,
}

impl Journal {
    /// Insert or update a study, atomically bumping the journal's logical version.
    ///
    /// Upsert is `INSERT … ON CONFLICT(id) DO UPDATE` — NOT `INSERT OR REPLACE`, whose implicit
    /// DELETE+INSERT would, once Epic 2 writes `judgments` rows, either FK-fail or cascade-delete
    /// the FR51 time-series on every re-save. `status` and `method_version` are not touched on
    /// update: they belong to Epic 2 features (`'active'` literal / `NULL` on first insert).
    ///
    /// A study whose `journal_id` differs from this journal's identity is rejected — a study
    /// from journal A is never silently written into journal B.
    pub fn put_study(&mut self, study: &Study) -> Result<()> {
        self.check_writable()?;
        if study.journal_id != self.id() {
            return Err(Error::JournalIdentityMismatch {
                study_journal_id: study.journal_id,
                journal_id: self.id(),
            });
        }
        let payload = serde_json::to_string(study)?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO studies
                 (id, journal_id, security_ticker, created_at, status, schema_version,
                  method_version, payload)
             VALUES (?1, ?2, ?3, ?4, 'active', ?5, NULL, ?6)
             ON CONFLICT(id) DO UPDATE SET
                 journal_id = excluded.journal_id,
                 security_ticker = excluded.security_ticker,
                 created_at = excluded.created_at,
                 schema_version = excluded.schema_version,
                 payload = excluded.payload",
            rusqlite::params![
                study.id.to_string(),
                study.journal_id.to_string(),
                study.security_ticker,
                study.created_at.0,
                study.schema_version,
                payload
            ],
        )?;
        tx.execute(
            "UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Read a study back by id, parsing the stored payload. `Ok(None)` when the id is absent.
    ///
    /// A row whose `schema_version` is newer than this build's `contract::SCHEMA_VERSION` fails
    /// with a clear typed error — never a silent partial parse. (Unknown *fields* within a
    /// known-version payload are tolerated by design — the contract's forward-compat rail.)
    pub fn get_study(&self, id: Uuid) -> Result<Option<Study>> {
        let row: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT schema_version, payload FROM studies WHERE id = ?1",
                rusqlite::params![id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((row_schema_version, payload)) = row else {
            return Ok(None);
        };
        if row_schema_version > i64::from(SCHEMA_VERSION) {
            return Err(Error::NewerRowSchema {
                row_schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        let study: Study = serde_json::from_str(&payload)?;
        Ok(Some(study))
    }

    /// List the indexed columns of every study — no payload parse (the Epic 2 dashboard's
    /// building block). Ordered by `created_at` then `id` for a deterministic listing.
    pub fn list_studies(&self) -> Result<Vec<StudySummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, security_ticker, created_at, status
             FROM studies ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id_text, security_ticker, created_at, status) = row?;
            let id = Uuid::parse_str(&id_text).map_err(|e| Error::CorruptPayload {
                detail: format!("studies.id {id_text:?} is not a valid UUID: {e}"),
            })?;
            out.push(StudySummary {
                id,
                security_ticker,
                created_at: Timestamp(created_at),
                status,
            });
        }
        Ok(out)
    }

    /// Set a study's lifecycle `status` (Story 2.12, FR54/FR55): `"archived"` hides it from the
    /// default dashboard view, `"active"` restores it. One transaction that also bumps the journal's
    /// logical version. A pure indexed-column change — the Study blob payload and the FR51
    /// `judgments` time-series are untouched, so archive is fully reversible. An absent id is a no-op
    /// success (idempotent).
    pub fn set_study_status(&mut self, id: Uuid, status: &str) -> Result<()> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        let changed = tx.execute(
            "UPDATE studies SET status = ?1 WHERE id = ?2",
            rusqlite::params![status, id.to_string()],
        )?;
        // Only a real change bumps the heartbeat — an absent id is a true no-op (no phantom version
        // drift, which the stale-restore detection / external sync would otherwise see).
        if changed > 0 {
            tx.execute(
                "UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1",
                [],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Permanently delete a study **and its own FR51 `judgments` time-series rows** in a single
    /// transaction (Story 2.12, FR55 — "without corrupting the journal time-series"): the judgments
    /// rows are removed first so the `judgments.study_id REFERENCES studies(id)` constraint (RESTRICT,
    /// `PRAGMA foreign_keys = true`) is never violated and **no orphan** is left behind; every other
    /// study's rows are untouched. Bumps the logical version. An absent id is a no-op success
    /// (idempotent — nothing to delete is not an error).
    pub fn delete_study(&mut self, id: Uuid) -> Result<()> {
        self.check_writable()?;
        let id_text = id.to_string();
        let tx = self.conn.transaction()?;
        let removed_judgments = tx.execute(
            "DELETE FROM judgments WHERE study_id = ?1",
            rusqlite::params![id_text],
        )?;
        let removed_study = tx.execute(
            "DELETE FROM studies WHERE id = ?1",
            rusqlite::params![id_text],
        )?;
        // Story 4.1 (FR34): a watchlist entry that pointed at this study must not dangle — clear the
        // soft link in the SAME transaction (the column is nullable, never a hard FK). Counts as a
        // real change so the version bumps when a link was actually cleared.
        let cleared_links = tx.execute(
            "UPDATE watchlist_items SET study_id = NULL WHERE study_id = ?1",
            rusqlite::params![id_text],
        )?;
        // Only a real removal bumps the heartbeat — deleting an absent id is a true no-op (no phantom
        // version drift for the stale-restore detection / external sync to see).
        if removed_study > 0 || removed_judgments > 0 || cleared_links > 0 {
            tx.execute(
                "UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1",
                [],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
