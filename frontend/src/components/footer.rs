use leptos::prelude::*;
use leptos_router::hooks::use_location;

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
    
    // Track location changes to simulate "navigation/render" latency
    // In a real app, we'd hook into the Router's transition start/end,
    // but for now, we'll measure the time since the last "navigation start" entry
    // if available, or just fallback to a mock/ping.
    //
    // Better yet, let's measure a "ping" to the health endpoint as a proxy for System Latency.
    // Or simpler: Use `window.performance` to get the latest navigation timing on mount/update.
    
    Effect::new(move |_| {
        let _ = location.pathname.get(); // Depend on location to trigger on navigation
        
        // Measure real network latency / system health response time
        if let Some(window) = web_sys::window() {
             if let Some(perf) = window.performance() {
                 let start_time = perf.now();
                 
                 wasm_bindgen_futures::spawn_local(async move {
                     // Ping the system health endpoint
                     // We use a HEAD request or simple GET to minimize payload
                     let _ = gloo_net::http::Request::get("/api/v1/system/health")
                        .send()
                        .await; // We don't care about the result payload for latency, just the round-trip
                     
                     if let Some(window) = web_sys::window() {
                        if let Some(perf) = window.performance() {
                            let end_time = perf.now();
                            let duration = end_time - start_time;
                            set_latency.set(duration);
                        }
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
                <span class="text-[#00FF00]">"ONLINE"</span>
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
