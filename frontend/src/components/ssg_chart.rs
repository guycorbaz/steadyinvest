use leptos::prelude::*;
use naic_logic::HistoricalData;
use rust_decimal::prelude::ToPrimitive;
use charming::{
    component::{Axis, Legend, Title},
    element::{AxisType, Tooltip, Trigger, LineStyle},
    series::Line,
    Chart, WasmRenderer,
};

#[component]
pub fn SSGChart(data: HistoricalData) -> impl IntoView {
    Effect::new(move |_| {
        // Transform data for charming
        let mut years = Vec::new();
        let mut sales = Vec::new();
        let mut eps = Vec::new();
        let mut prices = Vec::new();

        for record in &data.records {
            years.push(record.fiscal_year.to_string());
            sales.push(record.sales.to_f64().unwrap_or(0.0));
            eps.push(record.eps.to_f64().unwrap_or(0.0));
            prices.push(record.price_high.to_f64().unwrap_or(0.0));
        }

            let chart = Chart::new()
                .title(Title::new().text(format!("SSG Analysis: {}", data.ticker)).text_style(charming::element::TextStyle::new().color("#E0E0E0")))
                .legend(Legend::new().text_style(charming::element::TextStyle::new().color("#B0B0B0")))
                .tooltip(Tooltip::new().trigger(Trigger::Axis))
                .x_axis(Axis::new().type_(AxisType::Category).data(years))
                .y_axis(Axis::new().type_(AxisType::Log).name("Value"))
                .series(Line::new().name("Sales").data(sales).smooth(true).line_style(LineStyle::new().color("#1DB954")))
                .series(Line::new().name("EPS").data(eps).smooth(true).line_style(LineStyle::new().color("#3498DB")))
                .series(Line::new().name("Price High").data(prices).smooth(true).line_style(LineStyle::new().color("#F1C40F")));

            let renderer = WasmRenderer::new(1200, 600);
            renderer.render("ssg-chart-container", &chart).ok();
    });

    view! {
        <div class="ssg-chart-wrapper" style="background-color: #0F0F12; padding: 20px; border-radius: 8px; margin-bottom: 30px; border: 1px solid #333;">
            <div id="ssg-chart-container" style="width: 100%; height: 600px;"></div>
        </div>
    }
}
