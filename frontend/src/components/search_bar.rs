use leptos::prelude::*;
use naic_logic::TickerInfo;
use gloo_net::http::Request;

#[component]
pub fn SearchBar<F>(on_select: F) -> impl IntoView
where
    F: Fn(TickerInfo) + Send + Sync + Clone + 'static,
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
                    view! { <div /> }.into_any()
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
