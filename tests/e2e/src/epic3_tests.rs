#[cfg(test)]
mod epic3_tests {
    use super::super::common::TestContext;
    use thirtyfour::prelude::*;
    use anyhow::Result;

    #[tokio::test]
    async fn test_manual_override_flow() -> Result<()> {
        let ctx = TestContext::new().await?;
        ctx.navigate("/").await?;
        
        let search_input = ctx.driver.find(By::ClassName("zen-search-input")).await?;
        search_input.send_keys("AAPL").await?;
        
        let result_item = ctx.driver.query(By::ClassName("result-item")).first().await?;
        result_item.click().await?;
        
        // Wait for data table
        let data_ready = ctx.driver.query(By::ClassName("data-ready")).first().await?;
        assert!(data_ready.is_displayed().await?);

        // Find a Sales cell (second column of first data row)
        let table = data_ready.find(By::Tag("table")).await?;
        let tbody = table.find(By::Tag("tbody")).await?;
        let rows = tbody.find_all(By::Tag("tr")).await?;
        let cells = rows[0].find_all(By::Tag("td")).await?;
        
        // Use action chain for double click
        let actions = ctx.driver.action_chain();
        actions.double_click_element(&cells[1]).perform().await?;

        // Verify Modal appears
        let modal = ctx.driver.query(By::ClassName("modal-content")).first().await?;
        assert!(modal.is_displayed().await?);

        // Fill override value and note
        let value_input = modal.find(By::Tag("input")).await?;
        value_input.clear().await?;
        value_input.send_keys("500.50").await?;

        let note_textarea = modal.find(By::Tag("textarea")).await?;
        note_textarea.send_keys("E2E Test Override Note").await?;

        // Save
        let save_btn = modal.find(By::ClassName("btn-primary")).await?;
        save_btn.click().await?;

        // Wait for modal to close and cell to update
        // (The cell should now have 'overridden-cell' class)
        let overridden_cell = ctx.driver.query(By::ClassName("overridden-cell")).first().await?;
        assert!(overridden_cell.is_displayed().await?);
        assert!(overridden_cell.text().await?.contains("500.5"));

        // Verify tooltip (title attribute)
        let title = overridden_cell.attr("title").await?.unwrap_or_default();
        assert!(title.contains("E2E Test Override Note"));

        ctx.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_kinetic_chart_dragging() -> Result<()> {
        let ctx = TestContext::new().await?;
        ctx.navigate("/").await?;
        
        let search_input = ctx.driver.find(By::ClassName("zen-search-input")).await?;
        search_input.send_keys("MSFT").await?;
        
        let result_item = ctx.driver.query(By::ClassName("result-item")).first().await?;
        result_item.click().await?;
        
        // Wait for chart
        let chart_wrapper = ctx.driver.query(By::ClassName("ssg-chart-wrapper")).first().await?;
        assert!(chart_wrapper.is_displayed().await?);

        // Get initial Sales CAGR value
        // Looking at ssg_chart.rs, it's displayed in a span with text containing "%"
        let spans = chart_wrapper.find_all(By::Tag("span")).await?;
        let mut initial_cagr = "".to_string();
        for span in &spans {
            let t = span.text().await?;
            if t.contains("%") {
                initial_cagr = t;
                break;
            }
        }

        // We can't easily find the ECharts handle via Selenium By locators.
        // Instead, we interact with the canvas at an offset.
        // Sales handle is typically at the right edge.
        let chart_div = chart_wrapper.find(By::Tag("div")).await?; // The ECharts container
        
        // Drag from right edge slightly up/down
        let actions = ctx.driver.action_chain();
        actions
            .move_to_element_with_offset(&chart_div, -50, 100) // Near the right edge
            .click_and_hold()
            .move_by_offset(0, -50)
            .release()
            .perform()
            .await?;

        // Wait a bit for signal to propagate
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Verify CAGR updated
        let mut final_cagr = "".to_string();
        let spans = chart_wrapper.find_all(By::Tag("span")).await?;
        for span in &spans {
            let t = span.text().await?;
            if t.contains("%") {
                final_cagr = t;
                break;
            }
        }
        
        assert_ne!(initial_cagr, final_cagr);

        ctx.cleanup().await?;
        Ok(())
    }
}
