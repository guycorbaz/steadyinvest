use leptos::prelude::*;
use naic_logic::HistoricalData;
use rust_decimal::prelude::ToPrimitive;
use charming::{
    component::{Axis, Legend, Title},
    element::{AxisType, Orient, Tooltip, Trigger, LineStyle, LineStyleType},
    series::Line,
    Chart, WasmRenderer,
};

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
unsafe extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn setupDraggableHandles(chart_id: String, sales_start: f64, sales_years: f64, eps_start: f64, eps_years: f64);
}

// Global signals for JS access
thread_local! {
    static SALES_SIGNAL: std::cell::Cell<Option<RwSignal<f64>>> = const { std::cell::Cell::new(None) };
    static EPS_SIGNAL: std::cell::Cell<Option<RwSignal<f64>>> = const { std::cell::Cell::new(None) };
}

/// Updates the Sales projection CAGR from JavaScript drag handle
#[wasm_bindgen]
pub fn rust_update_sales_cagr(val: f64) {
    web_sys::console::log_1(&format!("[Slider] Sales CAGR updated to: {:.2}%", val).into());
    SALES_SIGNAL.with(|s| {
        if let Some(sig) = s.get() {
            sig.set(val);
        }
    });
}

/// Updates the EPS projection CAGR from JavaScript drag handle
#[wasm_bindgen]
pub fn rust_update_eps_cagr(val: f64) {
    web_sys::console::log_1(&format!("[Slider] EPS CAGR updated to: {:.2}%", val).into());
    EPS_SIGNAL.with(|s| {
        if let Some(sig) = s.get() {
            sig.set(val);
        }
    });
}

/// The Stock Selection Guide (SSG) Chart component.
/// 
/// Renders a logarithmic multi-series line chart (Sales, EPS, Price) with 
/// optional trendline overlays and CAGR labels.
/// 
/// Uses the `charming` library for ECharts-based rendering via WASM.
#[component]
pub fn SSGChart(
    data: HistoricalData,
    sales_projection_cagr: RwSignal<f64>,
    eps_projection_cagr: RwSignal<f64>
) -> impl IntoView {
    // Unique ID for the chart container to avoid conflicts
    let chart_id = format!("ssg-chart-{}", data.ticker.to_lowercase());
    
    // Reactive signal to toggle the visibility of trendlines and CAGR stats.
    let show_trends = RwSignal::new(true);
    
    // Projection state
    let is_projecting = RwSignal::new(false);

    // Store signals in thread-local for JS callbacks
    SALES_SIGNAL.with(|s| s.set(Some(sales_projection_cagr)));
    EPS_SIGNAL.with(|s| s.set(Some(eps_projection_cagr)));

    let cid_for_effect = chart_id.clone();
    Effect::new(move |_| {
        // Triggered when trends are toggled, data changes, or projections change.
        let trends_active = show_trends.get();
        let s_cagr = sales_projection_cagr.get();
        let e_cagr = eps_projection_cagr.get();
        let projecting = is_projecting.get();
        let cid = cid_for_effect.clone();
        
        // Transform data for charming
        let mut years = Vec::with_capacity(data.records.len());
        let mut prices = Vec::with_capacity(data.records.len());
        let mut prices_low = Vec::with_capacity(data.records.len());

        let mut raw_years = Vec::with_capacity(data.records.len());
        let mut sales = Vec::with_capacity(data.records.len());
        let mut eps = Vec::with_capacity(data.records.len());

        for record in &data.records {
            years.push(record.fiscal_year.to_string());
            raw_years.push(record.fiscal_year);

            sales.push(record.sales.to_f64().unwrap_or(0.0));
            eps.push(record.eps.to_f64().unwrap_or(0.0));
            prices.push(record.price_high.to_f64().unwrap_or(0.0));
            prices_low.push(record.price_low.to_f64().unwrap_or(0.0));
        }

        let mut chart = Chart::new()
            .title(Title::new()
                .text(format!("SSG Analysis: {}", data.ticker))
                .text_style(charming::element::TextStyle::new()
                    .color("#E0E0E0")
                    .font_family("Inter")
                    .font_size(16)
                    .font_weight("600")))
            .legend(Legend::new()
                .bottom(0)
                .orient(Orient::Horizontal)
                .text_style(charming::element::TextStyle::new()
                    .color("#B0B0B0")
                    .font_family("Inter")
                    .font_size(12)))
            .tooltip(Tooltip::new().trigger(Trigger::Axis))
            .x_axis(Axis::new()
                .type_(AxisType::Category)
                .data(years.clone())
                .axis_label(charming::element::AxisLabel::new()
                    .color("#B0B0B0")
                    .font_size(11)))
            .y_axis(Axis::new()
                .type_(AxisType::Log)
                .name("Value")
                .name_text_style(charming::element::TextStyle::new()
                    .color("#B0B0B0")
                    .font_family("Inter")
                    .font_size(12))
                .axis_label(charming::element::AxisLabel::new()
                    .color("#B0B0B0")
                    .font_size(11)));

        let mut sales_start = 0.0;
        let mut sales_years = 0.0;
        let mut eps_start = 0.0;
        let mut eps_years = 0.0;

        if trends_active {
            let sales_trend = naic_logic::calculate_growth_analysis(&raw_years, &sales);
            let eps_trend = naic_logic::calculate_growth_analysis(&raw_years, &eps);

            // Initialize projection signals if not yet set
            if !projecting {
                sales_projection_cagr.set(sales_trend.cagr);
                eps_projection_cagr.set(eps_trend.cagr);
                is_projecting.set(true);
            }

            sales_start = sales_trend.trendline[0].value;
            sales_years = (raw_years.last().unwrap_or(&2023) - raw_years[0] + 5) as f64;
            eps_start = eps_trend.trendline[0].value;
            // Fix: Calculate eps_years independently for clarity and correctness
            eps_years = (raw_years.last().unwrap_or(&2023) - raw_years[0] + 5) as f64;

            // Future years for projection (next 5 years)
            let last_year = *raw_years.last().unwrap_or(&2023);
            let future_years: Vec<i32> = (1..=5).map(|i| last_year + i).collect();
            let mut all_years_display = years.clone();
            for y in &future_years {
                all_years_display.push(y.to_string());
            }
            
            chart = chart.x_axis(Axis::new().type_(AxisType::Category).data(all_years_display));

            // Calculate projections
            // FIX: Negate CAGR values - slider increases should make projections go UP
            let s_proj = naic_logic::calculate_projected_trendline(
                raw_years[0],
                sales_start,
                -s_cagr,  // Negated to fix inversion bug
                &[raw_years.as_slice(), future_years.as_slice()].concat()
            );
            let e_proj = naic_logic::calculate_projected_trendline(
                raw_years[0],
                eps_start,
                -e_cagr,  // Negated to fix inversion bug
                &[raw_years.as_slice(), future_years.as_slice()].concat()
            );

            let s_proj_vals: Vec<f64> = s_proj.trendline.iter().map(|p| p.value).collect();
            let e_proj_vals: Vec<f64> = e_proj.trendline.iter().map(|p| p.value).collect();

            chart = chart
                .series(Line::new()
                    .name(format!("Sales (CAGR: {:.1}%)", s_cagr))
                    .data(sales.clone())
                    .smooth(true)
                    .line_style(LineStyle::new().color("#1DB954")))
                .series(Line::new()
                    .name("Sales Projection")
                    .data(s_proj_vals)
                    .line_style(LineStyle::new().color("#1DB954").width(2).type_(LineStyleType::Dashed)))
                .series(Line::new()
                    .name(format!("EPS (CAGR: {:.1}%)", e_cagr))
                    .data(eps.clone())
                    .smooth(true)
                    .line_style(LineStyle::new().color("#3498DB")))
                .series(Line::new()
                    .name("EPS Projection")
                    .data(e_proj_vals)
                    .line_style(LineStyle::new().color("#3498DB").width(2).type_(LineStyleType::Dashed)));
        } else {
            chart = chart
                .series(Line::new().name("Sales").data(sales).smooth(true).line_style(LineStyle::new().color("#1DB954")))
                .series(Line::new().name("EPS").data(eps).smooth(true).line_style(LineStyle::new().color("#3498DB")));
        }

        chart = chart
            .series(Line::new()
                .name("Price High")
                .data(prices)
                .smooth(true)
                .line_style(LineStyle::new().color("#F1C40F")))
            .series(Line::new()
                .name("Price Low")
                .data(prices_low)
                .smooth(true)
                .line_style(LineStyle::new().color("#E67E22")));

        // Use requestAnimationFrame to defer rendering until after DOM update
        let window = web_sys::window().expect("no global window");
        let render_callback = Closure::once(Box::new(move || {
            // Get actual container dimensions dynamically for responsive chart sizing
            let container = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id(&cid));
            let container_width = container.as_ref().map(|e| e.client_width()).unwrap_or(800) as u32;
            let container_height = container.as_ref().map(|e| e.client_height()).unwrap_or(500) as u32;

            // Ensure minimum dimensions and use actual container size
            let chart_width = container_width.max(600).min(1400);
            let chart_height = container_height.max(300).min(700);
            let renderer = WasmRenderer::new(chart_width, chart_height);
            if let Err(e) = renderer.render(&cid, &chart) {
                web_sys::console::log_1(&format!("Chart render error: {:?}", e).into());
            }

            if trends_active && projecting {
                setup_handles_js(cid, sales_start, sales_years, eps_start, eps_years);
            }
        }) as Box<dyn FnOnce()>);

        window.request_animation_frame(render_callback.as_ref().unchecked_ref()).ok();
        render_callback.forget();
    });

    fn setup_handles_js(cid: String, s_start: f64, s_years: f64, e_start: f64, e_years: f64) {
        setupDraggableHandles(cid, s_start, s_years, e_start, e_years);
    }

    let cid_for_view = chart_id.clone();
    view! {
        <div class="ssg-chart-wrapper" style="
            background-color: var(--background);
            padding: var(--spacing-5);
            border-radius: var(--border-radius-sharp);
            margin-bottom: var(--spacing-8);
            border: var(--border-width) solid var(--surface);
            width: 100%;
            max-width: 100%;
            box-sizing: border-box;
            overflow: hidden;
        ">
            <div class="chart-control-bar">
                <div class="chart-slider-controls">
                    <div class="chart-slider-row">
                        <span style="
                            color: var(--text-secondary);
                            font-size: var(--text-sm);
                            font-family: 'JetBrains Mono', monospace;
                            min-width: 160px;
                        ">"Projected Sales CAGR"</span>
                        <input
                            type="range" min="-20" max="50" step="0.1"
                            prop:value=move || sales_projection_cagr.get()
                            on:input=move |ev| {
                                if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                    sales_projection_cagr.set(val);
                                }
                            }
                            class="ssg-chart-slider"
                            style="
                                accent-color: var(--sales-color);
                                cursor: grab;
                            "
                        />
                        <span style="
                            color: var(--sales-color);
                            font-family: 'JetBrains Mono', monospace;
                            font-weight: bold;
                            width: 48px;
                            text-align: right;
                        ">{move || format!("{:.1}%", sales_projection_cagr.get())}</span>
                    </div>
                    <div class="chart-slider-row">
                        <span style="
                            color: var(--text-secondary);
                            font-size: var(--text-sm);
                            font-family: 'JetBrains Mono', monospace;
                            min-width: 160px;
                        ">"Projected EPS CAGR"</span>
                        <input
                            type="range" min="-20" max="50" step="0.1"
                            prop:value=move || eps_projection_cagr.get()
                            on:input=move |ev| {
                                if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                    eps_projection_cagr.set(val);
                                }
                            }
                            class="ssg-chart-slider"
                            style="
                                accent-color: var(--eps-color);
                                cursor: grab;
                            "
                        />
                        <span style="
                            color: var(--eps-color);
                            font-family: 'JetBrains Mono', monospace;
                            font-weight: bold;
                            width: 48px;
                            text-align: right;
                        ">{move || format!("{:.1}%", eps_projection_cagr.get())}</span>
                    </div>
                </div>
                <button
                    class="chart-trend-toggle"
                    on:click=move |_| show_trends.update(|v| *v = !*v)
                    style="
                        padding: var(--spacing-2) var(--spacing-4);
                        background: var(--surface);
                        color: var(--text-secondary);
                        font-size: var(--text-sm);
                        font-family: 'Inter', sans-serif;
                        font-weight: 500;
                        border-radius: var(--border-radius-sharp);
                        border: var(--border-width) solid rgba(255, 255, 255, 0.1);
                        transition: all var(--transition-fast);
                        cursor: pointer;
                        white-space: nowrap;
                    "
                    onmouseover="this.style.background='rgba(59, 130, 246, 0.1)'; this.style.color='var(--primary)'; this.style.borderColor='var(--primary)';"
                    onmouseout="this.style.background='var(--surface)'; this.style.color='var(--text-secondary)'; this.style.borderColor='rgba(255, 255, 255, 0.1)';"
                >
                    {move || if show_trends.get() { "Hide Trends" } else { "Show Trends" }}
                </button>
            </div>
            <div id=cid_for_view class="ssg-chart-container"></div>
            <div class="chart-cagr-mobile-summary">
                <div class="cagr-mobile-item">
                    <span class="cagr-mobile-label">"Sales CAGR"</span>
                    <span class="cagr-mobile-value" style="color: var(--sales-color);">
                        {move || format!("{:.1}%", sales_projection_cagr.get())}
                    </span>
                </div>
                <div class="cagr-mobile-item">
                    <span class="cagr-mobile-label">"EPS CAGR"</span>
                    <span class="cagr-mobile-value" style="color: var(--eps-color);">
                        {move || format!("{:.1}%", eps_projection_cagr.get())}
                    </span>
                </div>
            </div>
            <p class="chart-hint" style="
                color: var(--text-muted);
                font-size: var(--text-xs);
                margin-top: var(--spacing-2);
                font-style: italic;
                font-family: 'Inter', sans-serif;
            ">"Hint: Drag handles on the chart or use sliders for growth projections."</p>
        </div>
    }
}
