//! Comparison page — ranked grid view at `/compare`.
//!
//! Displays multiple analysis snapshots side-by-side as sortable Compact
//! Analysis Cards. Supports three entry paths:
//! - `?snapshot_ids=1,2,3` — pinned versions from Library "Compare Selected"
//! - `?ticker_ids=1,2,3`  — latest snapshots via ad-hoc compare
//! - `?id=5`              — saved comparison set
//!
//! Default sort: upside/downside ratio descending (NAIC 3-to-1 rule).
//!
//! Currency handling (Story 8.3): monetary values (current price, target
//! high/low) arrive in their native currency. The page fetches exchange rates
//! once on load and applies client-side conversion via `steady-invest-logic`.

use crate::components::compact_analysis_card::{CompactAnalysisCard, CompactCardData};
use crate::state::use_currency_preference;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use steady_invest_logic::{
    compute_upside_downside_from_snapshot, convert_monetary_value, extract_snapshot_prices,
    AnalysisSnapshot,
};

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
    native_currency: Option<String>,
    current_price: Option<f64>,
    target_high_price: Option<f64>,
    target_low_price: Option<f64>,
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

/// A single exchange rate pair from `GET /api/v1/exchange-rates`.
#[derive(Debug, Clone, Deserialize)]
struct ExchangeRatePair {
    from_currency: String,
    to_currency: String,
    rate: f64,
}

/// Exchange rate response from `GET /api/v1/exchange-rates`.
#[derive(Debug, Clone, Deserialize)]
struct ExchangeRateResponse {
    rates: Vec<ExchangeRatePair>,
    #[allow(dead_code)]
    rates_as_of: String,
    #[allow(dead_code)]
    stale: bool,
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
    native_currency: Option<String>,
    current_price: Option<f64>,
    target_high_price: Option<f64>,
    target_low_price: Option<f64>,
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
            native_currency: s.native_currency,
            current_price: s.current_price,
            target_high_price: s.target_high_price,
            target_low_price: s.target_low_price,
        }
    }
}

/// Build a ComparisonEntry from a full snapshot response by extracting fields
/// from snapshot_data and computing upside/downside ratio client-side.
fn entry_from_full_snapshot(resp: SnapshotFullResponse) -> ComparisonEntry {
    let snapshot: Option<AnalysisSnapshot> =
        serde_json::from_value(resp.snapshot_data.clone()).ok();

    if let Some(ref snap) = snapshot {
        let zone = resp
            .snapshot_data
            .get("valuation_zone")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        let ud = compute_upside_downside_from_snapshot(snap);
        let prices = extract_snapshot_prices(snap);

        ComparisonEntry {
            id: resp.id,
            ticker_symbol: snap.historical_data.ticker.clone(),
            captured_at: resp.captured_at.chars().take(10).collect(),
            thesis_locked: resp.thesis_locked,
            projected_sales_cagr: Some(snap.projected_sales_cagr),
            projected_eps_cagr: Some(snap.projected_eps_cagr),
            projected_high_pe: Some(snap.projected_high_pe),
            projected_low_pe: Some(snap.projected_low_pe),
            valuation_zone: zone,
            upside_downside_ratio: ud,
            native_currency: Some(snap.historical_data.currency.clone()),
            current_price: prices.current_price,
            target_high_price: prices.target_high_price,
            target_low_price: prices.target_low_price,
        }
    } else {
        ComparisonEntry {
            id: resp.id,
            ticker_symbol: format!("Ticker:{}", resp.ticker_id),
            captured_at: resp.captured_at.chars().take(10).collect(),
            thesis_locked: resp.thesis_locked,
            projected_sales_cagr: resp
                .snapshot_data
                .get("projected_sales_cagr")
                .and_then(|v| v.as_f64()),
            projected_eps_cagr: resp
                .snapshot_data
                .get("projected_eps_cagr")
                .and_then(|v| v.as_f64()),
            projected_high_pe: resp
                .snapshot_data
                .get("projected_high_pe")
                .and_then(|v| v.as_f64()),
            projected_low_pe: resp
                .snapshot_data
                .get("projected_low_pe")
                .and_then(|v| v.as_f64()),
            valuation_zone: None,
            upside_downside_ratio: None,
            native_currency: None,
            current_price: None,
            target_high_price: None,
            target_low_price: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Exchange rate helpers
// ---------------------------------------------------------------------------

/// Look up a directional exchange rate from the fetched rate list.
fn find_rate(rates: &[ExchangeRatePair], from: &str, to: &str) -> Option<f64> {
    rates
        .iter()
        .find(|r| r.from_currency == from && r.to_currency == to)
        .map(|r| r.rate)
}

/// Convert a monetary value from `native` to `target` currency using rates.
/// Falls back to the native value when rates are unavailable (AC#5).
fn convert_price(
    value: Option<f64>,
    native: Option<&str>,
    target: &str,
    rates: Option<&[ExchangeRatePair]>,
) -> Option<f64> {
    let val = value?;
    let nat = native?;
    if nat == target {
        return Some(val);
    }
    let Some(rate_list) = rates else {
        return Some(val);
    };
    let Some(rate) = find_rate(rate_list, nat, target) else {
        return Some(val);
    };
    Some(convert_monetary_value(val, rate))
}

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

/// Fetched comparison data: entries, optional set name, optional saved base currency.
#[derive(Clone)]
struct FetchedData {
    entries: Vec<ComparisonEntry>,
    set_name: Option<String>,
    saved_base_currency: Option<String>,
}

async fn fetch_comparison_data(search: &str) -> Result<FetchedData, String> {
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
        return Ok(FetchedData {
            entries: detail
                .items
                .into_iter()
                .map(|item| ComparisonEntry::from(item.snapshot))
                .collect(),
            set_name: Some(detail.name),
            saved_base_currency: Some(detail.base_currency),
        });
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
                let full: SnapshotFullResponse =
                    resp.json().await.map_err(|e| e.to_string())?;
                entries.push(entry_from_full_snapshot(full));
            }
            // Silently skip 404s (deleted snapshots)
        }
        return Ok(FetchedData {
            entries,
            set_name: None,
            saved_base_currency: None,
        });
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
        return Ok(FetchedData {
            entries: body
                .snapshots
                .into_iter()
                .map(ComparisonEntry::from)
                .collect(),
            set_name: None,
            saved_base_currency: None,
        });
    }

    // No params — empty state
    Ok(FetchedData {
        entries: Vec::new(),
        set_name: None,
        saved_base_currency: None,
    })
}

/// Fetch exchange rates from the backend. Returns None on error/503.
async fn fetch_exchange_rates() -> Option<ExchangeRateResponse> {
    let resp = gloo_net::http::Request::get("/api/v1/exchange-rates")
        .send()
        .await
        .ok()?;
    if !resp.ok() {
        return None;
    }
    resp.json().await.ok()
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

    // Currency: per-session override initialized from global preference.
    let global_currency = use_currency_preference();
    let active_currency = RwSignal::new(global_currency.get_untracked());

    // Exchange rates — fetched once on page load.
    let rates_resource = LocalResource::new(move || async move { fetch_exchange_rates().await });

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

    // When data loads from a saved set, initialize active_currency from it.
    Effect::new(move || {
        if let Some(Ok(fetched)) = data.get() {
            if let Some(ref currency) = fetched.saved_base_currency {
                active_currency.set(currency.clone());
            }
        }
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

            // Save/Load/Currency toolbar
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

                // Currency dropdown (per-session override)
                <select
                    class="currency-select"
                    on:change=move |ev| {
                        active_currency.set(event_target_value(&ev));
                    }
                >
                    <option value="CHF" selected=move || active_currency.get() == "CHF">"CHF"</option>
                    <option value="EUR" selected=move || active_currency.get() == "EUR">"EUR"</option>
                    <option value="USD" selected=move || active_currency.get() == "USD">"USD"</option>
                </select>
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
                        let currency = active_currency.get();
                        let req = CreateComparisonRequest {
                            name,
                            base_currency: currency,
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
                            Ok(fetched) => {
                                let entries = fetched.entries;
                                let set_name = fetched.set_name;
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

                                // Currency conversion context
                                let currency = active_currency.get();
                                let rate_list: Option<Vec<ExchangeRatePair>> = rates_resource
                                    .get()
                                    .and_then(|opt| opt)
                                    .map(|er| er.rates);
                                let rates_available = rate_list.is_some();
                                let has_mixed_currencies = sorted.iter().any(|e| {
                                    e.native_currency.as_deref().is_some_and(|nc| nc != currency.as_str())
                                });

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

                                // Fallback notice when rates unavailable but mixed currencies present
                                let rate_notice = if !rates_available && has_mixed_currencies {
                                    Some(view! {
                                        <div class="rate-notice">
                                            "Exchange rates unavailable \u{2014} values shown in original currencies"
                                        </div>
                                    })
                                } else {
                                    None
                                };

                                // Currency indicator — only show when all values are
                                // actually in the target currency (converted or same native)
                                let has_prices = sorted.iter().any(|e| e.current_price.is_some());
                                let currency_label = if has_prices && (rates_available || !has_mixed_currencies) {
                                    let c = currency.clone();
                                    Some(view! {
                                        <div class="currency-indicator">
                                            {format!("\u{00B7} Values in {}", c)}
                                        </div>
                                    })
                                } else {
                                    None
                                };

                                // Render cards with currency conversion
                                let cards = sorted.iter().map(|entry| {
                                    let rates_slice = rate_list.as_deref();
                                    let native = entry.native_currency.as_deref();

                                    // AC#5: when rates unavailable and currency differs,
                                    // show native currency prefix instead of target
                                    let entry_display_currency = if rates_available || native == Some(currency.as_str()) {
                                        Some(currency.clone())
                                    } else {
                                        entry.native_currency.clone()
                                    };

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
                                        current_price: convert_price(entry.current_price, native, &currency, rates_slice),
                                        target_high_price: convert_price(entry.target_high_price, native, &currency, rates_slice),
                                        target_low_price: convert_price(entry.target_low_price, native, &currency, rates_slice),
                                        display_currency: entry_display_currency,
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
                                    {rate_notice}
                                    {currency_label}
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
