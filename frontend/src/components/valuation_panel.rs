//! Valuation analysis panel with P/E slider controls.
//!
//! Displays historical P/E context and lets the analyst adjust future High/Low
//! P/E estimates via range sliders. Computes projected buy-zone (floor) and
//! sell-zone (ceiling) target prices from EPS CAGR projections.

use leptos::prelude::*;
use steady_invest_logic::HistoricalData;
use rust_decimal::prelude::ToPrimitive;

/// Number of years forward for EPS projection.
const PROJECTION_YEARS: f64 = 5.0;
/// Maximum value for the High P/E range slider.
const PE_SLIDER_MAX: f64 = 100.0;

/// Interactive valuation analysis panel.
///
/// Shows 10-year historical P/E averages and current EPS, then lets the analyst
/// drag sliders to set future P/E estimates. Target buy/sell prices update
/// reactively based on the chart's EPS CAGR projection.
///
/// # Props
///
/// * `data` — Historical financial data (for P/E context and current EPS).
/// * `projected_eps_cagr` — Reactive EPS CAGR from the SSG chart sliders.
/// * `future_high_pe` / `future_low_pe` — Two-way bound P/E projection signals.
#[component]
pub fn ValuationPanel(
    data: HistoricalData,
    #[prop(into)] projected_eps_cagr: Signal<f64>,
    future_high_pe: RwSignal<f64>,
    future_low_pe: RwSignal<f64>,
) -> impl IntoView {
    let pe_analysis = data.pe_range_analysis.clone().unwrap_or_default();
    
    // Calculate current TTM EPS (latest record)
    let latest_record = data.records.iter().max_by_key(|r| r.fiscal_year);
    let current_eps = latest_record.map(|r| r.eps.to_f64().unwrap_or(0.0)).unwrap_or(0.0);

    // Calculate Projected EPS
    let projected_eps = move || {
        let cagr = projected_eps_cagr.get();
        current_eps * (1.0 + cagr / 100.0).powf(PROJECTION_YEARS)
    };

    // Calculate Target Zones
    let target_high_price = move || future_high_pe.get() * projected_eps();
    let target_low_price = move || future_low_pe.get() * projected_eps();

    view! {
        <div class="valuation-panel" style="
            background-color: var(--background);
            padding: var(--spacing-5);
            border-radius: var(--border-radius-sharp);
            margin-top: var(--spacing-8);
            border: var(--border-width) solid var(--surface);
        ">
            <div class="panel-header" style="
                border-bottom: var(--border-width) solid rgba(255, 255, 255, 0.1);
                padding-bottom: var(--spacing-3);
                margin-bottom: var(--spacing-5);
            ">
                <h3 style="
                    color: var(--text-primary);
                    margin: 0;
                    display: flex;
                    align-items: center;
                    gap: var(--spacing-2);
                    font-family: 'Inter', sans-serif;
                    font-size: var(--text-base);
                    font-weight: 600;
                ">
                    <span style="color: var(--price-color);">"📊"</span> "Valuation Analysis & Projections"
                </h3>
            </div>

            <div class="valuation-grid">
                // Historical Context
                <div class="historical-stats">
                    <h4 style="
                        color: var(--text-secondary);
                        text-transform: uppercase;
                        font-size: var(--text-xs);
                        margin-bottom: var(--spacing-4);
                        font-family: 'Inter', sans-serif;
                        font-weight: 600;
                        letter-spacing: 0.05em;
                    ">"10-Year Historical Context"</h4>
                    <div style="
                        display: flex;
                        flex-direction: column;
                        gap: var(--spacing-3);
                    ">
                        <div style="
                            display: flex;
                            justify-content: space-between;
                            color: var(--text-secondary);
                            font-family: 'Inter', sans-serif;
                            font-size: var(--text-sm);
                        ">
                            <span>"Avg. High P/E"</span>
                            <span style="
                                color: var(--text-primary);
                                font-weight: 500;
                                font-family: 'JetBrains Mono', monospace;
                            ">{format!("{:.1}", pe_analysis.avg_high_pe)}</span>
                        </div>
                        <div style="
                            display: flex;
                            justify-content: space-between;
                            color: var(--text-secondary);
                            font-family: 'Inter', sans-serif;
                            font-size: var(--text-sm);
                        ">
                            <span>"Avg. Low P/E"</span>
                            <span style="
                                color: var(--text-primary);
                                font-weight: 500;
                                font-family: 'JetBrains Mono', monospace;
                            ">{format!("{:.1}", pe_analysis.avg_low_pe)}</span>
                        </div>
                        <div style="
                            display: flex;
                            justify-content: space-between;
                            color: var(--text-secondary);
                            padding-top: var(--spacing-2);
                            border-top: var(--border-width) dashed rgba(255, 255, 255, 0.1);
                            font-family: 'Inter', sans-serif;
                            font-size: var(--text-sm);
                        ">
                            <span>"Current EPS (TTM)"</span>
                            <span style="
                                color: var(--text-primary);
                                font-weight: 500;
                                font-family: 'JetBrains Mono', monospace;
                            ">{format!("{:.2}", current_eps)}</span>
                        </div>
                        <div style="
                            display: flex;
                            justify-content: space-between;
                            color: var(--text-secondary);
                            font-family: 'Inter', sans-serif;
                            font-size: var(--text-sm);
                        ">
                            <span>"Projected EPS (5Y)"</span>
                            <span style="
                                color: var(--eps-color);
                                font-weight: 500;
                                font-family: 'JetBrains Mono', monospace;
                            ">{move || format!("{:.2}", projected_eps())}</span>
                        </div>
                    </div>
                </div>

                // User Inputs & Projections
                <div class="projection-controls">
                    <h4 style="
                        color: var(--text-secondary);
                        text-transform: uppercase;
                        font-size: var(--text-xs);
                        margin-bottom: var(--spacing-4);
                        font-family: 'Inter', sans-serif;
                        font-weight: 600;
                        letter-spacing: 0.05em;
                    ">"Valuation Floor & Ceiling"</h4>

                    <div class="control-group" style="margin-bottom: var(--spacing-4);">
                        <div style="
                            display: flex;
                            justify-content: space-between;
                            margin-bottom: var(--spacing-2);
                            align-items: center;
                        ">
                            <label style="
                                color: var(--text-secondary);
                                font-size: var(--text-sm);
                                font-family: 'Inter', sans-serif;
                            ">"Future Avg High P/E"</label>
                            <span style="
                                color: var(--price-color);
                                font-weight: 500;
                                font-family: 'JetBrains Mono', monospace;
                            ">{move || format!("{:.1}", future_high_pe.get())}</span>
                        </div>
                        <input
                            type="range" min="5" max=PE_SLIDER_MAX step="0.5"
                            prop:value=move || future_high_pe.get()
                            on:input=move |ev| {
                                if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                    future_high_pe.set(val);
                                }
                            }
                            style="
                                width: 100%;
                                accent-color: var(--price-color);
                                cursor: grab;
                            "
                        />
                    </div>

                    <div class="control-group">
                        <div style="
                            display: flex;
                            justify-content: space-between;
                            margin-bottom: var(--spacing-2);
                            align-items: center;
                        ">
                            <label style="
                                color: var(--text-secondary);
                                font-size: var(--text-sm);
                                font-family: 'Inter', sans-serif;
                            ">"Future Avg Low P/E"</label>
                            <span style="
                                color: var(--success);
                                font-weight: 500;
                                font-family: 'JetBrains Mono', monospace;
                            ">{move || format!("{:.1}", future_low_pe.get())}</span>
                        </div>
                        <input
                            type="range" min="5" max="60" step="0.5"
                            prop:value=move || future_low_pe.get()
                            on:input=move |ev| {
                                if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                    future_low_pe.set(val);
                                }
                            }
                            style="
                                width: 100%;
                                accent-color: var(--success);
                                cursor: grab;
                            "
                        />
                    </div>
                </div>
            </div>

            // Target Results
            <div class="target-results">
                <div class="buy-zone" style="
                    background-color: rgba(16, 185, 129, 0.05);
                    border: var(--border-width) solid rgba(16, 185, 129, 0.2);
                    padding: var(--spacing-4);
                    border-radius: var(--border-radius-sharp);
                    text-align: center;
                ">
                    <div style="
                        color: var(--success);
                        font-size: var(--text-xs);
                        text-transform: uppercase;
                        margin-bottom: var(--spacing-2);
                        font-weight: 600;
                        font-family: 'Inter', sans-serif;
                        letter-spacing: 0.05em;
                    ">"Target Buy Zone (Floor)"</div>
                    <div style="
                        color: var(--text-primary);
                        font-size: var(--text-xl);
                        font-weight: bold;
                        font-family: 'JetBrains Mono', monospace;
                    ">
                        "$" {move || format!("{:.2}", target_low_price())}
                    </div>
                </div>
                <div class="sell-zone" style="
                    background-color: rgba(239, 68, 68, 0.05);
                    border: var(--border-width) solid rgba(239, 68, 68, 0.2);
                    padding: var(--spacing-4);
                    border-radius: var(--border-radius-sharp);
                    text-align: center;
                ">
                    <div style="
                        color: var(--danger);
                        font-size: var(--text-xs);
                        text-transform: uppercase;
                        margin-bottom: var(--spacing-2);
                        font-weight: 600;
                        font-family: 'Inter', sans-serif;
                        letter-spacing: 0.05em;
                    ">"Target Sell Zone (Ceiling)"</div>
                    <div style="
                        color: var(--text-primary);
                        font-size: var(--text-xl);
                        font-weight: bold;
                        font-family: 'JetBrains Mono', monospace;
                    ">
                        "$" {move || format!("{:.2}", target_high_price())}
                    </div>
                </div>
            </div>

            <p style="
                color: var(--text-muted);
                font-size: var(--text-xs);
                margin-top: var(--spacing-4);
                font-style: italic;
                font-family: 'Inter', sans-serif;
            ">
                "Note: Projections are based on the drag-adjusted EPS CAGR from the chart above."
            </p>
        </div>
    }
}
