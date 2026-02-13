//! Ticker search bar with autocomplete dropdown.
//!
//! Provides a text input that queries `/api/tickers/search` as the user types
//! (minimum 2 characters). Also includes an "Open from file" button for
//! importing previously saved analysis snapshots.

use leptos::prelude::*;
use steady_invest_logic::TickerInfo;
use gloo_net::http::Request;

/// Ticker search input with autocomplete results.
///
/// # Props
///
/// * `on_select` — Called with the chosen [`TickerInfo`] when a result is clicked.
/// * `on_import` — Called with a deserialized snapshot when a file is imported.
#[component]
pub fn SearchBar<F, G>(
    on_select: F,
    on_import: G,
) -> impl IntoView
where
    F: Fn(TickerInfo) + Send + Sync + Clone + 'static,
    G: Fn(steady_invest_logic::AnalysisSnapshot) + Send + Sync + Clone + 'static,
{
    let (query, set_query) = signal(String::new());
    let (is_expanded, set_is_expanded) = signal(false);

    let search_results = LocalResource::new(move || {
        let q = query.get();
        async move {
            if q.len() < 2 {
                return Vec::new();
            }
            let url = format!("/api/tickers/search?q={}", q);
            Request::get(&url)
                .send()
                .await
                .unwrap()
                .json::<Vec<TickerInfo>>()
                .await
                .unwrap_or_default()
        }
    });

    view! {
        <div class="search-container" class:expanded=move || is_expanded.get() || !query.get().is_empty()>
            <div class="search-wrapper">
                <input
                    type="text"
                    placeholder="Search Ticker (e.g. NESN.SW, AAPL)..."
                    prop:value=move || query.get()
                    on:input=move |ev| {
                        set_query.set(event_target_value(&ev));
                    }
                    class="zen-search-input"
                    autofocus
                />
                {move || if !query.get().is_empty() {
                    view! {
                        <button class="clear-btn" on:click=move |_| {
                            set_query.set(String::new());
                            set_is_expanded.set(false);
                        }>"×"</button>
                    }.into_any()
                } else {
                    view! {
                        <button 
                            class="open-analysis-btn" 
                            title="Open Analysis from File"
                            on:click={
                                let on_import = on_import.clone();
                                move |_| {
                                    let on_import = on_import.clone();
                                    let _ = crate::persistence::trigger_import(Callback::new(move |snapshot| {
                                        on_import(snapshot);
                                    }));
                                }
                            }
                        >
                            <span class="btn-icon">"📂"</span>
                        </button>
                    }.into_any()
                }}
            </div>

            <div class="autocomplete-results">
                <Suspense fallback=|| view! { <div class="searching-indicator">"Querying Terminal..."</div> }>
                    {move || {
                        search_results.get().map(|results| {
                            if results.is_empty() && query.get().len() >= 2 {
                                view! { <div class="no-results">"No matching instruments found."</div> }.into_any()
                            } else {
                                results.into_iter().map(|res| {
                                    let res_clone = res.clone();
                                    let on_select = on_select.clone();
                                    view! {
                                        <div class="result-item" on:click=move |_| {
                                            set_is_expanded.set(true);
                                            on_select(res_clone.clone());
                                            set_query.set(String::new());
                                        }>
                                            <span class="ticker-code">{res.ticker.clone()}</span>
                                            <span class="company-name">{res.name.clone()}</span>
                                            <span class="exchange-tag">{res.exchange.clone()}</span>
                                        </div>
                                    }
                                }).collect_view().into_any()
                            }
                        })
                    }}
                </Suspense>
            </div>
        </div>
    }
}
