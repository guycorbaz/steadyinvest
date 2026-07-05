//! Watchlist CRUD (Story 4.1, FR34): watched securities ordered by position — add/edit/delete/move
//! — plus the case-insensitive most-recent-study auto-match backing the watch→study link. All
//! writes ride the shared read-only / no-journal guards and map persistence errors to neutral
//! notices via [`super::watch_error`].

use steadyinvest_persistence::WatchItem;
use uuid::Uuid;

use super::{JournalState, MSG_BLANK_TICKER, MSG_NO_JOURNAL, MSG_READ_ONLY_WRITE, watch_error};

impl JournalState {
    // ── Watchlist (Story 4.1, FR34) ──

    /// Every watched security, ordered by position. Empty when no journal is open.
    pub fn list_watch_items(&self) -> Vec<WatchItem> {
        let Some(journal) = self.journal.as_ref() else {
            return Vec::new();
        };
        journal.list_watch_items().unwrap_or_else(|error| {
            tracing::warn!("list_watch_items failed: {error}");
            Vec::new()
        })
    }

    /// The most-recent saved study whose ticker matches `ticker` **case-insensitively** (Story 4.1
    /// watchlist link auto-match), or `None`. `list_studies` is ascending by `created_at`, so the
    /// last match is the newest. Case-insensitive so a watched `"nesn"` still finds the `"NESN"`
    /// study (tickers are stored as entered, not normalized).
    pub fn study_id_for_ticker(&self, ticker: &str) -> Option<Uuid> {
        self.list_studies()
            .into_iter()
            .rev()
            .find(|s| s.security_ticker.eq_ignore_ascii_case(ticker))
            .map(|s| s.id)
    }

    /// Add a watched security (FR34) — appended at the end. Id/timestamp from the injected sources
    /// (ADD15). Guarded (read-only / no-journal / save-failure → a neutral notice).
    pub fn add_watch_item(&mut self, ticker: &str, study_id: Option<Uuid>) -> Result<(), String> {
        let ticker = ticker.trim();
        if ticker.is_empty() {
            return Err(MSG_BLANK_TICKER.to_string());
        }
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let id = self.idgen.new_id();
        let created_at = self.clock.now();
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .add_watch_item(id, ticker, study_id, &created_at)
            .map(|_| ())
            .map_err(watch_error)
    }

    /// Edit a watched security's ticker and/or study link (FR34). Blank ticker is refused.
    pub fn update_watch_item(
        &mut self,
        id: Uuid,
        ticker: &str,
        study_id: Option<Uuid>,
    ) -> Result<(), String> {
        let ticker = ticker.trim();
        if ticker.is_empty() {
            return Err(MSG_BLANK_TICKER.to_string());
        }
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .update_watch_item(id, ticker, study_id)
            .map_err(watch_error)
    }

    /// Remove a watched security (FR34); the remaining rows re-pack to contiguous positions.
    pub fn delete_watch_item(&mut self, id: Uuid) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal.delete_watch_item(id).map_err(watch_error)
    }

    /// Move a watched security one slot up (`up = true`) or down in the order (FR34): swap its
    /// position with its neighbour. A no-op at the list edge.
    pub fn move_watch_item(&mut self, id: Uuid, up: bool) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let items = self.list_watch_items();
        let Some(index) = items.iter().position(|w| w.id == id) else {
            return Ok(()); // gone — nothing to move
        };
        let neighbour = if up {
            index.checked_sub(1)
        } else {
            (index + 1 < items.len()).then_some(index + 1)
        };
        let Some(n) = neighbour else {
            return Ok(()); // already at the edge — a neutral no-op
        };
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .set_watch_positions(&[
                (items[index].id, items[n].position),
                (items[n].id, items[index].position),
            ])
            .map_err(watch_error)
    }
}
