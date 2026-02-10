use leptos::prelude::*;
use naic_logic::{HistoricalData, AnalysisSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockRequest {
    ticker: String,
    snapshot: AnalysisSnapshot,
    analyst_note: String,
}

#[component]
pub fn LockThesisModal(
    ticker: String,
    historical_data: HistoricalData,
    sales_projection_cagr: f64,
    eps_projection_cagr: f64,
    future_high_pe: f64,
    future_low_pe: f64,
    on_close: Callback<()>,
    on_locked: Callback<()>,
) -> impl IntoView {
    let (note, set_note) = signal(String::new());
    let (error, set_error) = signal(None::<String>);
    let (loading, set_loading) = signal(false);

    // Keyboard navigation: Close modal on Escape key
    // Accessibility: role="dialog" + aria-modal="true" marks background as inert for screen readers
    // Note: True keyboard focus trapping requires JS interception of Tab; ARIA alone does not trap Tab focus
    let on_keydown = {
        let on_close = on_close.clone();
        move |ev: leptos::ev::KeyboardEvent| {
            if ev.key() == "Escape" && !loading.get() {
                on_close.run(());
            }
        }
    };

    let lock = {
        let ticker = ticker.clone();
        let historical_data = historical_data.clone();
        move |_| {
            let note_val = note.get().trim().to_string();
            if note_val.is_empty() {
                set_error.set(Some("An analyst note is required to lock your thesis (AC 2).".to_string()));
                return;
            }

            let ticker = ticker.clone();
            let snapshot = AnalysisSnapshot {
                historical_data: historical_data.clone(),
                projected_sales_cagr: sales_projection_cagr,
                projected_eps_cagr: eps_projection_cagr,
                projected_high_pe: future_high_pe,
                projected_low_pe: future_low_pe,
                analyst_note: note_val.clone(),
                captured_at: chrono::Utc::now(),
            };

            leptos::task::spawn_local(async move {
                set_loading.set(true);
                let req = LockRequest {
                    ticker,
                    snapshot,
                    analyst_note: note_val,
                };

                let response = gloo_net::http::Request::post("/api/analyses/lock")
                    .json(&req)
                    .unwrap()
                    .send()
                    .await;

                match response {
                    Ok(res) if res.ok() => {
                        on_locked.run(());
                        on_close.run(());
                    }
                    _ => set_error.set(Some("Failed to lock thesis on server.".to_string())),
                }
                set_loading.set(false);
            });
        }
    };

    view! {
        <div class="modal-backdrop analyst-modal" on:keydown=on_keydown tabindex="-1" role="dialog" aria-modal="true" aria-labelledby="lock-thesis-modal-title">
            <div class="modal-content standard-border">
                <header>
                    <h3 id="lock-thesis-modal-title">"Finalize & Lock Analysis"</h3>
                    <button class="close-btn" on:click=move |_| on_close.run(()) aria-label="Close modal">"×"</button>
                </header>
                
                <div class="modal-body">
                    <p class="modal-intro">
                        "Locking this analysis creates a permanent, immutable snapshot of your projections and historical data. You will be able to retrieve this record later to verify your thesis."
                    </p>

                    <div class="summary-pill">
                        <div class="summary-item">
                            <span class="label">"Sales Growth"</span>
                            <span class="value">{format!("{:.1}%", sales_projection_cagr)}</span>
                        </div>
                        <div class="summary-item">
                            <span class="label">"EPS Growth"</span>
                            <span class="value">{format!("{:.1}%", eps_projection_cagr)}</span>
                        </div>
                        <div class="summary-item">
                            <span class="label">"Target P/E"</span>
                            <span class="value">{format!("{:.1}", future_high_pe)}</span>
                        </div>
                    </div>

                    <div class="input-group">
                        <label>"Investment Thesis Summary (Required)"</label>
                        <textarea
                            prop:value=note
                            on:input=move |ev| set_note.set(event_target_value(&ev))
                            placeholder="Why are you bullish/bearish? What are the key catalysts?"
                            rows="6"
                            autofocus
                        ></textarea>
                    </div>

                    {move || error.get().map(|e| view! { <div class="error-msg">{e}</div> })}
                </div>

                <footer>
                    <button class="secondary-btn" on:click=move |_| on_close.run(())>"Cancel"</button>
                    <button class="primary-btn security-blue-bg" on:click=lock disabled=loading>
                        {move || if loading.get() { "Locking..." } else { "Lock Permanent Snapshot" }}
                    </button>
                </footer>
            </div>
        </div>
    }
}
