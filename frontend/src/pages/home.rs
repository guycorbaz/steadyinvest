//! Home page — main analysis workspace (`/`).
//!
//! Coordinates ticker selection, data harvesting, snapshot management,
//! file import, and thesis history timeline, then delegates rendering to
//! [`AnalystHUD`] or [`SnapshotHUD`].
//!
//! Layout uses a CSS Grid with named regions (`status`, `chart`, `sidebar`, `hud`)
//! per Architecture spec. The `status` region is reserved for Epic 9 (portfolio signals).

use crate::components::history_timeline::{HistoryResponse, HistoryTimeline, MetricDelta, TimelineEntry};
use crate::components::search_bar::SearchBar;
use crate::components::analyst_hud::AnalystHUD;
use crate::components::snapshot_comparison::SnapshotComparison;
use crate::components::snapshot_hud::SnapshotHUD;
use crate::types::LockedAnalysisModel;
use crate::ActiveLockedAnalysisId;
use leptos::prelude::*;
use leptos_router::hooks::use_location;
use steady_invest_logic::{HistoricalData, TickerInfo, AnalysisSnapshot};
use serde::Deserialize;

/// DTO for the raw `GET /api/v1/snapshots/:id` response used by deep linking.
#[derive(Debug, Clone, Deserialize)]
struct FullSnapshotResponse {
    id: i32,
    ticker_id: i32,
    snapshot_data: serde_json::Value,
    #[allow(dead_code)]
    thesis_locked: bool,
    notes: Option<String>,
    captured_at: chrono::DateTime<chrono::Utc>,
}

/// Main analysis page rendered at `/`.
///
/// Manages ticker selection state, fetches historical data and locked snapshots,
/// renders either the live analysis HUD or a read-only snapshot view, and
/// provides a History Timeline Sidebar for thesis evolution comparison.
#[component]
pub fn Home() -> impl IntoView {
    let (selected_ticker, set_selected_ticker) = signal(Option::<TickerInfo>::None);
    let (target_currency, set_target_currency) = signal("USD".to_string());
    let (selected_snapshot_id, set_selected_snapshot_id) = signal(Option::<i32>::None);
    let (imported_snapshot, set_imported_snapshot) = signal(Option::<AnalysisSnapshot>::None);
    let navigate_home = leptos_router::hooks::use_navigate();

    // History sidebar state (local to this view, not global)
    let (history_open, set_history_open) = signal(false);
    let (selected_past_id, set_selected_past_id) = signal(Option::<i32>::None);
    let history_data: RwSignal<Option<HistoryResponse>> = RwSignal::new(None);

    // Deep linking: read `?snapshot=ID` from URL for Library card navigation
    let location = use_location();
    let deep_link_id = move || {
        let search = location.search.get();
        let s = search.strip_prefix('?').unwrap_or(&search);
        s.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            if k == "snapshot" { v.parse::<i32>().ok() } else { None }
        })
    };

    let deep_link_snapshot = LocalResource::new(move || {
        let snap_id = deep_link_id();
        async move {
            match snap_id {
                Some(id) => {
                    let url = format!("/api/v1/snapshots/{}", id);
                    let response = gloo_net::http::Request::get(&url)
                        .send()
                        .await
                        .map_err(|e| e.to_string())?;
                    if response.ok() {
                        response
                            .json::<FullSnapshotResponse>()
                            .await
                            .map_err(|e| e.to_string())
                    } else {
                        Err(format!("Snapshot not found: {}", response.status()))
                    }
                }
                None => Err("no deep link".to_string()),
            }
        }
    });

    // Sync the active locked analysis ID to app-level context so the
    // Command Strip can enable/disable the PDF export action.
    let locked_ctx = use_context::<ActiveLockedAnalysisId>();
    Effect::new(move |_| {
        if let Some(ctx) = locked_ctx {
            let snap_id = selected_snapshot_id.get();
            let has_import = imported_snapshot.get().is_some();
            let deep_id = if selected_ticker.get().is_none() && !has_import {
                deep_link_id()
            } else {
                None
            };
            // Only expose a real DB snapshot ID (imported snapshots use id=0)
            ctx.0.set(if !has_import { snap_id.or(deep_id) } else { None });
        }
    });

    // Clear the locked analysis context when leaving the Home page so the
    // Command Strip Export PDF button doesn't stay enabled with a stale ID.
    on_cleanup(move || {
        if let Some(ctx) = locked_ctx {
            ctx.0.set(None);
        }
    });

    let historicals = LocalResource::new(move || {
        let ticker_info = selected_ticker.get();
        let target_cur = target_currency.get();
        async move {
            if let Some(info) = ticker_info {
                let url = format!("/api/harvest/{}", info.ticker);
                let response = gloo_net::http::Request::post(&url)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;

                if response.ok() {
                    let mut data = response
                        .json::<HistoricalData>()
                        .await
                        .map_err(|e| e.to_string())?;

                    if !data.is_complete {
                        return Err("Integrity Alert: Data population incomplete for this ticker.".to_string());
                    }

                    if data.currency != target_cur {
                        data.apply_normalization(&target_cur);
                    }

                    Ok(data)
                } else {
                    Err(format!("Harvest failed: {}", response.status()))
                }
            } else {
                Ok::<HistoricalData, String>(HistoricalData::default())
            }
        }
    });

    let snapshots = LocalResource::new(move || {
        let ticker_info = selected_ticker.get();
        async move {
            let res: Result<Vec<LockedAnalysisModel>, String> = if let Some(info) = ticker_info {
                let url = format!("/api/analyses/{}", info.ticker);
                let response = gloo_net::http::Request::get(&url)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;

                if response.ok() {
                    response.json::<Vec<LockedAnalysisModel>>().await.map_err(|e| e.to_string())
                } else {
                    Ok(vec![])
                }
            } else {
                Ok(vec![])
            };
            res
        }
    });

    // Fetch history when sidebar is opened — use latest snapshot ID as anchor
    let history_resource = LocalResource::new(move || {
        let is_open = history_open.get();
        let snap_list = snapshots.get();
        async move {
            if !is_open {
                return Err("Sidebar closed".to_string());
            }
            // Find anchor snapshot ID from existing snapshots list
            let anchor_id = match snap_list {
                Some(Ok(ref list)) if !list.is_empty() => list
                    .iter()
                    .max_by_key(|s| s.created_at)
                    .map(|s| s.id),
                _ => None,
            };
            let Some(id) = anchor_id else {
                return Err("No snapshots available".to_string());
            };
            let url = format!("/api/v1/snapshots/{}/history", id);
            let response = gloo_net::http::Request::get(&url)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if response.ok() {
                response
                    .json::<HistoryResponse>()
                    .await
                    .map_err(|e| e.to_string())
            } else {
                Err(format!("HTTP {}", response.status()))
            }
        }
    });

    // Sync history data into signal for use in comparison cards
    Effect::new(move || {
        if let Some(Ok(resp)) = history_resource.get() {
            history_data.set(Some(resp));
        }
    });

    // Auto-open history sidebar on deep-link if multiple snapshots exist
    Effect::new(move || {
        if deep_link_id().is_some() {
            if let Some(Ok(list)) = snapshots.get() {
                if list.len() > 1 {
                    set_history_open.set(true);
                }
            }
        }
    });

    // Determine if History toggle should be enabled (at least 1 snapshot exists)
    let has_snapshots = move || {
        matches!(snapshots.get(), Some(Ok(ref list)) if !list.is_empty())
    };

    // Toggle history sidebar with focus management
    let toggle_history = move |_| {
        let new_state = !history_open.get_untracked();
        set_history_open.set(new_state);
        if !new_state {
            // Closing sidebar — clear selection and return to live view
            set_selected_past_id.set(None);
            set_selected_snapshot_id.set(None);
        }
    };

    // Navigate helper for timeline selection (must be called during component init)
    let navigate_select = leptos_router::hooks::use_navigate();

    // Handle selection from timeline
    let on_timeline_select = Callback::new(move |id: Option<i32>| {
        set_selected_past_id.set(id);
        if let Some(past_id) = id {
            // Update URL for shareability
            navigate_select(&format!("/?snapshot={}", past_id), Default::default());
            set_selected_snapshot_id.set(Some(past_id));
        } else {
            // Deselected — return to live view
            set_selected_snapshot_id.set(None);
        }
    });

    // Get current snapshot ID (the most recent one, for highlighting in timeline)
    let current_snapshot_id_for_timeline = move || -> Option<i32> {
        match snapshots.get() {
            Some(Ok(ref list)) if !list.is_empty() => list
                .iter()
                .max_by_key(|s| s.created_at)
                .map(|s| s.id),
            _ => None,
        }
    };

    // Find the metric delta originating from the selected past snapshot.
    // NOTE: The API provides consecutive deltas (A→B, B→C). When the selected
    // past snapshot is not immediately before the current, this returns the delta
    // to the next adjacent snapshot, not to the current. This is a known limitation
    // of the consecutive-delta API contract (Story 8.4).
    let find_delta = move |past_id: i32| -> Option<MetricDelta> {
        let data = history_data.get()?;
        data.metric_deltas.iter().find(|d| d.from_snapshot_id == past_id).cloned()
    };

    // Find a timeline entry by ID
    let find_entry = move |target_id: i32| -> Option<TimelineEntry> {
        let data = history_data.get()?;
        data.snapshots.iter().find(|e| e.id == target_id).cloned()
    };

    view! {
        <ErrorBoundary fallback=|errors| {
            view! {
                <div class="error-hub">
                    <h1>"Institutional Data Gap"</h1>
                    <ul>
                        {move || {
                            errors
                                .get()
                                .into_iter()
                                .map(|(_, e)| view! { <li>{e.to_string()}</li> })
                                .collect_view()
                        }}
                    </ul>
                </div>
            }
        }>
            <SearchBar
                on_select=move |info| {
                    set_selected_ticker.set(Some(info));
                    set_selected_snapshot_id.set(None);
                    set_imported_snapshot.set(None);
                    // Close sidebar and clear history when ticker changes
                    set_history_open.set(false);
                    set_selected_past_id.set(None);
                    history_data.set(None);
                    // Clear ?snapshot= from URL to prevent stale deep link on refresh
                    navigate_home("/", Default::default());
                }
                on_import=move |snapshot| {
                    set_imported_snapshot.set(Some(snapshot));
                    set_selected_ticker.set(None);
                    set_history_open.set(false);
                    set_selected_past_id.set(None);
                }
            />

            // Deep-linked snapshot from Library (/?snapshot=ID)
            {move || {
                // Skip if user has actively selected a ticker or imported a file
                if selected_ticker.get().is_some() || imported_snapshot.get().is_some() {
                    return None;
                }
                if deep_link_id().is_none() {
                    return None;
                }
                match deep_link_snapshot.get() {
                    None => Some(view! {
                        <div class="loading-overlay">
                            <div class="pulse"></div>
                            <div class="status-text">"Loading Snapshot..."</div>
                        </div>
                    }.into_any()),
                    Some(Ok(raw)) => {
                        let hd = raw.snapshot_data.get("historical_data");
                        let ticker_sym = hd
                            .and_then(|h| h.get("ticker"))
                            .and_then(|t| t.as_str())
                            .unwrap_or("UNKNOWN")
                            .to_string();
                        let currency = hd
                            .and_then(|h| h.get("currency"))
                            .and_then(|c| c.as_str())
                            .unwrap_or("USD")
                            .to_string();
                        let ticker = TickerInfo {
                            ticker: ticker_sym.clone(),
                            name: ticker_sym,
                            exchange: String::new(),
                            currency,
                        };
                        let model = LockedAnalysisModel {
                            id: raw.id,
                            ticker_id: raw.ticker_id,
                            snapshot_data: raw.snapshot_data,
                            analyst_note: raw.notes.unwrap_or_default(),
                            created_at: raw.captured_at,
                        };
                        Some(view! {
                            <div class="analyst-hud-init">
                                <SnapshotHUD ticker=ticker model=model />
                            </div>
                        }.into_any())
                    }
                    Some(Err(_)) => None,
                }
            }}

            {move || selected_ticker.get().map(|ticker| {
                let grid_class = if history_open.get() {
                    "analysis-grid sidebar-open"
                } else {
                    "analysis-grid"
                };

                view! {
                    <div class=grid_class>
                        // Status region (hidden, reserved for Epic 9)
                        <div class="analysis-status-area"></div>

                        // Chart region — inner analyst-hud-init reuses existing styles
                        <div class="analysis-chart-area">
                          <div class="analyst-hud-init">
                            <div class="header-control-bar top-nav standard-border">
                                <div class="ticker-box">
                                    <h2>{ticker.name.clone()} " (" {ticker.ticker.clone()} ")"</h2>
                                    <div class="hud-meta">
                                        <span>"Ex: " {ticker.exchange.clone()}</span>
                                        " | "
                                        <span>"Rep: " {ticker.currency.clone()}</span>
                                    </div>
                                </div>
                                <div class="hud-controls">
                                    <a
                                        href="/system"
                                        class="system-monitor-link"
                                        title="Go to System Monitor"
                                    >
                                        "SYS_MON"
                                    </a>
                                    <div class="currency-selector">
                                        <label>"Display: "</label>
                                        <select on:change=move |ev| {
                                            set_target_currency.set(event_target_value(&ev));
                                        }>
                                            <option value="USD" selected={move || target_currency.get() == "USD"}>"USD"</option>
                                            <option value="CHF" selected={move || target_currency.get() == "CHF"}>"CHF"</option>
                                            <option value="EUR" selected={move || target_currency.get() == "EUR"}>"EUR"</option>
                                        </select>
                                    </div>
                                    // History toggle button (replaces view-selector dropdown)
                                    <button
                                        class="history-toggle-btn"
                                        on:click=toggle_history
                                        disabled=move || !has_snapshots()
                                        aria-expanded=move || history_open.get().to_string()
                                        aria-controls="history-sidebar"
                                        title=move || {
                                            if !has_snapshots() {
                                                "No saved analyses yet".to_string()
                                            } else if history_open.get() {
                                                "Close history".to_string()
                                            } else {
                                                "Show analysis history".to_string()
                                            }
                                        }
                                    >
                                        {move || if history_open.get() { "History \u{2715}" } else { "History" }}
                                    </button>
                                </div>
                            </div>

                            <Suspense fallback=|| view! {
                                <div class="loading-overlay">
                                    <div class="pulse"></div>
                                    <div class="status-text">"Querying Terminal Data..."</div>
                                </div>
                            }>
                                {move || {
                                    // Priority 1: Imported File Snapshot
                                    if let Some(snapshot) = imported_snapshot.get() {
                                        let ticker_name = snapshot.historical_data.ticker.clone();
                                        let model = LockedAnalysisModel {
                                            id: 0,
                                            ticker_id: 0,
                                            snapshot_data: serde_json::to_value(snapshot.clone()).unwrap(),
                                            analyst_note: snapshot.analyst_note.clone(),
                                            created_at: snapshot.captured_at,
                                        };
                                        let ticker = TickerInfo {
                                            ticker: ticker_name.clone(),
                                            name: format!("{} (Imported)", ticker_name),
                                            exchange: "Portable File".to_string(),
                                            currency: snapshot.historical_data.currency.clone(),
                                        };

                                        return view! {
                                            <div class="import-banner">
                                                "Viewing Imported Analysis"
                                                <button
                                                    class="close-import"
                                                    on:click=move |_| set_imported_snapshot.set(None)
                                                >
                                                    "Close & Return to Terminal"
                                                </button>
                                            </div>
                                            <SnapshotHUD ticker=ticker model=model />
                                        }.into_any();
                                    }

                                    let ticker = ticker.clone();
                                    let target_id = selected_snapshot_id.get();
                                    if let Some(id) = target_id {
                                        // Render Snapshot View
                                        if let Some(Ok(list)) = snapshots.get() {
                                            if let Some(model) = list.iter().find(|s: &&LockedAnalysisModel| s.id == id) {
                                                return view! {
                                                    <SnapshotHUD
                                                        ticker=ticker.clone()
                                                        model=model.clone()
                                                    />
                                                }.into_any();
                                            }
                                        }
                                        view! { <div class="error-msg">"Snapshot not found"</div> }.into_any()
                                    } else {
                                        // Render Live HUD
                                        match historicals.get() {
                                            Some(Ok(res)) if !res.records.is_empty() => {
                                                view! {
                                                    <AnalystHUD
                                                        ticker=ticker.clone()
                                                        data=res
                                                        on_refetch=Callback::new(move |_| {
                                                            historicals.refetch();
                                                            snapshots.refetch();
                                                        })
                                                    />
                                                }.into_any()
                                            }
                                            Some(Err(e)) => view! { <div class="integrity-alert">"Integrity Alert: " {e}</div> }.into_any(),
                                            _ => view! { <div class="awaiting">"Awaiting population trigger..."</div> }.into_any(),
                                        }
                                    }
                                }}
                            </Suspense>
                          </div>
                        </div>

                        // Sidebar region (History Timeline)
                        <div class="analysis-sidebar">
                            {move || {
                                if !history_open.get() {
                                    return view! {}.into_any();
                                }
                                match history_resource.get() {
                                    None => view! {
                                        <div class="timeline-loading">
                                            <div class="pulse"></div>
                                            "Loading history..."
                                        </div>
                                    }.into_any(),
                                    Some(Err(e)) => {
                                        if e == "Sidebar closed" || e == "No snapshots available" {
                                            view! {}.into_any()
                                        } else {
                                            view! {
                                                <div class="timeline-error">
                                                    "Could not load history"
                                                </div>
                                            }.into_any()
                                        }
                                    },
                                    Some(Ok(resp)) => {
                                        view! {
                                            <HistoryTimeline
                                                entries=resp.snapshots.clone()
                                                current_snapshot_id=current_snapshot_id_for_timeline()
                                                selected_past_id=selected_past_id.get()
                                                on_select=on_timeline_select
                                            />
                                        }.into_any()
                                    }
                                }
                            }}
                        </div>

                        // HUD region (Snapshot Comparison Cards — shown below chart)
                        <div class="analysis-hud-area">
                            {move || {
                                let past_id = selected_past_id.get()?;
                                let past_entry = find_entry(past_id)?;
                                let current_id = current_snapshot_id_for_timeline()?;
                                let current_entry = find_entry(current_id)?;
                                let delta = find_delta(past_id);

                                // Chart image URL for past analysis
                                let chart_url = Some(format!("/api/v1/snapshots/{}/chart-image", past_id));

                                Some(view! {
                                    <SnapshotComparison
                                        current=current_entry
                                        past=past_entry
                                        delta=delta
                                        past_chart_image_url=chart_url
                                    />
                                })
                            }}
                        </div>
                    </div>
                }
            })}
        </ErrorBoundary>
    }
}
