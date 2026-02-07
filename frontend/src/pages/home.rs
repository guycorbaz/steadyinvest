use crate::components::search_bar::SearchBar;
use crate::components::ssg_chart::SSGChart;
use leptos::prelude::*;
use naic_logic::{HistoricalData, TickerInfo};

/// Default Home Page
#[component]
pub fn Home() -> impl IntoView {
    let (selected_ticker, set_selected_ticker) = signal(Option::<TickerInfo>::None);
    let (target_currency, set_target_currency) = signal("USD".to_string());

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
                    
                    // AC 5: Integrity Alert if data is incomplete
                    if !data.is_complete {
                        return Err("Integrity Alert: Data population incomplete for this ticker.".to_string());
                    }

                    // Apply normalization if needed (AC 3, 5)
                    if data.currency != target_cur {
                        data.apply_normalization(&target_cur);
                    }
                    
                    Ok(data)
                } else {
                    Err(format!("Harvest failed: {}", response.status()))
                }
            } else {
                Ok(HistoricalData::default())
            }
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
            <SearchBar on_select=move |info| {
                set_selected_ticker.set(Some(info));
            } />

            {move || selected_ticker.get().map(|ticker| {
                view! {
                    <div class="analyst-hud-init">
                        <div class="header-control-bar">
                            <h2>"Analyzing: " {ticker.name} " (" {ticker.ticker} ")"</h2>
                            <div class="currency-selector">
                                <label>"Display Currency: "</label>
                                <select on:change=move |ev| {
                                    set_target_currency.set(event_target_value(&ev));
                                }>
                                    <option value="USD" selected={move || target_currency.get() == "USD"}>"USD"</option>
                                    <option value="CHF" selected={move || target_currency.get() == "CHF"}>"CHF"</option>
                                    <option value="EUR" selected={move || target_currency.get() == "EUR"}>"EUR"</option>
                                </select>
                            </div>
                        </div>
                        <div class="hud-meta">
                            <span>"Exchange: " {ticker.exchange}</span>
                            " | "
                            <span>"Reporting Currency: " {ticker.currency}</span>
                        </div>

                        <Suspense fallback=|| view! {
                            <div class="loading-overlay">
                                <div class="pulse"></div>
                                <div class="status-text">"Querying Terminal Data..."</div>
                            </div>
                        }>
                            {move || historicals.get().map(|res| {
                                match res {
                                    Ok(ref data) if !data.records.is_empty() => {
                                        view! {
                                            <div class="data-ready">
                                                <div class="header-flex">
                                                    <h3>"10-Year Historicals Populated"</h3>
                                                    <div class="badge-group">
                                                        {if data.is_split_adjusted {
                                                            view! { <span class="badge split-badge">"Split-Adjusted"</span> }.into_any()
                                                        } else {
                                                            ().into_any()
                                                        }}
                                                        {if let Some(display_cur) = data.display_currency.as_ref() {
                                                            view! { <span class="badge norm-badge">"Normalized to " {display_cur.to_string()}</span> }.into_any()
                                                        } else {
                                                            ().into_any()
                                                        }}
                                                    </div>
                                                </div>
                                                <SSGChart data=data.clone() />
                                                <div class="records-grid">
                                                    <table>
                                                        <thead>
                                                            <tr>
                                                                <th>"Year"</th>
                                                                <th>"Sales"</th>
                                                                <th>"EPS"</th>
                                                                <th>"High"</th>
                                                                <th>"Low"</th>
                                                            </tr>
                                                        </thead>
                                                        <tbody>
                                                            {data.records.iter().map(|rec| {
                                                                view! {
                                                                    <tr>
                                                                        <td>{rec.fiscal_year}</td>
                                                                        <td>{rec.sales.to_string()}</td>
                                                                        <td>{rec.eps.to_string()}</td>
                                                                        <td>{rec.price_high.to_string()}</td>
                                                                        <td>{rec.price_low.to_string()}</td>
                                                                    </tr>
                                                                }
                                                            }).collect_view()}
                                                        </tbody>
                                                    </table>
                                                </div>
                                            </div>
                                        }.into_any()
                                    }
                                    Ok(_) => view! { <div class="awaiting">"Awaiting population trigger..."</div> }.into_any(),
                                    Err(e) => view! { <div class="integrity-alert">"Integrity Alert: " {e}</div> }.into_any(),
                                }
                            })}
                        </Suspense>
                    </div>
                }
            })}
        </ErrorBoundary>
    }
}
