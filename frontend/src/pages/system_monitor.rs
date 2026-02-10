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
        let resp = gloo_net::http::Request::get("/api/v1/system/health")
            .send()
            .await
            .unwrap();
        let data: Vec<ProviderHealth> = resp.json().await.unwrap();
        data
    });

    view! {
        <div class="p-8 bg-[#0F0F12] min-h-screen text-white font-mono">
            <div class="flex justify-between items-center mb-8 border-b border-gray-800 pb-4">
                <h1 class="text-3xl font-bold">"SYSTEM MONITOR"</h1>
                <button 
                    class="text-[10px] border border-gray-700 px-3 py-1 hover:bg-gray-800 transition-colors"
                    on:click=move |_| health_resource.refetch()
                >
                    "FORCE_REFRESH"
                </button>
            </div>
            
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                <Suspense fallback=|| view! { <div class="text-gray-500">"Scanning API endpoints..."</div> }>
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

            <div class="mt-12 p-4 border border-gray-800 rounded bg-[#16161A]">
                <h2 class="text-sm text-gray-500 mb-2">"ADMIN CONSOLE"</h2>
                <p class="text-xs text-green-500">"> Monitoring primary feeds (CH, DE, US)..."</p>
                <p class="text-xs text-green-500">"> All systems nominal."</p>
                <div class="flex gap-4 mt-4">
                    <a href="/audit" class="text-[10px] text-blue-500 hover:underline">"> System Audit Log"</a>
                    <a href="/" class="text-[10px] text-blue-500 hover:underline">"<- Return to Terminal"</a>
                </div>
            </div>
        </div>
    }
}

#[component]
fn HealthIndicator(provider: ProviderHealth) -> impl IntoView {
    let latency_color = if provider.latency_ms > 500 {
        "text-red-500"
    } else if provider.latency_ms > 200 {
        "text-yellow-500"
    } else {
        "text-green-500"
    };

    let status_bg = if provider.status == "Online" {
        "bg-green-500"
    } else {
        "bg-red-500"
    };

    view! {
        <div class="p-6 border border-gray-800 rounded bg-[#16161A] shadow-lg hover:border-gray-600 transition-colors">
            <div class="flex justify-between items-center mb-4">
                <span class="text-lg font-bold uppercase tracking-wider">{provider.name}</span>
                <span class=format!("px-2 py-1 text-[10px] rounded uppercase font-bold {}", status_bg)>
                    {provider.status}
                </span>
            </div>

            <div class="space-y-4">
                <div>
                    <label class="text-[10px] text-gray-500 block uppercase">"Latency"</label>
                    <span class=format!("text-2xl font-bold {}", latency_color)>
                        {provider.latency_ms}"ms"
                    </span>
                </div>

                <div>
                    <label class="text-[10px] text-gray-500 block uppercase mb-1">"Rate Limit Consumption"</label>
                    <div class="w-full bg-gray-900 h-2 rounded-full overflow-hidden">
                        <div 
                            class="bg-blue-600 h-full transition-all" 
                            style=format!("width: {}%", provider.rate_limit_percent)
                        ></div>
                    </div>
                    <span class="text-[10px] text-gray-400 mt-1 block text-right">
                        {provider.rate_limit_percent}"%"
                    </span>
                </div>
            </div>
        </div>
    }
}
