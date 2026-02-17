//! PDF report generation service.
//!
//! Creates professional SSG report PDFs containing an embedded chart image,
//! quality dashboard table, valuation projections, and analyst notes.
//! Uses `charming` for server-side chart rendering and `genpdf` for PDF assembly.

use std::io::Cursor;
use charming::{
    component::{Axis, Legend},
    datatype::DataPointItem,
    element::{AxisType, ItemStyle, LineStyle, LineStyleType, Tooltip, Trigger},
    renderer::ImageRenderer,
    series::{Candlestick, Line},
    Chart,
};
use genpdf::{elements, fonts, style};
use steady_invest_logic::{AnalysisSnapshot, calculate_growth_analysis, calculate_projected_trendline};
use rust_decimal::prelude::ToPrimitive;

/// Alias for fallible report operations.
pub type ReportResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Stateless service for generating SSG report exports.
pub struct ReportingService;

impl ReportingService {
    /// Generates a PDF report. Note: This is a synchronous, CPU-intensive operation.
    /// Should be called within `tokio::task::spawn_blocking`.
    ///
    /// # Errors
    ///
    /// Returns an error if chart rendering, SVG-to-PNG conversion, font loading,
    /// or PDF assembly fails.
    pub fn generate_ssg_report(
        ticker_symbol: &str,
        created_at: chrono::DateTime<chrono::FixedOffset>,
        analyst_note: &str,
        snapshot: &AnalysisSnapshot,
    ) -> ReportResult<Vec<u8>> {
        // 1. Generate Chart SVG via charming (SSR)
        let chart = Self::create_ssg_chart(snapshot);
        let mut renderer = ImageRenderer::new(800, 600);
        
        let svg_content = renderer.render(&chart)
            .map_err(|e| format!("Charming error: {:?}", e))?;

        // 2. Render SVG to PNG via resvg (for embedding in PDF)
        let png_data = Self::svg_to_png(svg_content.as_bytes())?;

        // 3. Create PDF document via genpdf
        let font_dirs = vec![
            "/usr/share/fonts/truetype/dejavu",
            "/usr/share/fonts/TTF",
            "/usr/share/fonts/truetype/liberation",
            "/usr/local/share/fonts", // macOS fallback attempt
        ];
        
        let mut font_family = None;
        for dir in font_dirs {
            if std::path::Path::new(dir).exists() {
                if let Ok(ff) = fonts::from_files(dir, "DejaVuSans", None) {
                    font_family = Some(ff);
                    break;
                }
                if let Ok(ff) = fonts::from_files(dir, "LiberationSans", None) {
                    font_family = Some(ff);
                    break;
                }
            }
        }

        let font_family = font_family.ok_or_else(|| {
            let msg = "No suitable system fonts (DejaVuSans/LiberationSans) found. Please install them to enable PDF export.";
            tracing::error!(msg);
            msg
        })?;
        
        let mut doc = genpdf::Document::new(font_family);
        doc.set_title(format!("SSG Report: {}", ticker_symbol));

        let mut decorator = genpdf::SimplePageDecorator::new();
        decorator.set_margins(10);
        doc.set_page_decorator(decorator);

        // Header
        let header_style = style::Style::new().bold().with_font_size(18);
        doc.push(elements::StyledElement::new(
            elements::Text::new(format!("Stock Selection Guide: {}", ticker_symbol)),
            header_style
        ));
        
        doc.push(elements::StyledElement::new(
            elements::Text::new(format!("Analysis Date: {}", created_at.format("%Y-%m-%d"))),
            style::Style::new().with_font_size(10)
        ));
        
        doc.push(elements::Break::new(1.0));

        // Embed Chart
        let cursor = Cursor::new(png_data);
        // Bubble up image errors instead of silent string in PDF
        let img = elements::Image::from_reader(cursor)
            .map_err(|e| format!("Failed to load PNG into PDF: {}", e))?
            .with_alignment(genpdf::Alignment::Center);
        doc.push(img);

        // Analyst Note
        doc.push(elements::Break::new(1.0));
        doc.push(elements::StyledElement::new(
            elements::Text::new("Analyst Note:"),
            style::Style::new().bold()
        ));
        doc.push(elements::Paragraph::new(analyst_note));

        // Quality Dashboard Table
        doc.push(elements::Break::new(1.5));
        doc.push(elements::StyledElement::new(
            elements::Text::new("Evaluate Management"),
            style::Style::new().bold().with_font_size(14)
        ));
        
        let mut table = elements::TableLayout::new(vec![1, 1, 1]);
        table.set_cell_decorator(elements::FrameCellDecorator::new(true, true, true));
        
        let table_header_style = style::Style::new().bold();
        let mut header_row = table.row();
        header_row.push_element(elements::StyledElement::new(elements::Paragraph::new("Year"), table_header_style));
        header_row.push_element(elements::StyledElement::new(elements::Paragraph::new("% Earned on Equity"), table_header_style));
        header_row.push_element(elements::StyledElement::new(elements::Paragraph::new("% Pre-Tax Profit on Sales"), table_header_style));
        header_row.push().map_err(|e| format!("Table error: {}", e))?;

        let hist = &snapshot.historical_data;
        let quality = steady_invest_logic::calculate_quality_analysis(hist);
        
        for point in quality.points {
            let mut row = table.row();
            row.push_element(elements::Paragraph::new(point.year.to_string()));
            row.push_element(elements::Paragraph::new(format!("{:.1}", point.roe)));
            row.push_element(elements::Paragraph::new(format!("{:.1}", point.profit_on_sales)));
            row.push().map_err(|e| format!("Table error: {}", e))?;
        }
        doc.push(table);

        // Valuation Summary
        doc.push(elements::Break::new(1.5));
        doc.push(elements::StyledElement::new(
            elements::Text::new("Price-Earnings History & Valuation"),
            style::Style::new().bold().with_font_size(14)
        ));
        
        doc.push(elements::Text::new(format!("Estimated Sales Growth Rate: {:.1}%", snapshot.projected_sales_cagr)));
        doc.push(elements::Text::new(format!("Estimated EPS Growth Rate: {:.1}%", snapshot.projected_eps_cagr)));
        doc.push(elements::Text::new(format!("Estimated Average High P/E: {:.1}", snapshot.projected_high_pe)));
        doc.push(elements::Text::new(format!("Estimated Average Low P/E: {:.1}", snapshot.projected_low_pe)));

        let mut buffer = Vec::new();
        doc.render(&mut buffer).map_err(|e| format!("PDF render error: {}", e))?;

        Ok(buffer)
    }

    /// Builds an ECharts `Chart` matching the frontend SSG chart (NAIC Figure 2.1).
    ///
    /// Includes: Sales/EPS/PTP data + trendlines + projections + price candlestick bars.
    fn create_ssg_chart(snapshot: &AnalysisSnapshot) -> Chart {
        let hist = &snapshot.historical_data;

        let raw_years: Vec<i32> = hist.records.iter().map(|r| r.fiscal_year).collect();
        // Use NAN for non-positive values on the log-scale Y axis (log(0) is undefined)
        let sales_data: Vec<f64> = hist.records.iter().map(|r| {
            let v = r.sales.to_f64().unwrap_or(0.0);
            if v > 0.0 { v } else { f64::NAN }
        }).collect();
        let eps_data: Vec<f64> = hist.records.iter().map(|r| {
            let v = r.eps.to_f64().unwrap_or(0.0);
            if v > 0.0 { v } else { f64::NAN }
        }).collect();
        let ptp_data: Vec<f64> = hist.records.iter().map(|r| {
            r.pretax_income
                .map(|v| { let f = v.to_f64().unwrap_or(0.0); if f > 0.0 { f } else { f64::NAN } })
                .unwrap_or(f64::NAN)
        }).collect();
        let high_price: Vec<f64> = hist.records.iter().map(|r| r.price_high.to_f64().unwrap_or(0.0)).collect();
        let low_price: Vec<f64> = hist.records.iter().map(|r| r.price_low.to_f64().unwrap_or(0.0)).collect();

        // Compute trendlines
        let sales_trend = calculate_growth_analysis(&raw_years, &sales_data);
        let eps_trend = calculate_growth_analysis(&raw_years, &eps_data);

        let ptp_valid: Vec<(i32, f64)> = ptp_data.iter().zip(raw_years.iter())
            .filter(|(&v, _)| v > 0.0)
            .map(|(&v, &y)| (y, v))
            .collect();
        let ptp_years: Vec<i32> = ptp_valid.iter().map(|(y, _)| *y).collect();
        let ptp_vals: Vec<f64> = ptp_valid.iter().map(|(_, v)| *v).collect();
        let ptp_trend = calculate_growth_analysis(&ptp_years, &ptp_vals);

        // Build extended x-axis (historical + 5 projection years)
        let hist_len = raw_years.len();
        let last_year = *raw_years.last().unwrap_or(&2023);
        let future_years: Vec<i32> = (1..=5).map(|i| last_year + i).collect();
        let mut all_years: Vec<String> = raw_years.iter().map(|y| y.to_string()).collect();
        for y in &future_years {
            all_years.push(y.to_string());
        }

        let mut chart = Chart::new()
            .tooltip(Tooltip::new().trigger(Trigger::Axis))
            .legend(Legend::new().data(vec![
                "Sales", "Sales Trend", "Sales Est.",
                "EPS", "EPS Trend", "EPS Est.",
                "Pre-Tax Profit", "PTP Trend", "PTP Est.",
                "Stock Price",
            ]))
            .x_axis(Axis::new().type_(AxisType::Category).data(all_years))
            .y_axis(Axis::new().type_(AxisType::Log));

        // Trendline values padded with NaN for projection years
        let sales_trend_vals: Vec<f64> = sales_trend.trendline.iter().map(|p| p.value).collect();
        let eps_trend_vals: Vec<f64> = eps_trend.trendline.iter().map(|p| p.value).collect();

        let sales_last_trend = sales_trend_vals.last().copied().unwrap_or(0.0);
        let eps_last_trend = eps_trend_vals.last().copied().unwrap_or(0.0);

        let mut sales_tl = sales_trend_vals.clone();
        let mut eps_tl = eps_trend_vals.clone();
        for _ in 0..5 { sales_tl.push(f64::NAN); eps_tl.push(f64::NAN); }

        // PTP trendline mapped to full year range
        let mut ptp_tl: Vec<f64> = Vec::with_capacity(hist_len + 5);
        let mut ptp_tidx = 0;
        for &v in &ptp_data {
            if v > 0.0 && ptp_tidx < ptp_trend.trendline.len() {
                ptp_tl.push(ptp_trend.trendline[ptp_tidx].value);
                ptp_tidx += 1;
            } else {
                ptp_tl.push(f64::NAN);
            }
        }
        let ptp_last_trend = if !ptp_trend.trendline.is_empty() {
            ptp_trend.trendline.last().unwrap().value
        } else { 0.0 };
        for _ in 0..5 { ptp_tl.push(f64::NAN); }

        // Projections
        let s_proj = calculate_projected_trendline(last_year, sales_last_trend, snapshot.projected_sales_cagr, &future_years);
        let e_proj = calculate_projected_trendline(last_year, eps_last_trend, snapshot.projected_eps_cagr, &future_years);
        let p_proj = calculate_projected_trendline(last_year, ptp_last_trend, snapshot.projected_ptp_cagr, &future_years);

        let mut s_proj_data: Vec<f64> = vec![f64::NAN; hist_len - 1];
        s_proj_data.push(sales_last_trend);
        for p in &s_proj.trendline { s_proj_data.push(p.value); }

        let mut e_proj_data: Vec<f64> = vec![f64::NAN; hist_len - 1];
        e_proj_data.push(eps_last_trend);
        for p in &e_proj.trendline { e_proj_data.push(p.value); }

        let mut p_proj_data: Vec<f64> = vec![f64::NAN; hist_len - 1];
        // Guard: use NAN when PTP anchor is non-positive to avoid log(0) on chart
        p_proj_data.push(if ptp_last_trend > 0.0 { ptp_last_trend } else { f64::NAN });
        for p in &p_proj.trendline { p_proj_data.push(p.value); }

        // Sales series
        chart = chart
            .series(Line::new().name("Sales").data(sales_data).smooth(true)
                .line_style(LineStyle::new().color("#1DB954")))
            .series(Line::new().name("Sales Trend").data(sales_tl)
                .line_style(LineStyle::new().color("#1DB954").width(1).type_(LineStyleType::Dotted)))
            .series(Line::new().name("Sales Est.").data(s_proj_data)
                .line_style(LineStyle::new().color("#1DB954").width(2).type_(LineStyleType::Dashed)));

        // EPS series
        chart = chart
            .series(Line::new().name("EPS").data(eps_data).smooth(true)
                .line_style(LineStyle::new().color("#3498DB")))
            .series(Line::new().name("EPS Trend").data(eps_tl)
                .line_style(LineStyle::new().color("#3498DB").width(1).type_(LineStyleType::Dotted)))
            .series(Line::new().name("EPS Est.").data(e_proj_data)
                .line_style(LineStyle::new().color("#3498DB").width(2).type_(LineStyleType::Dashed)));

        // PTP series
        chart = chart
            .series(Line::new().name("Pre-Tax Profit").data(ptp_data).smooth(true)
                .line_style(LineStyle::new().color("#E74C3C")))
            .series(Line::new().name("PTP Trend").data(ptp_tl)
                .line_style(LineStyle::new().color("#E74C3C").width(1).type_(LineStyleType::Dotted)))
            .series(Line::new().name("PTP Est.").data(p_proj_data)
                .line_style(LineStyle::new().color("#E74C3C").width(2).type_(LineStyleType::Dashed)));

        // Price bars: candlestick with collapsed body (wick only)
        let candle_data: Vec<DataPointItem> = high_price.iter().zip(low_price.iter())
            .map(|(&high, &low)| {
                DataPointItem::new(vec![low, low, low, high])
                    .item_style(ItemStyle::new().color("#333333").border_color("#333333"))
            })
            .collect();
        chart = chart.series(Candlestick::new().name("Stock Price").data(candle_data));

        chart
    }

    /// Converts SVG bytes to a PNG image buffer via `resvg`.
    fn svg_to_png(svg_bytes: &[u8]) -> ReportResult<Vec<u8>> {
        let svg_content = std::str::from_utf8(svg_bytes).map_err(|e| e.to_string())?;
        let opt = resvg::usvg::Options::default();
        let mut fontdb = resvg::usvg::fontdb::Database::new();
        fontdb.load_system_fonts();
        
        let tree = resvg::usvg::Tree::from_str(svg_content, &opt, &fontdb).map_err(|e| e.to_string())?;
        
        let pixmap_size = tree.size().to_int_size();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).ok_or("Failed to create pixmap")?;
        
        // Fill with white background (AC 4: Institutional Aesthetic)
        pixmap.fill(resvg::tiny_skia::Color::WHITE);
        
        resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());
        
        // genpdf/printpdf doesn't support alpha channel. 
        // We convert RGBA to RGB by stripping the alpha.
        // Since we filled with white, the transparency is now flattened onto white.
        let mut img_buffer: image::RgbImage = image::ImageBuffer::new(pixmap.width(), pixmap.height());
        for (x, y, pixel) in img_buffer.enumerate_pixels_mut() {
            if let Some(p) = pixmap.pixel(x, y) {
                // tiny-skia pixels are premultiplied, but since we have a white background,
                // and if the SVG is mostly opaque, we can just take the raw values or unpremultiply.
                // Simple approach: premultiplied on white.
                *pixel = image::Rgb([p.red(), p.green(), p.blue()]);
            }
        }

        let mut png_data = Cursor::new(Vec::new());
        img_buffer.write_to(&mut png_data, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        
        Ok(png_data.into_inner())
    }
}
