use leptos::prelude::*;
use leptos_router::hooks::use_location;
use serde::Deserialize;

/// Health status record matching the backend ProviderHealth DTO.
#[derive(Deserialize, Clone)]
struct ProviderHealth {
    #[allow(dead_code)]
    name: String,
    status: String,
    #[allow(dead_code)]
    latency_ms: u64,
    #[allow(dead_code)]
    rate_limit_percent: u32,
}

/// Derive an overall status label from individual provider statuses.
fn overall_status(providers: &[ProviderHealth]) -> &'static str {
    if providers.is_empty() {
        return "OFFLINE";
    }
    let all_online = providers.iter().all(|p| p.status == "Online");
    let any_online = providers.iter().any(|p| p.status == "Online");
    if all_online {
        "ONLINE"
    } else if any_online {
        "DEGRADED"
    } else {
        "OFFLINE"
    }
}

/// Persistent global footer component with latency visualization.
///
/// Follows "Institutional HUD" styling:
/// - Dark background (#0F0F12)
/// - Monospace font (JetBrains Mono)
/// - Minimalist aesthetic
#[component]
pub fn Footer() -> impl IntoView {
    let location = use_location();
    let (latency, set_latency) = signal(0.0);
    let (status_text, set_status_text) = signal("OFFLINE".to_string());

    Effect::new(move |_| {
        let _ = location.pathname.get(); // Depend on location to trigger on navigation

        // Measure real network latency and parse health response
        if let Some(window) = web_sys::window() {
            if let Some(perf) = window.performance() {
                let start_time = perf.now();

                wasm_bindgen_futures::spawn_local(async move {
                    let result = gloo_net::http::Request::get("/api/v1/system/health")
                        .send()
                        .await;

                    // Measure latency regardless of parse outcome
                    if let Some(window) = web_sys::window() {
                        if let Some(perf) = window.performance() {
                            let end_time = perf.now();
                            set_latency.set(end_time - start_time);
                        }
                    }

                    match result {
                        Ok(resp) if resp.ok() => {
                            match resp.json::<Vec<ProviderHealth>>().await {
                                Ok(providers) => {
                                    set_status_text.set(overall_status(&providers).to_string());
                                }
                                Err(_) => set_status_text.set("ERROR".to_string()),
                            }
                        }
                        _ => set_status_text.set("OFFLINE".to_string()),
                    }
                });
            }
        }
    });

    let is_slow = move || latency.get() > 500.0;

    view! {
        <footer class="fixed bottom-0 w-full bg-[#0F0F12] text-[#8F8F8F] font-mono text-xs py-1 px-4 border-t border-[#2A2A2A] flex justify-between items-center z-50">
            <div class="flex items-center gap-4">
                <span class="opacity-50">"SYSTEM MARKETS"</span>
                <span class=move || {
                    match status_text.get().as_str() {
                        "ONLINE" => "text-[#00FF00]",
                        "DEGRADED" => "text-[#FFA500]",
                        _ => "text-[#DC143C]",
                    }.to_string()
                }>
                    {move || status_text.get()}
                </span>
            </div>

            <div class="flex items-center gap-2">
                <span class="opacity-50">"LATENCY"</span>
                <span
                    class=move || if is_slow() { "text-[#DC143C] font-bold" } else { "text-[#E5E5E5]" }
                >
                    {move || format!("{:.0}ms", latency.get())}
                </span>
            </div>
        </footer>
    }
}
