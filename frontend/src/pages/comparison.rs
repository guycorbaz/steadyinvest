//! Comparison page — ranked grid view at `/compare`.
//!
//! Displays multiple analysis snapshots side-by-side as sortable Compact
//! Analysis Cards. Supports three entry paths:
//! - `?snapshot_ids=1,2,3` — pinned versions from Library "Compare Selected"
//! - `?ticker_ids=1,2,3`  — latest snapshots via ad-hoc compare
//! - `?id=5`              — saved comparison set
//!
//! Default sort: upside/downside ratio descending (NAIC 3-to-1 rule).

use crate::components::compact_analysis_card::{CompactAnalysisCard, CompactCardData};
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use steady_invest_logic::{compute_upside_downside_from_snapshot, AnalysisSnapshot};

// ---------------------------------------------------------------------------
// DTOs — matching backend API responses
// ---------------------------------------------------------------------------

/// Full snapshot response from `GET /api/v1/snapshots/{id}`.
#[derive(Debug, Clone, Deserialize)]
struct SnapshotFullResponse {
    id: i32,
    #[allow(dead_code)]
    user_id: i32,
    ticker_id: i32,
    snapshot_data: serde_json::Value,
    thesis_locked: bool,
    #[allow(dead_code)]
    notes: Option<String>,
    captured_at: String,
}

/// Snapshot summary from comparison API endpoints.
#[derive(Debug, Clone, Deserialize)]
struct ComparisonSnapshotSummary {
    id: i32,
    #[allow(dead_code)]
    ticker_id: i32,
    ticker_symbol: String,
    thesis_locked: bool,
    captured_at: String,
    #[allow(dead_code)]
    notes: Option<String>,
    projected_sales_cagr: Option<f64>,
    projected_eps_cagr: Option<f64>,
    projected_high_pe: Option<f64>,
    projected_low_pe: Option<f64>,
    valuation_zone: Option<String>,
    upside_downside_ratio: Option<f64>,
}

/// Ad-hoc compare response from `GET /api/v1/compare`.
#[derive(Debug, Clone, Deserialize)]
struct AdHocCompareResponse {
    #[allow(dead_code)]
    base_currency: Option<String>,
    snapshots: Vec<ComparisonSnapshotSummary>,
}

/// Item in a saved comparison set detail.
#[derive(Debug, Clone, Deserialize)]
struct ComparisonSetItemDetail {
    #[allow(dead_code)]
    id: i32,
    #[allow(dead_code)]
    sort_order: i32,
    snapshot: ComparisonSnapshotSummary,
}

/// Saved comparison set detail from `GET /api/v1/comparisons/{id}`.
#[derive(Debug, Clone, Deserialize)]
struct ComparisonSetDetail {
    #[allow(dead_code)]
    id: i32,
    name: String,
    #[allow(dead_code)]
    base_currency: String,
    #[allow(dead_code)]
    created_at: String,
    #[allow(dead_code)]
    updated_at: String,
    items: Vec<ComparisonSetItemDetail>,
}

/// Request body for saving a comparison set.
#[derive(Debug, Clone, Serialize)]
struct CreateComparisonRequest {
    name: String,
    base_currency: String,
    items: Vec<CreateComparisonItem>,
}

/// A single item in a save request.
#[derive(Debug, Clone, Serialize)]
struct CreateComparisonItem {
    analysis_snapshot_id: i32,
    sort_order: i32,
}

/// Summary of saved comparison sets for the load dropdown.
#[derive(Debug, Clone, Deserialize)]
struct ComparisonSetSummary {
    id: i32,
    name: String,
    #[allow(dead_code)]
    base_currency: String,
    item_count: i64,
    #[allow(dead_code)]
    created_at: String,
}

// ---------------------------------------------------------------------------
// Internal unified entry type
// ---------------------------------------------------------------------------

/// Unified entry for all comparison paths (snapshot_ids, ticker_ids, saved set).
#[derive(Debug, Clone)]
struct ComparisonEntry {
    id: i32,
    ticker_symbol: String,
    captured_at: String,
    thesis_locked: bool,
    projected_sales_cagr: Option<f64>,
    projected_eps_cagr: Option<f64>,
    projected_high_pe: Option<f64>,
    projected_low_pe: Option<f64>,
    valuation_zone: Option<String>,
    upside_downside_ratio: Option<f64>,
}

impl From<ComparisonSnapshotSummary> for ComparisonEntry {
    fn from(s: ComparisonSnapshotSummary) -> Self {
        Self {
            id: s.id,
            ticker_symbol: s.ticker_symbol,
            captured_at: s.captured_at.chars().take(10).collect(),
            thesis_locked: s.thesis_locked,
            projected_sales_cagr: s.projected_sales_cagr,
            projected_eps_cagr: s.projected_eps_cagr,
            projected_high_pe: s.projected_high_pe,
            projected_low_pe: s.projected_low_pe,
            valuation_zone: s.valuation_zone,
            upside_downside_ratio: s.upside_downside_ratio,
        }
    }
}

/// Build a ComparisonEntry from a full snapshot response by extracting fields
/// from snapshot_data and computing upside/downside ratio client-side.
fn entry_from_full_snapshot(resp: SnapshotFullResponse) -> ComparisonEntry {
    let snapshot: Option<AnalysisSnapshot> =
        serde_json::from_value(resp.snapshot_data.clone()).ok();

    let (ticker_symbol, sales_cagr, eps_cagr, high_pe, low_pe, zone, ud_ratio) =
        if let Some(ref snap) = snapshot {
            let sym = snap.historical_data.ticker.clone();
            let zone = resp
                .snapshot_data
                .get("valuation_zone")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned());

            // Compute upside/downside ratio via shared logic (Cardinal Rule)
            let ud = compute_upside_downside_from_snapshot(snap);

            (
                sym,
                Some(snap.projected_sales_cagr),
                Some(snap.projected_eps_cagr),
                Some(snap.projected_high_pe),
                Some(snap.projected_low_pe),
                zone,
                ud,
            )
        } else {
            (
                format!("Ticker:{}", resp.ticker_id),
                resp.snapshot_data
                    .get("projected_sales_cagr")
                    .and_then(|v| v.as_f64()),
                resp.snapshot_data
                    .get("projected_eps_cagr")
                    .and_then(|v| v.as_f64()),
                resp.snapshot_data
                    .get("projected_high_pe")
                    .and_then(|v| v.as_f64()),
                resp.snapshot_data
                    .get("projected_low_pe")
                    .and_then(|v| v.as_f64()),
                None,
                None,
            )
        };

    ComparisonEntry {
        id: resp.id,
        ticker_symbol,
        captured_at: resp.captured_at.chars().take(10).collect(),
        thesis_locked: resp.thesis_locked,
        projected_sales_cagr: sales_cagr,
        projected_eps_cagr: eps_cagr,
        projected_high_pe: high_pe,
        projected_low_pe: low_pe,
        valuation_zone: zone,
        upside_downside_ratio: ud_ratio,
    }
}

// Client-side U/D ratio delegates to steady-invest-logic (Cardinal Rule).

// ---------------------------------------------------------------------------
// Sorting
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum SortColumn {
    Ticker,
    Date,
    SalesCagr,
    EpsCagr,
    HighPe,
    LowPe,
    ValuationZone,
    UpsideDownside,
}

fn sort_entries(entries: &mut [ComparisonEntry], col: &SortColumn, ascending: bool) {
    entries.sort_by(|a, b| {
        let cmp = match col {
            SortColumn::Ticker => a.ticker_symbol.cmp(&b.ticker_symbol),
            SortColumn::Date => a.captured_at.cmp(&b.captured_at),
            SortColumn::SalesCagr => cmp_opt_f64(a.projected_sales_cagr, b.projected_sales_cagr),
            SortColumn::EpsCagr => cmp_opt_f64(a.projected_eps_cagr, b.projected_eps_cagr),
            SortColumn::HighPe => cmp_opt_f64(a.projected_high_pe, b.projected_high_pe),
            SortColumn::LowPe => cmp_opt_f64(a.projected_low_pe, b.projected_low_pe),
            SortColumn::ValuationZone => {
                a.valuation_zone
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.valuation_zone.as_deref().unwrap_or(""))
            }
            SortColumn::UpsideDownside => {
                cmp_opt_f64(a.upside_downside_ratio, b.upside_downside_ratio)
            }
        };
        if ascending {
            cmp
        } else {
            cmp.reverse()
        }
    });
}

fn cmp_opt_f64(a: Option<f64>, b: Option<f64>) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

// ---------------------------------------------------------------------------
// URL param parsing
// ---------------------------------------------------------------------------

fn parse_query(search: &str) -> (Vec<i32>, Vec<i32>, Option<i32>) {
    let s = search.strip_prefix('?').unwrap_or(search);
    let mut snapshot_ids = Vec::new();
    let mut ticker_ids = Vec::new();
    let mut set_id = None;

    for pair in s.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            match k {
                "snapshot_ids" => {
                    snapshot_ids = v
                        .split(',')
                        .filter_map(|x| x.trim().parse::<i32>().ok())
                        .collect();
                }
                "ticker_ids" => {
                    ticker_ids = v
                        .split(',')
                        .filter_map(|x| x.trim().parse::<i32>().ok())
                        .collect();
                }
                "id" => {
                    set_id = v.trim().parse::<i32>().ok();
                }
                _ => {}
            }
        }
    }
    (snapshot_ids, ticker_ids, set_id)
}

// ---------------------------------------------------------------------------
// Data fetching
// ---------------------------------------------------------------------------

async fn fetch_comparison_data(
    search: &str,
) -> Result<(Vec<ComparisonEntry>, Option<String>), String> {
    let (snapshot_ids, ticker_ids, set_id) = parse_query(search);

    // Path 3: saved comparison set
    if let Some(id) = set_id {
        let url = format!("/api/v1/comparisons/{}", id);
        let resp = gloo_net::http::Request::get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.ok() {
            return Err(format!("Failed to load comparison set: {}", resp.status()));
        }
        let detail: ComparisonSetDetail = resp.json().await.map_err(|e| e.to_string())?;
        let name = Some(detail.name);
        let entries = detail
            .items
            .into_iter()
            .map(|item| ComparisonEntry::from(item.snapshot))
            .collect();
        return Ok((entries, name));
    }

    // Path 1: explicit snapshot IDs (from Library "Compare Selected")
    if !snapshot_ids.is_empty() {
        let mut entries = Vec::with_capacity(snapshot_ids.len());
        for sid in &snapshot_ids {
            let url = format!("/api/v1/snapshots/{}", sid);
            let resp = gloo_net::http::Request::get(&url)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if resp.ok() {
                let full: SnapshotFullResponse = resp.json().await.map_err(|e| e.to_string())?;
                entries.push(entry_from_full_snapshot(full));
            }
            // Silently skip 404s (deleted snapshots)
        }
        return Ok((entries, None));
    }

    // Path 2: ticker IDs (ad-hoc latest)
    if !ticker_ids.is_empty() {
        let ids_str = ticker_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let url = format!("/api/v1/compare?ticker_ids={}", ids_str);
        let resp = gloo_net::http::Request::get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.ok() {
            return Err(format!("Failed to compare tickers: {}", resp.status()));
        }
        let body: AdHocCompareResponse = resp.json().await.map_err(|e| e.to_string())?;
        let entries = body.snapshots.into_iter().map(ComparisonEntry::from).collect();
        return Ok((entries, None));
    }

    // No params — empty state
    Ok((Vec::new(), None))
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// Comparison page at `/compare`.
#[component]
pub fn Comparison() -> impl IntoView {
    let location = use_location();
    let (sort_col, set_sort_col) = signal(SortColumn::UpsideDownside);
    let (sort_asc, set_sort_asc) = signal(false); // default descending

    // Save comparison state
    let (show_save_form, set_show_save_form) = signal(false);
    let (save_name, set_save_name) = signal(String::new());
    let (save_feedback, set_save_feedback) = signal(Option::<String>::None);

    // Resolved snapshot IDs from fetched data — used by save flow.
    // Populated when entries render, so it works for all three entry paths.
    let resolved_ids = RwSignal::new(Vec::<i32>::new());

    // Load saved comparisons
    let saved_sets = LocalResource::new(move || async move {
        let resp = gloo_net::http::Request::get("/api/v1/comparisons")
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.ok() {
            resp.json::<Vec<ComparisonSetSummary>>()
                .await
                .map_err(|e| e.to_string())
        } else {
            Err(format!("Failed to load saved: {}", resp.status()))
        }
    });

    let data = LocalResource::new(move || {
        let search = location.search.get();
        async move { fetch_comparison_data(&search).await }
    });

    let navigate = leptos_router::hooks::use_navigate();
    let on_card_click = Callback::new(move |id: i32| {
        navigate(&format!("/?snapshot={}", id), Default::default());
    });

    let toggle_sort = move |col: SortColumn| {
        let current = sort_col.get();
        if current == col {
            set_sort_asc.update(|a| *a = !*a);
        } else {
            set_sort_col.set(col);
            set_sort_asc.set(false); // default descending for new column
        }
    };

    let sort_indicator = move |col: &SortColumn| -> &'static str {
        if sort_col.get() == *col {
            if sort_asc.get() {
                "\u{25B2}" // ▲
            } else {
                "\u{25BC}" // ▼
            }
        } else {
            ""
        }
    };

    view! {
        <div class="comparison-page">
            <h1 class="comparison-title">"Comparison"</h1>

            // Save/Load toolbar
            <div class="comparison-toolbar">
                <button
                    class="toolbar-btn"
                    on:click=move |_| set_show_save_form.update(|v| *v = !*v)
                >
                    "Save"
                </button>

                // Load saved dropdown
                {move || {
                    saved_sets.get().map(|result| {
                        match result {
                            Ok(sets) if !sets.is_empty() => {
                                let on_load = move |ev: leptos::ev::Event| {
                                    let val = event_target_value(&ev);
                                    if let Ok(id) = val.parse::<i32>() {
                                        let nav = leptos_router::hooks::use_navigate();
                                        nav(&format!("/compare?id={}", id), Default::default());
                                    }
                                };
                                view! {
                                    <select class="toolbar-select" on:change=on_load>
                                        <option value="" selected=true>"Load Saved..."</option>
                                        {sets.iter().map(|s| {
                                            let val = s.id.to_string();
                                            let label = format!("{} ({})", s.name, s.item_count);
                                            view! { <option value=val>{label}</option> }
                                        }).collect_view()}
                                    </select>
                                }.into_any()
                            }
                            _ => view! {}.into_any(),
                        }
                    })
                }}
            </div>

            // Save form (inline, shown when Save clicked)
            {move || {
                if show_save_form.get() {
                    let on_save_submit = move |ev: web_sys::SubmitEvent| {
                        ev.prevent_default();
                        let name = save_name.get();
                        if name.trim().is_empty() {
                            set_save_feedback.set(Some("Name is required".to_string()));
                            return;
                        }
                        // Use resolved snapshot IDs from fetched data (not URL params)
                        let entry_ids = resolved_ids.get();
                        if entry_ids.is_empty() {
                            set_save_feedback.set(Some("No analyses to save".to_string()));
                            return;
                        }
                        let req = CreateComparisonRequest {
                            name,
                            base_currency: "USD".to_string(),
                            items: entry_ids.iter().enumerate().map(|(i, id)| {
                                CreateComparisonItem {
                                    analysis_snapshot_id: *id,
                                    sort_order: i as i32,
                                }
                            }).collect(),
                        };
                        wasm_bindgen_futures::spawn_local(async move {
                            let body = serde_json::to_string(&req).unwrap_or_default();
                            let result = gloo_net::http::Request::post("/api/v1/comparisons")
                                .header("Content-Type", "application/json")
                                .body(&body)
                                .unwrap()
                                .send()
                                .await;
                            match result {
                                Ok(resp) if resp.ok() => {
                                    set_save_feedback.set(Some("Saved!".to_string()));
                                    set_show_save_form.set(false);
                                    set_save_name.set(String::new());
                                }
                                Ok(resp) => {
                                    set_save_feedback.set(Some(format!("Error: {}", resp.status())));
                                }
                                Err(e) => {
                                    set_save_feedback.set(Some(format!("Error: {}", e)));
                                }
                            }
                        });
                    };
                    view! {
                        <form class="save-comparison-form" on:submit=on_save_submit>
                            <input
                                type="text"
                                class="save-name-input"
                                placeholder="Comparison name..."
                                on:input=move |ev| set_save_name.set(event_target_value(&ev))
                                prop:value=save_name
                            />
                            <button type="submit" class="save-submit-btn">"Save"</button>
                            <button type="button" class="save-cancel-btn"
                                on:click=move |_| set_show_save_form.set(false)>"Cancel"</button>
                        </form>
                    }.into_any()
                } else {
                    view! {}.into_any()
                }
            }}

            // Save feedback
            {move || {
                save_feedback.get().map(|msg| view! {
                    <div class="save-feedback">{msg}</div>
                })
            }}

            <Suspense fallback=|| view! {
                <div class="loading-overlay">
                    <div class="pulse"></div>
                    <div class="status-text">"Loading comparison..."</div>
                </div>
            }>
                {move || {
                    data.get().map(|result| {
                        match result {
                            Ok((entries, set_name)) => {
                                if entries.is_empty() {
                                    // Empty state
                                    return view! {
                                        <div class="comparison-empty">
                                            <h2>"No analyses to compare"</h2>
                                            <p>"Select analyses from the Library to compare them side by side."</p>
                                            <A href="/library" attr:class="comparison-cta">
                                                "Go to Library \u{2192}"
                                            </A>
                                        </div>
                                    }.into_any();
                                }

                                // Show set name if loaded from saved comparison
                                let name_view = set_name.map(|n| view! {
                                    <div class="comparison-set-name">{n}</div>
                                });

                                // Sort entries
                                let col = sort_col.get();
                                let asc = sort_asc.get();
                                let mut sorted = entries.clone();
                                sort_entries(&mut sorted, &col, asc);

                                // Update resolved IDs for save flow (works for all 3 paths)
                                resolved_ids.set(sorted.iter().map(|e| e.id).collect());

                                // Sort headers (desktop)
                                let sort_headers = {
                                    let ts = toggle_sort.clone();
                                    view! {
                                        <div class="comparison-sort-headers">
                                            <button class="sort-header" on:click=move |_| ts(SortColumn::Ticker)>
                                                "Ticker " {sort_indicator(&SortColumn::Ticker)}
                                            </button>
                                            <button class="sort-header" on:click=move |_| ts(SortColumn::SalesCagr)>
                                                "Sales CAGR " {sort_indicator(&SortColumn::SalesCagr)}
                                            </button>
                                            <button class="sort-header" on:click=move |_| ts(SortColumn::EpsCagr)>
                                                "EPS CAGR " {sort_indicator(&SortColumn::EpsCagr)}
                                            </button>
                                            <button class="sort-header" on:click=move |_| ts(SortColumn::UpsideDownside)>
                                                "U/D Ratio " {sort_indicator(&SortColumn::UpsideDownside)}
                                            </button>
                                        </div>
                                    }
                                };

                                // Mobile sort dropdown
                                let mobile_sort = {
                                    let on_sort_change = move |ev: leptos::ev::Event| {
                                        let val = event_target_value(&ev);
                                        let col = match val.as_str() {
                                            "ticker" => SortColumn::Ticker,
                                            "date" => SortColumn::Date,
                                            "sales_cagr" => SortColumn::SalesCagr,
                                            "eps_cagr" => SortColumn::EpsCagr,
                                            "high_pe" => SortColumn::HighPe,
                                            "low_pe" => SortColumn::LowPe,
                                            "zone" => SortColumn::ValuationZone,
                                            _ => SortColumn::UpsideDownside,
                                        };
                                        set_sort_col.set(col);
                                    };
                                    let on_dir_toggle = move |_| {
                                        set_sort_asc.update(|a| *a = !*a);
                                    };
                                    view! {
                                        <div class="sort-dropdown-mobile">
                                            <select on:change=on_sort_change>
                                                <option value="upside_downside" selected=true>"U/D Ratio"</option>
                                                <option value="ticker">"Ticker"</option>
                                                <option value="date">"Date"</option>
                                                <option value="sales_cagr">"Sales CAGR"</option>
                                                <option value="eps_cagr">"EPS CAGR"</option>
                                                <option value="high_pe">"High P/E"</option>
                                                <option value="low_pe">"Low P/E"</option>
                                                <option value="zone">"Zone"</option>
                                            </select>
                                            <button class="sort-dir-btn" on:click=on_dir_toggle>
                                                {move || if sort_asc.get() { "\u{25B2}" } else { "\u{25BC}" }}
                                            </button>
                                        </div>
                                    }
                                };

                                // Render cards
                                let cards = sorted.iter().map(|entry| {
                                    let data = CompactCardData {
                                        id: entry.id,
                                        ticker_symbol: entry.ticker_symbol.clone(),
                                        captured_at: entry.captured_at.clone(),
                                        thesis_locked: entry.thesis_locked,
                                        projected_sales_cagr: entry.projected_sales_cagr,
                                        projected_eps_cagr: entry.projected_eps_cagr,
                                        projected_high_pe: entry.projected_high_pe,
                                        projected_low_pe: entry.projected_low_pe,
                                        valuation_zone: entry.valuation_zone.clone(),
                                        upside_downside_ratio: entry.upside_downside_ratio,
                                    };
                                    view! {
                                        <CompactAnalysisCard
                                            data=data
                                            on_click=on_card_click
                                        />
                                    }
                                }).collect_view();

                                view! {
                                    {name_view}
                                    <div class="comparison-controls">
                                        {sort_headers}
                                        {mobile_sort}
                                    </div>
                                    <div class="comparison-grid">
                                        {cards}
                                    </div>
                                }.into_any()
                            }
                            Err(e) => {
                                view! {
                                    <div class="comparison-empty">
                                        <p>"Error loading comparison: " {e}</p>
                                        <A href="/library" attr:class="comparison-cta">
                                            "Go to Library \u{2192}"
                                        </A>
                                    </div>
                                }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
