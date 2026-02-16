//! Library page — browse all saved analysis snapshots (`/library`).
//!
//! Fetches snapshot summaries from the API, displays them as a responsive
//! grid of Compact Analysis Cards, and provides client-side filtering by
//! ticker symbol and locked/unlocked status.
//!
//! Supports a "Compare Selected" flow: users select multiple cards via
//! checkboxes, then navigate to the Comparison view with those snapshot IDs.

use crate::components::compact_analysis_card::{CompactAnalysisCard, CompactCardData};
use leptos::prelude::*;
use leptos_router::components::A;
use serde::Deserialize;

/// DTO matching the enhanced `SnapshotSummary` from the backend.
#[derive(Debug, Clone, Deserialize)]
struct SnapshotSummary {
    id: i32,
    #[allow(dead_code)]
    ticker_id: i32,
    ticker_symbol: String,
    thesis_locked: bool,
    #[allow(dead_code)]
    notes: Option<String>,
    captured_at: String,
    projected_sales_cagr: Option<f64>,
    projected_eps_cagr: Option<f64>,
    projected_high_pe: Option<f64>,
    projected_low_pe: Option<f64>,
}

/// Library page at `/library`.
#[component]
pub fn Library() -> impl IntoView {
    let (ticker_filter, set_ticker_filter) = signal(String::new());
    let (lock_filter, set_lock_filter) = signal("all".to_string());

    // Compare Selected: track (id, ticker_symbol) of selected cards
    let selected = RwSignal::new(Vec::<(i32, String)>::new());

    let snapshots = LocalResource::new(move || async move {
        let response = gloo_net::http::Request::get("/api/v1/snapshots")
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if response.ok() {
            response
                .json::<Vec<SnapshotSummary>>()
                .await
                .map_err(|e| e.to_string())
        } else {
            Err(format!("Failed to load snapshots: {}", response.status()))
        }
    });

    let navigate = leptos_router::hooks::use_navigate();

    let on_card_click = Callback::new(move |id: i32| {
        navigate(&format!("/?snapshot={}", id), Default::default());
    });

    view! {
        <div class="library-page">
            <h1 class="library-title">"Analysis Library"</h1>

            <div class="library-filters">
                <input
                    type="text"
                    class="filter-input"
                    placeholder="Search by ticker..."
                    on:input=move |ev| set_ticker_filter.set(event_target_value(&ev))
                    prop:value=ticker_filter
                />
                <select
                    class="filter-select"
                    on:change=move |ev| set_lock_filter.set(event_target_value(&ev))
                >
                    <option value="all" selected=true>"All"</option>
                    <option value="locked">"Locked only"</option>
                    <option value="unlocked">"Unlocked only"</option>
                </select>
            </div>

            <Suspense fallback=|| view! {
                <div class="loading-overlay">
                    <div class="pulse"></div>
                    <div class="status-text">"Loading Library..."</div>
                </div>
            }>
                {move || {
                    snapshots.get().map(|result| {
                        match result {
                            Ok(list) => {
                                if list.is_empty() {
                                    return view! {
                                        <div class="library-empty">
                                            <p>"No analyses saved yet."</p>
                                            <p class="hint">"Create an analysis from the Search page to get started."</p>
                                        </div>
                                    }.into_any();
                                }

                                let filter_text = ticker_filter.get().to_uppercase();
                                let lock_val = lock_filter.get();

                                let filtered: Vec<&SnapshotSummary> = list.iter().filter(|s| {
                                    // Ticker text filter
                                    let ticker_match = filter_text.is_empty()
                                        || s.ticker_symbol.to_uppercase().contains(&filter_text);
                                    // Lock status filter
                                    let lock_match = match lock_val.as_str() {
                                        "locked" => s.thesis_locked,
                                        "unlocked" => !s.thesis_locked,
                                        _ => true,
                                    };
                                    ticker_match && lock_match
                                }).collect();

                                if filtered.is_empty() {
                                    view! {
                                        <div class="library-empty">
                                            <p>"No analyses match your filters."</p>
                                            <p class="hint">"Try adjusting your search or filter settings."</p>
                                        </div>
                                    }.into_any()
                                } else {
                                    let cards = filtered.iter().map(|s| {
                                        let card_id = s.id;
                                        let ticker_sym = s.ticker_symbol.clone();
                                        let data = CompactCardData {
                                            id: s.id,
                                            ticker_symbol: s.ticker_symbol.clone(),
                                            captured_at: s.captured_at.chars().take(10).collect::<String>(),
                                            thesis_locked: s.thesis_locked,
                                            projected_sales_cagr: s.projected_sales_cagr,
                                            projected_eps_cagr: s.projected_eps_cagr,
                                            projected_high_pe: s.projected_high_pe,
                                            projected_low_pe: s.projected_low_pe,
                                            valuation_zone: None,
                                            upside_downside_ratio: None,
                                        };
                                        let is_selected = {
                                            let sel = selected;
                                            move || sel.get().iter().any(|(id, _)| *id == card_id)
                                        };
                                        let toggle_select = {
                                            let sym = ticker_sym.clone();
                                            move |_ev: leptos::ev::Event| {
                                                selected.update(|v| {
                                                    if let Some(pos) = v.iter().position(|(id, _)| *id == card_id) {
                                                        v.remove(pos);
                                                    } else {
                                                        v.push((card_id, sym.clone()));
                                                    }
                                                });
                                            }
                                        };
                                        view! {
                                            <div class="library-card-wrapper">
                                                <label class="compare-checkbox" on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()>
                                                    <input
                                                        type="checkbox"
                                                        prop:checked=is_selected
                                                        on:change=toggle_select
                                                    />
                                                    <span class="compare-checkbox-label">"Compare"</span>
                                                </label>
                                                <CompactAnalysisCard
                                                    data=data
                                                    on_click=on_card_click
                                                />
                                            </div>
                                        }
                                    }).collect_view();

                                    view! {
                                        <div class="library-grid">
                                            {cards}
                                        </div>
                                    }.into_any()
                                }
                            }
                            Err(e) => {
                                view! {
                                    <div class="library-empty">
                                        <p>"Error loading library: " {e}</p>
                                    </div>
                                }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>

            // Floating action bar for Compare Selected
            {move || {
                let sel = selected.get();
                let count = sel.len();
                if count >= 2 {
                    let ids_str = sel.iter()
                        .map(|(id, _)| id.to_string())
                        .collect::<Vec<_>>()
                        .join(",");
                    let symbols = sel.iter()
                        .map(|(_, s)| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let href = format!("/compare?snapshot_ids={}", ids_str);
                    view! {
                        <div class="compare-selection-bar">
                            <span class="compare-selection-info">{symbols}</span>
                            <A href=href attr:class="compare-btn">
                                {format!("Compare ({}) \u{2192}", count)}
                            </A>
                        </div>
                    }.into_any()
                } else if count == 1 {
                    view! {
                        <div class="compare-selection-bar compare-selection-bar--hint">
                            <span class="compare-selection-info">"Select one more to compare"</span>
                        </div>
                    }.into_any()
                } else {
                    view! {}.into_any()
                }
            }}
        </div>
    }
}
