use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderHealth {
    pub name: String,
    pub status: String,
    pub latency_ms: u64,
    pub rate_limit_percent: u32,
}

#[component]
pub fn SystemMonitor() -> impl IntoView {
    let health_resource = LocalResource::new(move || async move {
        match gloo_net::http::Request::get("/api/v1/system/health").send().await {
            Ok(resp) if resp.ok() => resp.json::<Vec<ProviderHealth>>().await.unwrap_or_default(),
            Ok(resp) => {
                leptos::logging::error!("Health API failed with status: {}", resp.status());
                vec![]
            }
            Err(e) => {
                leptos::logging::error!("Health API network error: {:?}", e);
                vec![]
            }
        }
    });

    view! {
        <div class="system-monitor-page">
            <div class="system-header">
                <h1>"SYSTEM MONITOR"</h1>
                <button
                    class="system-action-btn"
                    on:click=move |_| health_resource.refetch()
                >
                    "FORCE_REFRESH"
                </button>
            </div>

            <div class="health-indicators-grid">
                <Suspense fallback=|| view! { <div class="loading-message">"Scanning API endpoints..."</div> }>
                    {move || {
                        health_resource.get().map(|data| {
                            data.into_iter().map(|p| {
                                view! {
                                    <HealthIndicator provider=p />
                                }
                            }).collect_view()
                        })
                    }}
                </Suspense>
            </div>

            <div class="admin-console-panel">
                <h2>"ADMIN CONSOLE"</h2>
                <p class="console-line success">"> Monitoring primary feeds (CH, DE, US)..."</p>
                <p class="console-line success">"> All systems nominal."</p>
                <div class="console-nav">
                    <a href="/audit" class="console-link">"> System Audit Log"</a>
                    <a href="/" class="console-link">"<- Return to Terminal"</a>
                </div>
            </div>
        </div>
    }
}

#[component]
fn HealthIndicator(provider: ProviderHealth) -> impl IntoView {
    let latency_class = if provider.latency_ms > 500 {
        "latency-critical"
    } else if provider.latency_ms > 200 {
        "latency-warning"
    } else {
        "latency-good"
    };

    let status_class = if provider.status == "Online" {
        "status-online"
    } else {
        "status-offline"
    };

    view! {
        <div class="health-indicator-card">
            <div class="indicator-header">
                <span class="provider-name">{provider.name}</span>
                <span class=format!("status-badge {}", status_class)>
                    {provider.status}
                </span>
            </div>

            <div class="indicator-metrics">
                <div class="metric-block">
                    <label class="metric-label">"Latency"</label>
                    <span class=format!("metric-value {}", latency_class)>
                        {provider.latency_ms}"ms"
                    </span>
                </div>

                <div class="metric-block">
                    <label class="metric-label">"Rate Limit Consumption"</label>
                    <div class="rate-limit-bar">
                        <div
                            class="rate-limit-fill"
                            style=format!("width: {}%", provider.rate_limit_percent)
                        ></div>
                    </div>
                    <span class="rate-limit-percent">
                        {provider.rate_limit_percent}"%"
                    </span>
                </div>
            </div>
        </div>
    }
}
