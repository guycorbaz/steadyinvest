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
    /// study (tickers are stored as entered, not normalized). Absence-blind — a consumer that
    /// STATES absence must use [`Self::try_study_id_for_ticker`] (issue #95).
    pub fn study_id_for_ticker(&self, ticker: &str) -> Option<Uuid> {
        self.try_study_id_for_ticker(ticker).ok().flatten()
    }

    /// Fallible [`Self::study_id_for_ticker`] (issue #95): `Ok(None)` is a true no-match, `Err` a
    /// read failure — so « aucune étude » is never stated over a failed listing.
    pub fn try_study_id_for_ticker(&self, ticker: &str) -> Result<Option<Uuid>, String> {
        Ok(self
            .try_list_studies()?
            .into_iter()
            .rev()
            .find(|s| s.security_ticker.eq_ignore_ascii_case(ticker))
            .map(|s| s.id))
    }

    /// Issue #81: like [`Self::study_id_for_ticker`], but a holding auto-match ALSO requires the same
    /// currency — so a CHF holding never links a USD study of the same ticker (which would then price
    /// its sale / ratchet its stop / display its register row at the wrong-currency figure). A holding
    /// with **no** declared currency (`None`) falls back to the ticker-only match (today's behaviour).
    /// The watchlist link (no currency) keeps using [`Self::study_id_for_ticker`]. Absence-blind —
    /// a consumer that STATES absence must use [`Self::try_matched_study_in_currency`] (issue #95).
    pub fn study_id_for_ticker_in_currency(
        &self,
        ticker: &str,
        currency: Option<&str>,
    ) -> Option<Uuid> {
        self.try_study_id_for_ticker_in_currency(ticker, currency)
            .ok()
            .flatten()
    }

    /// Fallible [`Self::study_id_for_ticker_in_currency`] (issue #95): `Ok(None)` is a true
    /// no-match, `Err` a read failure — including a candidate whose payload could not be parsed
    /// (the absence-blind version silently skipped it).
    pub fn try_study_id_for_ticker_in_currency(
        &self,
        ticker: &str,
        currency: Option<&str>,
    ) -> Result<Option<Uuid>, String> {
        // No declared currency → keep the cheap ticker-only match (today's behaviour).
        let Some(currency) = currency else {
            return self.try_study_id_for_ticker(ticker);
        };
        // The currency lives in the JSON payload (not an indexed summary column), so load only the
        // same-ticker candidates — usually 0–2 — newest-first, and take the first currency match.
        for summary in self
            .try_list_studies()?
            .into_iter()
            .rev()
            .filter(|s| s.security_ticker.eq_ignore_ascii_case(ticker))
        {
            let Some(study) = self.try_get_study(summary.id)? else {
                continue; // deleted between the listing and the read — a true absence
            };
            if study.native_currency.eq_ignore_ascii_case(currency) {
                return Ok(Some(study.id));
            }
        }
        Ok(None)
    }

    /// The tri-state holding→study auto-match (issue #95): `Ok(Some)` the matched study,
    /// `Ok(None)` truly none — the only case a consumer may state « aucune étude liée » —
    /// `Err` a read failure (« indisponible »).
    pub fn try_matched_study_in_currency(
        &self,
        ticker: &str,
        currency: Option<&str>,
    ) -> Result<Option<steadyinvest_contract::Study>, String> {
        match self.try_study_id_for_ticker_in_currency(ticker, currency)? {
            Some(sid) => self.try_get_study(sid),
            None => Ok(None),
        }
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
