use leptos::prelude::*;
use naic_logic::HistoricalData;
use rust_decimal::prelude::ToPrimitive;

const PROJECTION_YEARS: f64 = 5.0;
const PE_SLIDER_MAX: f64 = 100.0;

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
        <div class="valuation-panel" style="background-color: #0F0F12; padding: 20px; border-radius: 8px; margin-top: 30px; border: 1px solid #333; font-family: 'JetBrains Mono', monospace;">
            <div class="panel-header" style="border-bottom: 1px solid #333; padding-bottom: 10px; margin-bottom: 20px;">
                <h3 style="color: #E0E0E0; margin: 0; display: flex; align-items: center; gap: 8px;">
                    <span style="color: #F1C40F;">"📊"</span> "Valuation Analysis & Projections"
                </h3>
            </div>

            <div class="valuation-grid" style="display: grid; grid-template-columns: 1fr 1fr; gap: 30px;">
                // Historical Context
                <div class="historical-stats">
                    <h4 style="color: #888; text-transform: uppercase; font-size: 0.8rem; margin-bottom: 15px;">"10-Year Historical Context"</h4>
                    <div style="display: flex; flex-direction: column; gap: 10px;">
                        <div style="display: flex; justify-content: space-between; color: #BBB;">
                            <span>"Avg. High P/E"</span>
                            <span style="color: #E0E0E0; font-weight: bold;">{format!("{:.1}", pe_analysis.avg_high_pe)}</span>
                        </div>
                        <div style="display: flex; justify-content: space-between; color: #BBB;">
                            <span>"Avg. Low P/E"</span>
                            <span style="color: #E0E0E0; font-weight: bold;">{format!("{:.1}", pe_analysis.avg_low_pe)}</span>
                        </div>
                        <div style="display: flex; justify-content: space-between; color: #BBB; padding-top: 5px; border-top: 1px dashed #333;">
                            <span>"Current EPS (TTM)"</span>
                            <span style="color: #E0E0E0; font-weight: bold;">{format!("{:.2}", current_eps)}</span>
                        </div>
                        <div style="display: flex; justify-content: space-between; color: #BBB;">
                            <span>"Projected EPS (5Y)"</span>
                            <span style="color: #3498DB; font-weight: bold;">{move || format!("{:.2}", projected_eps())}</span>
                        </div>
                    </div>
                </div>

                // User Inputs & Projections
                <div class="projection-controls">
                    <h4 style="color: #888; text-transform: uppercase; font-size: 0.8rem; margin-bottom: 15px;">"Valuation Floor & Ceiling"</h4>
                    
                    <div class="control-group" style="margin-bottom: 15px;">
                        <div style="display: flex; justify-content: space-between; margin-bottom: 5px;">
                            <label style="color: #BBB; font-size: 0.9rem;">"Future Avg High P/E"</label>
                            <span style="color: #F1C40F; font-weight: bold;">{move || format!("{:.1}", future_high_pe.get())}</span>
                        </div>
                        <input 
                            type="range" min="5" max=PE_SLIDER_MAX step="0.5" 
                            prop:value=move || future_high_pe.get()
                            on:input=move |ev| {
                                if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                    future_high_pe.set(val);
                                }
                            }
                            style="width: 100%; accent-color: #F1C40F;"
                        />
                    </div>

                    <div class="control-group">
                        <div style="display: flex; justify-content: space-between; margin-bottom: 5px;">
                            <label style="color: #BBB; font-size: 0.9rem;">"Future Avg Low P/E"</label>
                            <span style="color: #2ECC71; font-weight: bold;">{move || format!("{:.1}", future_low_pe.get())}</span>
                        </div>
                        <input 
                            type="range" min="5" max="60" step="0.5" 
                            prop:value=move || future_low_pe.get()
                            on:input=move |ev| {
                                if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                    future_low_pe.set(val);
                                }
                            }
                            style="width: 100%; accent-color: #2ECC71;"
                        />
                    </div>
                </div>
            </div>

            // Target Results
            <div class="target-results" style="margin-top: 30px; display: grid; grid-template-columns: 1fr 1fr; gap: 20px;">
                <div class="buy-zone" style="background-color: #16261E; border: 1px solid #1B3B2A; padding: 15px; border-radius: 4px; text-align: center;">
                    <div style="color: #2ECC71; font-size: 0.7rem; text-transform: uppercase; margin-bottom: 5px; font-weight: bold;">"Target Buy Zone (Floor)"</div>
                    <div style="color: #FFF; font-size: 1.5rem; font-weight: bold;">
                        "$" {move || format!("{:.2}", target_low_price())}
                    </div>
                </div>
                <div class="sell-zone" style="background-color: #2A1A1A; border: 1px solid #3D1C1C; padding: 15px; border-radius: 4px; text-align: center;">
                    <div style="color: #E74C3C; font-size: 0.7rem; text-transform: uppercase; margin-bottom: 5px; font-weight: bold;">"Target Sell Zone (Ceiling)"</div>
                    <div style="color: #FFF; font-size: 1.5rem; font-weight: bold;">
                        "$" {move || format!("{:.2}", target_high_price())}
                    </div>
                </div>
            </div>
            
            <p style="color: #555; font-size: 0.7rem; margin-top: 15px; font-style: italic;">
                "Note: Projections are based on the drag-adjusted EPS CAGR from the chart above."
            </p>
        </div>
    }
}
