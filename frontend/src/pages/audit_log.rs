//! Audit Log page (`/audit-log`).
//!
//! Displays a filterable table of data-integrity events (anomalies and manual
//! overrides) with an option to export the full log as CSV.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// URL-encodes a string for use in query parameters.
fn encode_uri(s: &str) -> String {
    js_sys::encode_uri_component(s).as_string().unwrap_or_default()
}

/// A single audit log entry as returned by the backend API.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    /// Database row ID.
    pub id: i32,
    /// Timestamp string (ISO 8601).
    pub created_at: String,
    /// Ticker symbol of the affected security.
    pub ticker: String,
    /// Exchange where the security is listed.
    pub exchange: String,
    /// The data field that was changed or flagged.
    pub field_name: String,
    /// Previous value (if applicable).
    pub old_value: Option<String>,
    /// New or flagged value.
    pub new_value: Option<String>,
    /// Event classification ("Anomaly" or "Override").
    pub event_type: String,
    /// Event origin ("System" or "User").
    pub source: String,
}

/// Audit log viewer page with ticker and event-type filters.
///
/// Fetches audit entries from `/api/v1/system/audit-logs` and renders them
/// in a sortable table. Provides a CSV export link and force-refresh button.
#[component]
pub fn AuditLog() -> impl IntoView {
    let (ticker_filter, set_ticker_filter) = signal(String::new());
    let (type_filter, set_type_filter) = signal(String::new());
    let (api_error, set_api_error) = signal(None::<String>);

    let audit_resource = LocalResource::new(move || {
        let t = ticker_filter.get();
        let et = type_filter.get();
        async move {
            let mut url = String::from("/api/v1/system/audit-logs?");
            if !t.is_empty() {
                url.push_str(&format!("ticker={}&", encode_uri(&t)));
            }
            if !et.is_empty() {
                url.push_str(&format!("event_type={}&", encode_uri(&et)));
            }

            match gloo_net::http::Request::get(&url).send().await {
                Ok(resp) => {
                    if resp.ok() {
                        set_api_error.set(None);
                        resp.json::<Vec<AuditEntry>>().await.unwrap_or_default()
                    } else {
                        let msg = format!("Audit API returned status {}", resp.status());
                        leptos::logging::error!("{}", msg);
                        set_api_error.set(Some(msg));
                        vec![]
                    }
                }
                Err(e) => {
                    let msg = format!("Network error loading audit data: {:?}", e);
                    leptos::logging::error!("{}", msg);
                    set_api_error.set(Some(msg));
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
            url.push_str(&format!("ticker={}&", encode_uri(&t)));
        }
        if !et.is_empty() {
            url.push_str(&format!("event_type={}&", encode_uri(&et)));
        }
        url
    };

    view! {
        <div class="audit-log-page">
            <div class="audit-header">
                <div class="audit-title-group">
                    <h1>"AUDIT LOG"</h1>
                    <span class="integrity-badge">"INTEGRITY_SHIELD_ACTIVE"</span>
                </div>
                <div class="audit-actions">
                    <a
                        href=export_url
                        target="_blank"
                        class="audit-export-btn"
                    >
                        "EXPORT_CSV"
                    </a>
                    <button
                        class="system-action-btn"
                        on:click=move |_| audit_resource.refetch()
                    >
                        "FORCE_REFRESH"
                    </button>
                </div>
            </div>

            // Filters
            <div class="audit-filters">
                <div class="filter-group">
                    <label class="filter-label">"Ticker Query"</label>
                    <input
                        type="text"
                        placeholder="ALL"
                        class="filter-input"
                        on:input=move |ev| set_ticker_filter.set(event_target_value(&ev))
                        prop:value=ticker_filter
                    />
                </div>
                <div class="filter-group">
                    <label class="filter-label">"Event Type"</label>
                    <select
                        class="filter-select"
                        on:change=move |ev| set_type_filter.set(event_target_value(&ev))
                    >
                        <option value="">"ALL_EVENTS"</option>
                        <option value="Anomaly">"ANOMALY"</option>
                        <option value="Override">"OVERRIDE"</option>
                    </select>
                </div>
            </div>

            <div class="audit-table-container">
                <table class="audit-table">
                    <thead>
                        <tr>
                            <th>"Timestamp"</th>
                            <th>"Source"</th>
                            <th>"Type"</th>
                            <th>"Asset"</th>
                            <th>"Field"</th>
                            <th>"Delta (Old -> New)"</th>
                        </tr>
                    </thead>
                    <tbody>
                        <Suspense fallback=|| view! { <tr><td colspan="6" class="audit-loading">"Scanning audit sequence..."</td></tr> }>
                            {move || {
                                audit_resource.get().map(|data| {
                                    if data.is_empty() {
                                        return if let Some(err) = api_error.get() {
                                            view! { <tr><td colspan="6" class="audit-empty error-msg">{format!("Failed to load audit data: {}", err)}</td></tr> }.into_any()
                                        } else {
                                            view! { <tr><td colspan="6" class="audit-empty">"No integrity events recorded or access restricted."</td></tr> }.into_any()
                                        };
                                    }
                                    data.into_iter().map(|entry| {
                                        let type_class = if entry.event_type == "Anomaly" { "audit-type-anomaly" } else { "audit-type-override" };
                                        let source_class = if entry.source == "System" { "audit-source-system" } else { "audit-source-user" };

                                        view! {
                                            <tr class="audit-row">
                                                <td class="audit-timestamp">{entry.created_at}</td>
                                                <td><span class=source_class>{entry.source}</span></td>
                                                <td><span class=type_class>{entry.event_type}</span></td>
                                                <td class="audit-asset">{entry.exchange}":"{entry.ticker}</td>
                                                <td><span class="audit-field-badge">{entry.field_name}</span></td>
                                                <td class="audit-delta">
                                                    <span class="delta-old">{entry.old_value.unwrap_or_else(|| "NULL".to_string())}</span>
                                                    <span class="delta-arrow">"->"</span>
                                                    <span class="delta-new">{entry.new_value.unwrap_or_else(|| "NULL".to_string())}</span>
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

            <div class="audit-footer">
                <a href="/system" class="audit-back-link">"<- Back to System Monitor"</a>
            </div>
        </div>
    }
}
