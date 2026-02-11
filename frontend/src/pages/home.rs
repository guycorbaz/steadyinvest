//! Home page — main analysis workspace (`/`).
//!
//! Coordinates ticker selection, data harvesting, snapshot management, and
//! file import, then delegates rendering to [`AnalystHUD`] or [`SnapshotHUD`].

use crate::components::search_bar::SearchBar;
use crate::components::analyst_hud::AnalystHUD;
use crate::components::snapshot_hud::SnapshotHUD;
use crate::types::LockedAnalysisModel;
use crate::ActiveLockedAnalysisId;
use leptos::prelude::*;
use naic_logic::{HistoricalData, TickerInfo, AnalysisSnapshot};

/// Main analysis page rendered at `/`.
///
/// Manages ticker selection state, fetches historical data and locked snapshots,
/// and renders either the live analysis HUD or a read-only snapshot view.
#[component]
pub fn Home() -> impl IntoView {
    let (selected_ticker, set_selected_ticker) = signal(Option::<TickerInfo>::None);
    let (target_currency, set_target_currency) = signal("USD".to_string());
    let (selected_snapshot_id, set_selected_snapshot_id) = signal(Option::<i32>::None);
    let (imported_snapshot, set_imported_snapshot) = signal(Option::<AnalysisSnapshot>::None);

    // Sync the active locked analysis ID to app-level context so the
    // Command Strip can enable/disable the PDF export action.
    let locked_ctx = use_context::<ActiveLockedAnalysisId>();
    Effect::new(move |_| {
        if let Some(ctx) = locked_ctx {
            let snap_id = selected_snapshot_id.get();
            let has_import = imported_snapshot.get().is_some();
            // Only expose a real DB snapshot ID (imported snapshots use id=0)
            ctx.0.set(if !has_import { snap_id } else { None });
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
                } 
                on_import=move |snapshot| {
                    set_imported_snapshot.set(Some(snapshot));
                    set_selected_ticker.set(None);
                }
            />

            {move || selected_ticker.get().map(|ticker| {
                view! {
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
                                <div class="view-selector">
                                    <label>"View: "</label>
                                    <select on:change=move |ev| {
                                        let val = event_target_value(&ev);
                                        if val == "live" {
                                            set_selected_snapshot_id.set(None);
                                        } else {
                                            set_selected_snapshot_id.set(val.parse::<i32>().ok());
                                        }
                                    }>
                                        <option value="live" selected={move || selected_snapshot_id.get().is_none()}>"Live Analysis"</option>
                                        <Suspense fallback=|| view! { <option disabled=true>"Loading snapshots..."</option> }>
                                            {move || snapshots.get().map(|res: Result<Vec<LockedAnalysisModel>, String>| {
                                                match res {
                                                    Ok(list) => {
                                                        list.iter().map(|s| {
                                                            let id = s.id;
                                                            let date = s.created_at.format("%Y-%m-%d %H:%M").to_string();
                                                            view! {
                                                                <option 
                                                                    value=id.to_string()
                                                                    selected={move || selected_snapshot_id.get() == Some(id)}
                                                                >
                                                                    {format!("Snapshot: {}", date)}
                                                                </option>
                                                            }
                                                        }).collect_view().into_any()
                                                    }
                                                    _ => view! { <option disabled=true>"No snapshots"</option> }.into_any()
                                                }
                                            })}
                                        </Suspense>
                                    </select>
                                </div>
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
                                    // Wrap in a temporary model for the HUD
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
                }
            })}
        </ErrorBoundary>
    }
}
