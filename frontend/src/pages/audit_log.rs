use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    pub id: i32,
    pub created_at: String,
    pub ticker: String,
    pub exchange: String,
    pub field_name: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub event_type: String,
    pub source: String,
}

#[component]
pub fn AuditLog() -> impl IntoView {
    let (ticker_filter, set_ticker_filter) = signal(String::new());
    let (type_filter, set_type_filter) = signal(String::new());

    let audit_resource = LocalResource::new(move || {
        let t = ticker_filter.get();
        let et = type_filter.get();
        async move {
            let mut url = String::from("/api/v1/system/audit-logs?");
            if !t.is_empty() {
                url.push_str(&format!("ticker={}&", t));
            }
            if !et.is_empty() {
                url.push_str(&format!("event_type={}&", et));
            }
            
            match gloo_net::http::Request::get(&url).send().await {
                Ok(resp) => {
                    if resp.ok() {
                        resp.json::<Vec<AuditEntry>>().await.unwrap_or_default()
                    } else {
                        leptos::logging::error!("Audit API failed with status: {}", resp.status());
                        vec![]
                    }
                }
                Err(e) => {
                    leptos::logging::error!("Audit network error: {:?}", e);
                    vec![]
                }
            }
        }
    });

    let export_url = move || {
        let t = ticker_filter.get();
        let et = type_filter.get();
        let mut url = String::from("/api/v1/system/audit-logs/export?");
        if !t.is_empty() {
            url.push_str(&format!("ticker={}&", t));
        }
        if !et.is_empty() {
            url.push_str(&format!("event_type={}&", et));
        }
        url
    };

    view! {
        <div class="p-8 bg-[#0F0F12] min-h-screen text-white font-mono">
            <div class="flex justify-between items-center mb-8 border-b border-gray-800 pb-4">
                <div class="flex items-center gap-4">
                    <h1 class="text-3xl font-bold">"AUDIT LOG"</h1>
                    <span class="text-[10px] text-yellow-500 border border-yellow-500/30 px-2 py-0.5 rounded">"INTEGRITY_SHIELD_ACTIVE"</span>
                </div>
                <div class="flex gap-4">
                     <a 
                        href=export_url 
                        target="_blank"
                        class="text-[10px] bg-blue-600/20 border border-blue-500/50 text-blue-400 px-3 py-1 hover:bg-blue-600/40 transition-colors"
                    >
                        "EXPORT_CSV"
                    </a>
                    <button 
                        class="text-[10px] border border-gray-700 px-3 py-1 hover:bg-gray-800 transition-colors"
                        on:click=move |_| audit_resource.refetch()
                    >
                        "FORCE_REFRESH"
                    </button>
                </div>
            </div>

            // Filters
            <div class="mb-6 flex gap-4 bg-[#16161A] p-4 border border-gray-800 rounded">
                <div class="flex flex-col gap-1">
                    <label class="text-[9px] text-gray-500 uppercase">"Ticker Query"</label>
                    <input 
                        type="text" 
                        placeholder="ALL"
                        class="bg-black border border-gray-800 text-xs p-1 focus:border-blue-500 outline-none w-32"
                        on:input=move |ev| set_ticker_filter.set(event_target_value(&ev))
                        prop:value=ticker_filter
                    />
                </div>
                <div class="flex flex-col gap-1">
                    <label class="text-[9px] text-gray-500 uppercase">"Event Type"</label>
                    <select 
                        class="bg-black border border-gray-800 text-xs p-1 focus:border-blue-500 outline-none"
                        on:change=move |ev| set_type_filter.set(event_target_value(&ev))
                    >
                        <option value="">"ALL_EVENTS"</option>
                        <option value="Anomaly">"ANOMALY"</option>
                        <option value="Override">"OVERRIDE"</option>
                    </select>
                </div>
            </div>
            
            <div class="border border-gray-800 rounded bg-[#16161A] overflow-x-auto">
                <table class="w-full text-left border-collapse min-w-[1000px]">
                    <thead>
                        <tr class="text-[10px] text-gray-500 border-b border-gray-800 uppercase tracking-tighter">
                            <th class="p-4 font-normal">"Timestamp"</th>
                            <th class="p-4 font-normal">"Source"</th>
                            <th class="p-4 font-normal">"Type"</th>
                            <th class="p-4 font-normal">"Asset"</th>
                            <th class="p-4 font-normal">"Field"</th>
                            <th class="p-4 font-normal">"Delta (Old -> New)"</th>
                        </tr>
                    </thead>
                    <tbody class="text-xs">
                        <Suspense fallback=|| view! { <tr><td colspan="6" class="p-8 text-center text-gray-600 italic">"Scanning audit sequence..."</td></tr> }>
                            {move || {
                                audit_resource.get().map(|data| {
                                    if data.is_empty() {
                                        return view! { <tr><td colspan="6" class="p-8 text-center text-gray-600">"No integrity events recorded or access restricted."</td></tr> }.into_any();
                                    }
                                    data.into_iter().map(|entry| {
                                        let type_class = if entry.event_type == "Anomaly" { "text-red-400" } else { "text-blue-400" };
                                        let source_class = if entry.source == "System" { "text-gray-400" } else { "text-purple-400" };
                                        
                                        view! {
                                            <tr class="border-b border-gray-800/50 hover:bg-white/5 transition-colors">
                                                <td class="p-4 text-[10px] text-gray-500 font-mono italic">{entry.created_at}</td>
                                                <td class="p-4"><span class=source_class>{entry.source}</span></td>
                                                <td class="p-4 font-bold uppercase tracking-tighter"><span class=type_class>{entry.event_type}</span></td>
                                                <td class="p-4 font-bold uppercase">{entry.exchange}":"{entry.ticker}</td>
                                                <td class="p-4"><span class="bg-gray-800 px-1 rounded text-[10px]">{entry.field_name}</span></td>
                                                <td class="p-4 font-mono">
                                                    <span class="text-gray-500">{entry.old_value.unwrap_or_else(|| "NULL".to_string())}</span>
                                                    <span class="mx-2 text-gray-700">"->"</span>
                                                    <span class="text-green-400">{entry.new_value.unwrap_or_else(|| "NULL".to_string())}</span>
                                                </td>
                                            </tr>
                                        }
                                    }).collect_view().into_any()
                                })
                            }}
                        </Suspense>
                    </tbody>
                </table>
            </div>

            <div class="mt-8">
                <a href="/system" class="text-[10px] text-gray-500 hover:text-white transition-colors">"<- Back to System Monitor"</a>
            </div>
        </div>
    }
}
