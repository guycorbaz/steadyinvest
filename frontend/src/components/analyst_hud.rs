//! Main analysis workspace component.
//!
//! The Analyst HUD is the primary interactive view, combining the SSG chart,
//! valuation panel, quality dashboard, and historical data grid into a single
//! cohesive workspace. It owns the shared projection signals that link the
//! chart sliders to the valuation calculations.

use crate::components::ssg_chart::SSGChart;
use crate::components::quality_dashboard::QualityDashboard;
use crate::components::valuation_panel::ValuationPanel;
use crate::components::override_modal::OverrideModal;
use crate::components::lock_thesis_modal::LockThesisModal;
use leptos::prelude::*;
use naic_logic::{HistoricalData, TickerInfo};

/// Multi-panel analysis workspace for live data.
///
/// Renders the SSG chart, valuation panel, quality dashboard, and data grid.
/// Manages shared reactive signals for Sales/EPS CAGR and P/E projections.
///
/// # Props
///
/// * `ticker` — Identity info for the selected security.
/// * `data` — The harvested 10-year historical data.
/// * `on_refetch` — Callback to trigger a data refresh after overrides or locks.
#[component]
pub fn AnalystHUD(
    ticker: TickerInfo,
    data: HistoricalData,
    on_refetch: Callback<()>,
) -> impl IntoView {
    // Shared projection signals for cross-component reactivity (AC 5)
    let sales_projection_cagr = RwSignal::new(0.0);
    let eps_projection_cagr = RwSignal::new(0.0);
    let future_high_pe = RwSignal::new(0.0);
    let future_low_pe = RwSignal::new(0.0);

    // Manual Override UI state
    #[derive(Clone, Debug)]
    struct ActiveOverride {
        year: i32,
        field: String,
        current_value: rust_decimal::Decimal,
        current_note: Option<String>,
    }
    let (active_override, set_active_override) = signal(Option::<ActiveOverride>::None);
    let (show_lock_modal, set_show_lock_modal) = signal(false);

    // Initialize P/E projections from historical averages
    Effect::new({
        let data = data.clone();
        move |_| {
            if let Some(pe) = &data.pe_range_analysis {
                future_high_pe.set(pe.avg_high_pe);
                future_low_pe.set(pe.avg_low_pe);
            }
        }
    });

    view! {
        <div class="data-ready">
            <div class="header-control-bar shadow-nav">
                <div class="actions">
                    <button 
                        class="primary-btn security-blue-bg" 
                        on:click=move |_| set_show_lock_modal.set(true)
                    >
                        "🔒 Lock Thesis"
                    </button>
                    <button 
                        class="btn-secondary save-btn"
                        on:click={
                            let data = data.clone();
                            move |_| {
                                let snapshot = naic_logic::AnalysisSnapshot {
                                    historical_data: data.clone(),
                                    projected_sales_cagr: sales_projection_cagr.get(),
                                    projected_eps_cagr: eps_projection_cagr.get(),
                                    projected_high_pe: future_high_pe.get(),
                                    projected_low_pe: future_low_pe.get(),
                                    analyst_note: String::new(),
                                    captured_at: chrono::Utc::now(),
                                };
                                let _ = crate::persistence::save_snapshot(&snapshot);
                            }
                        }
                    >
                        <span class="btn-icon">"💾"</span>
                        "Save to File"
                    </button>
                </div>
            </div>

            <div class="header-flex" style="margin-top: 1rem;">
                <h3>"10-Year Historicals Populated"</h3>
                <div class="badge-group">
                    {if data.is_split_adjusted {
                        view! { <span class="badge split-badge">"Split-Adjusted"</span> }.into_any()
                    } else {
                        ().into_any()
                    }}
                    {if let Some(display_cur) = data.display_currency.as_ref() {
                        let display_cur_str = display_cur.to_string();
                        view! { <span class="badge norm-badge">"Normalized to " {display_cur_str}</span> }.into_any()
                    } else {
                        ().into_any()
                    }}
                </div>
            </div>

            <SSGChart 
                data=data.clone() 
                sales_projection_cagr=sales_projection_cagr
                eps_projection_cagr=eps_projection_cagr
            />
            <ValuationPanel 
                data=data.clone()
                projected_eps_cagr=eps_projection_cagr
                future_high_pe=future_high_pe
                future_low_pe=future_low_pe
            />
            <QualityDashboard data=data.clone() />

            <div class="records-grid">
                <table>
                    <thead>
                        <tr>
                             <th>"Year"</th>
                             <th>"Sales"</th>
                             <th>"EPS"</th>
                             <th>"High"</th>
                             <th>"Low"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {data.records.iter().map(|rec| {
                            let year = rec.fiscal_year;
                            let get_ovr = |field_name: &str| {
                                rec.overrides.iter().find(|o| o.field_name == field_name).cloned()
                            };

                            let render_cell = move |val: rust_decimal::Decimal, field: &str| {
                                let field_owned = field.to_string();
                                let ovr = get_ovr(field);
                                let is_overridden = ovr.is_some();
                                let note = ovr.map(|o| o.note).flatten();
                                
                                let note_display = note.clone().unwrap_or_default();
                                let cell_title = if is_overridden { format!("Override: {}\nNote: {}", val, note_display) } else { "".to_string() };

                                view! {
                                    <td 
                                        class=move || if is_overridden { "overridden-cell" } else { "" }
                                        title=cell_title
                                        on:dblclick=move |_| {
                                            set_active_override.set(Some(ActiveOverride {
                                                year,
                                                field: field_owned.clone(),
                                                current_value: val,
                                                current_note: note.clone(),
                                            }));
                                        }
                                    >
                                        {val.to_string()}
                                        {if is_overridden { view! { <span class="override-mark">"*"</span> }.into_any() } else { ().into_any() }}
                                        {if note.is_some() { view! { <span class="note-dot"></span> }.into_any() } else { ().into_any() }}
                                    </td>
                                }
                            };

                            view! {
                                <tr>
                                    <td>{rec.fiscal_year}</td>
                                    {render_cell(rec.sales, "sales")}
                                    {render_cell(rec.eps, "eps")}
                                    {render_cell(rec.price_high, "price_high")}
                                    {render_cell(rec.price_low, "price_low")}
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
            
            {let data = data.clone(); move || active_override.get().map(|ovr| {
                let data = data.clone();
                view! {
                    <OverrideModal 
                        ticker=data.ticker
                        year=ovr.year
                        field=ovr.field
                        current_value=ovr.current_value
                        current_note=ovr.current_note
                        on_close=Callback::new(move |_| set_active_override.set(None))
                        on_save=Callback::new(move |_| on_refetch.run(()))
                    />
                }
            })}

            {let ticker = ticker.clone(); let data = data.clone(); move || show_lock_modal.get().then(|| {
                let data = data.clone();
                let ticker = ticker.clone();
                let chart_id = format!("ssg-chart-{}", ticker.ticker.to_lowercase());
                view! {
                    <LockThesisModal
                        ticker=ticker.ticker
                        chart_id=chart_id
                        historical_data=data
                        sales_projection_cagr=sales_projection_cagr.get()
                        eps_projection_cagr=eps_projection_cagr.get()
                        future_high_pe=future_high_pe.get()
                        future_low_pe=future_low_pe.get()
                        on_close=Callback::new(move |_| set_show_lock_modal.set(false))
                        on_locked=Callback::new(move |_| {
                            set_show_lock_modal.set(false);
                            on_refetch.run(());
                        })
                    />
                }
            })}
        </div>
    }
}
