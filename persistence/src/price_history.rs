//! Post-decision price-history cache (Story 5.1, FR50/ADD13).
//!
//! Confront mode overlays a study's recorded projection on the security's **actual** close trajectory
//! since the decision. That trajectory is dated closes per ticker, sourced via the Epic-3/4 refresh
//! (the provider `/eod`/`/price` path) and cached here (the `price_history` table, migration v5).
//!
//! Append-on-refresh, **keep-all**, deduplicated by `(security_ticker, close_date)`: [`upsert_closes`]
//! is an idempotent `INSERT OR IGNORE` (a re-fetched same-date close is a no-op). [`closes_since`]
//! reads the confront window (decision-date → now), ordered by date. Closes are exact-decimal
//! **TEXT** (NFR-C1 — never REAL).
//!
//! **This cache does NOT bump `journal_meta.logical_version`** — and is the *only* writer that
//! doesn't. `logical_version` identifies the **exported** journal content (the Story-5.3
//! `JournalSnapshot`: studies, watchlist, portfolio, holdings, transactions); `price_history` is a
//! local, reconstructible cache **excluded** from that snapshot. Bumping the identity counter on a
//! price refresh would desync version-from-content (two journals with identical exported content but
//! different cached prices would carry different versions; an export before/after a refresh would
//! yield identical bytes under different claimed versions). So the cache write touches **only**
//! `price_history` — keeping confront strictly read-only of journal identity (AC3).

use crate::error::Result;
use crate::journal::Journal;
use steadyinvest_contract::Timestamp;

/// One dated close to cache: `(close_date "YYYY-MM-DD", close TEXT-decimal, source)`.
pub type ClosePoint<'a> = (&'a str, &'a str, &'a str);

impl Journal {
    /// Append dated closes for a ticker into the price-history cache (Story 5.1). Idempotent: a close
    /// for an already-cached `(ticker, date)` is ignored (the unique index), and the `id` is the
    /// deterministic `"{ticker}:{date}"` (no injected UUID needed — ADD15). `created_at` comes from the
    /// caller's injected clock. Deliberately does **not** bump `logical_version` — the cache is local,
    /// reconstructible, and excluded from the export snapshot (see the module note).
    pub fn upsert_closes(
        &mut self,
        ticker: &str,
        closes: &[ClosePoint<'_>],
        now: &Timestamp,
    ) -> Result<()> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        for (date, close, source) in closes {
            let id = format!("{ticker}:{date}");
            tx.execute(
                "INSERT OR IGNORE INTO price_history
                     (id, security_ticker, close_date, close, source, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![id, ticker, date, close, source, now.0],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The cached closes for a ticker on or after `since_date` (`"YYYY-MM-DD"`), oldest-first (Story
    /// 5.1) — the confront window from the decision date to now. `close_date` is a date string, so the
    /// lexical `>=` is the chronological comparison. Returns `(close_date, close)` pairs.
    pub fn closes_since(&self, ticker: &str, since_date: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT close_date, close FROM price_history
             WHERE security_ticker = ?1 AND close_date >= ?2
             ORDER BY close_date",
        )?;
        let rows = stmt.query_map(rusqlite::params![ticker, since_date], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn ts(s: &str) -> Timestamp {
        Timestamp(s.to_string())
    }

    fn journal() -> (tempfile::TempDir, Journal) {
        let dir = tempdir().unwrap();
        let j = Journal::create(
            dir.path().join("j.db"),
            Uuid::from_u128(0x510),
            &ts("2026-06-01T00:00:00Z"),
        )
        .unwrap();
        (dir, j)
    }

    #[test]
    fn upsert_dedups_by_date_and_closes_since_returns_the_ordered_window() {
        let (_d, mut j) = journal();
        let now = ts("2026-06-30T00:00:00Z");
        j.upsert_closes(
            "NESN",
            &[
                ("2026-06-10", "104.0", "eodhd"),
                ("2026-06-09", "103.0", "eodhd"),
                ("2026-05-01", "98.0", "eodhd"), // before the window below
                ("2026-06-10", "999.0", "eodhd"), // duplicate date → ignored
            ],
            &now,
        )
        .unwrap();

        // The window from the decision date (2026-06-01), oldest-first; the dup kept the first value.
        let window = j.closes_since("NESN", "2026-06-01").unwrap();
        assert_eq!(
            window,
            vec![
                ("2026-06-09".to_string(), "103.0".to_string()),
                ("2026-06-10".to_string(), "104.0".to_string()),
            ],
            "ordered by date, pre-decision close excluded, duplicate date ignored (first wins)"
        );

        // A different ticker / a window with no closes is empty.
        assert!(j.closes_since("ROG", "2026-06-01").unwrap().is_empty());
        assert!(j.closes_since("NESN", "2027-01-01").unwrap().is_empty());
    }

    #[test]
    fn caching_closes_never_bumps_the_journal_identity_version() {
        // The price-history cache is local, reconstructible, and excluded from the export snapshot,
        // so it is the ONE writer that must not touch `logical_version` (the exported-content
        // identity counter) — neither a genuinely new close nor a duplicate may bump it. This keeps
        // version-from-content in sync for Epic-5 export/import/sync (AC3).
        let (_d, mut j) = journal();
        let now = ts("2026-06-30T00:00:00Z");
        let baseline = j.logical_version().unwrap();
        j.upsert_closes("NESN", &[("2026-06-10", "104.0", "eodhd")], &now)
            .unwrap();
        assert_eq!(
            j.logical_version().unwrap(),
            baseline,
            "a brand-new cached close leaves the identity version untouched"
        );
        // A genuinely new date — still no bump.
        j.upsert_closes("NESN", &[("2026-06-11", "105.0", "eodhd")], &now)
            .unwrap();
        // A duplicate — still no bump.
        j.upsert_closes("NESN", &[("2026-06-11", "105.0", "eodhd")], &now)
            .unwrap();
        assert_eq!(
            j.logical_version().unwrap(),
            baseline,
            "the cache never advances the journal identity counter"
        );
    }
}
