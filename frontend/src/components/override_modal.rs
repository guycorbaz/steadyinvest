use leptos::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OverrideRequest {
    ticker: String,
    fiscal_year: i32,
    field_name: String,
    value: Decimal,
    note: Option<String>,
}

#[component]
pub fn OverrideModal(
    ticker: String,
    year: i32,
    field: String,
    current_value: Decimal,
    current_note: Option<String>,
    on_close: Callback<()>,
    on_save: Callback<()>,
) -> impl IntoView {
    let (value, set_value) = signal(current_value.to_string());
    let (note, set_note) = signal(current_note.unwrap_or_default());
    let (error, set_error) = signal(None::<String>);
    let (loading, set_loading) = signal(false);

    // Keyboard navigation: Close modal on Escape key
    let on_keydown = {
        let on_close = on_close.clone();
        move |ev: leptos::ev::KeyboardEvent| {
            if ev.key() == "Escape" && !loading.get() {
                on_close.run(());
            }
        }
    };

    let save = {
        let ticker = ticker.clone();
        let field = field.clone();
        move |_| {
            let val_str = value.get();
            let ticker = ticker.clone();
            let field = field.clone();
            let note_val = note.get().trim().to_string();
            
            if note_val.is_empty() {
                set_error.set(Some("Audit note is required to explain this adjustment (AC 3).".to_string()));
                return;
            }

            leptos::task::spawn_local(async move {
                set_loading.set(true);
                match Decimal::from_str_exact(&val_str) {
                    Ok(decimal_val) => {
                        let req = OverrideRequest {
                            ticker,
                            fiscal_year: year,
                            field_name: field,
                            value: decimal_val,
                            note: Some(note_val),
                        };

                        let response = gloo_net::http::Request::post("/api/overrides")
                            .json(&req)
                            .unwrap()
                            .send()
                            .await;

                        match response {
                            Ok(res) if res.ok() => {
                                on_save.run(());
                                on_close.run(());
                            }
                            _ => set_error.set(Some("Failed to save override to server.".to_string())),
                        }
                    }
                    Err(_) => set_error.set(Some("Please enter a valid numeric value.".to_string())),
                }
                set_loading.set(false);
            });
        }
    };

    let delete = {
        let ticker = ticker.clone();
        let field = field.clone();
        move |_| {
            let ticker = ticker.clone();
            let field = field.clone();
            leptos::task::spawn_local(async move {
                set_loading.set(true);
                let url = format!("/api/overrides/{}/{}/{}", ticker, year, field);
                let response = gloo_net::http::Request::delete(&url)
                    .send()
                    .await;

                match response {
                    Ok(res) if res.ok() => {
                        on_save.run(());
                        on_close.run(());
                    }
                    _ => set_error.set(Some("Failed to remove override.".to_string())),
                }
                set_loading.set(false);
            });
        }
    };

    view! {
        <div class="modal-backdrop analyst-modal" on:keydown=on_keydown tabindex="-1">
            <div class="modal-content crimson-border">
                <header>
                    <h3>"Manual Override Request"</h3>
                    <button class="close-btn" on:click=move |_| on_close.run(()) aria-label="Close modal">"×"</button>
                </header>
                
                <div class="modal-body">
                    <div class="field-meta">
                        <span class="label">"Ticker:"</span> <span>{ticker.clone()}</span>
                        <span class="label">"Year:"</span> <span>{year}</span>
                        <span class="label">"Metric:"</span> <span>{field.clone()}</span>
                    </div>

                    <div class="input-group">
                        <label>"New Normalized Value"</label>
                        <input 
                            type="text" 
                            prop:value=value 
                            on:input=move |ev| set_value.set(event_target_value(&ev))
                            placeholder="e.g. 123.45"
                        />
                    </div>

                    <div class="input-group">
                        <label>"Audit Note (Required per AC 3)"</label>
                        <textarea 
                            prop:value=note 
                            on:input=move |ev| set_note.set(event_target_value(&ev))
                            placeholder="Explain why this adjustment is necessary..."
                        ></textarea>
                    </div>

                    {move || error.get().map(|e| view! { <div class="error-msg">{e}</div> })}
                </div>

                <footer>
                    <button class="secondary-btn" on:click=move |_| on_close.run(())>"Cancel"</button>
                    <button class="danger-btn" on:click=delete disabled=loading>"Remove Override"</button>
                    <button class="primary-btn crimson-bg" on:click=save disabled=loading>
                        {move || if loading.get() { "Saving..." } else { "Apply Override" }}
                    </button>
                </footer>
            </div>
        </div>
    }
}
