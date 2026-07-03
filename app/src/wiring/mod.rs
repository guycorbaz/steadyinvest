//! UI wiring — the per-domain Slint callback registration, split out of `main.rs` so the binary's
//! composition root stays thin. Each `wire_*` submodule registers ONE domain's callbacks on the
//! generated `MainWindow` globals (the types come from `slint::include_modules!()` in `main.rs`,
//! shared here via `crate::` paths), cloning the session-scoped handles it needs from [`Session`]
//! exactly as the closure blocks in `main()` did before the split — purely structural, no behavior
//! change. Shared cross-domain helpers live in `push` (the form/view-state pushers) and here
//! (`persist`); everything else stays with its domain.

pub(crate) mod cells;
pub(crate) mod fetch;
pub(crate) mod fx;
pub(crate) mod holdings;
pub(crate) mod journal;
pub(crate) mod judgment;
pub(crate) mod overlays;
pub(crate) mod prefs;
pub(crate) mod push;
pub(crate) mod replacement;
pub(crate) mod studies;
pub(crate) mod watchlist;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;

use uuid::Uuid;

use crate::config::AppConfig;
use crate::state::{JournalState, UnlockScope};
use crate::wiring::holdings::HoldingFreshnessMap;

/// The session-scoped shared handles the wiring closures capture — the `Rc`/`RefCell` cells
/// created once in `main()` and shared across domains: the open journal + app-config (+ its path,
/// ADD7), the fetch-worker sender (Story 3.1), and the transient per-session UI state (the open
/// study id, the pending unlock / study-action confirmations of Stories 2.5/2.12, the §1 drag and
/// scenario-compare caches of Stories 2.8/2.9, the Story 4.4 holdings freshness + Story 4.7
/// dismissed-trigger sets, and the issue-#52 in-flight refresh count). Each `wire_*` destructures
/// the handles it needs and clones per closure, exactly as the blocks in `main()` did before the
/// split.
pub(crate) struct Session {
    pub(crate) journal_state: Rc<RefCell<JournalState>>,
    pub(crate) config: Rc<RefCell<AppConfig>>,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) fetch_tx: mpsc::Sender<crate::fetch::WorkerJob>,
    pub(crate) current_study: Rc<RefCell<Option<String>>>,
    pub(crate) pending_unlock: Rc<RefCell<Option<UnlockScope>>>,
    pub(crate) pending_study_action: Rc<RefCell<Option<(String, Uuid)>>>,
    pub(crate) drag_study: Rc<RefCell<Option<steadyinvest_contract::Study>>>,
    pub(crate) drag_moved: Rc<RefCell<bool>>,
    pub(crate) compare_study: Rc<RefCell<Option<steadyinvest_contract::Study>>>,
    pub(crate) holding_freshness: Rc<RefCell<HoldingFreshnessMap>>,
    pub(crate) holding_dismissed: Rc<RefCell<std::collections::HashSet<String>>>,
    pub(crate) refresh_pending: Rc<RefCell<usize>>,
}

/// Persist `config`, surfacing (not swallowing) a failure — a config that cannot be written is
/// a visible event, never a silence, but it must not take the app down.
pub(crate) fn persist(path: Option<&PathBuf>, config: &AppConfig) {
    let Some(path) = path else { return };
    if let Err(error) = crate::config::save(path, config) {
        let message = format!("app-config save to {} failed: {error}", path.display());
        tracing::warn!("{message}");
        eprintln!("steadyinvest: {message}");
    }
}
