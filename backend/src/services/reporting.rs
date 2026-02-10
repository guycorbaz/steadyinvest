//! PDF report generation service.
//!
//! Creates professional SSG report PDFs containing an embedded chart image,
//! quality dashboard table, valuation projections, and analyst notes.
//! Uses `charming` for server-side chart rendering and `genpdf` for PDF assembly.

use std::io::Cursor;
use charming::{
    component::{Axis, Legend},
    element::{AxisType, Tooltip, Trigger},
    renderer::ImageRenderer,
    series::Line,
    Chart,
};
use genpdf::{elements, fonts, style};
use naic_logic::AnalysisSnapshot;
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
            elements::Text::new("Quality Dashboard"),
            style::Style::new().bold().with_font_size(14)
        ));
        
        let mut table = elements::TableLayout::new(vec![1, 1, 1]);
        table.set_cell_decorator(elements::FrameCellDecorator::new(true, true, true));
        
        let table_header_style = style::Style::new().bold();
        let mut header_row = table.row();
        header_row.push_element(elements::StyledElement::new(elements::Paragraph::new("Year"), table_header_style));
        header_row.push_element(elements::StyledElement::new(elements::Paragraph::new("ROE (%)"), table_header_style));
        header_row.push_element(elements::StyledElement::new(elements::Paragraph::new("Profit on Sales (%)"), table_header_style));
        header_row.push().map_err(|e| format!("Table error: {}", e))?;

        let hist = &snapshot.historical_data;
        let quality = naic_logic::calculate_quality_analysis(hist);
        
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
            elements::Text::new("Valuation Projections"),
            style::Style::new().bold().with_font_size(14)
        ));
        
        doc.push(elements::Text::new(format!("Projected Sales CAGR: {:.1}%", snapshot.projected_sales_cagr)));
        doc.push(elements::Text::new(format!("Projected EPS CAGR: {:.1}%", snapshot.projected_eps_cagr)));
        doc.push(elements::Text::new(format!("Projected High P/E: {:.1}", snapshot.projected_high_pe)));
        doc.push(elements::Text::new(format!("Projected Low P/E: {:.1}", snapshot.projected_low_pe)));

        let mut buffer = Vec::new();
        doc.render(&mut buffer).map_err(|e| format!("PDF render error: {}", e))?;

        Ok(buffer)
    }

    /// Builds an ECharts `Chart` object with Sales, EPS, and Price series.
    fn create_ssg_chart(snapshot: &AnalysisSnapshot) -> Chart {
        let hist = &snapshot.historical_data;
        
        let mut chart = Chart::new()
            .tooltip(Tooltip::new().trigger(Trigger::Axis))
            .legend(Legend::new().data(vec!["Sales", "EPS", "High Price", "Low Price"]))
            .x_axis(Axis::new()
                .type_(AxisType::Category)
                .data(hist.records.iter().map(|r| r.fiscal_year.to_string()).collect::<Vec<_>>()))
            .y_axis(Axis::new()
                .type_(AxisType::Log));

        let sales_data: Vec<f64> = hist.records.iter().map(|r| r.sales.to_f64().unwrap_or(0.0)).collect();
        let eps_data: Vec<f64> = hist.records.iter().map(|r| r.eps.to_f64().unwrap_or(0.0)).collect();
        let high_price: Vec<f64> = hist.records.iter().map(|r| r.price_high.to_f64().unwrap_or(0.0)).collect();
        let low_price: Vec<f64> = hist.records.iter().map(|r| r.price_low.to_f64().unwrap_or(0.0)).collect();

        chart = chart
            .series(Line::new().name("Sales").data(sales_data))
            .series(Line::new().name("EPS").data(eps_data))
            .series(Line::new().name("High Price").data(high_price))
            .series(Line::new().name("Low Price").data(low_price));

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
