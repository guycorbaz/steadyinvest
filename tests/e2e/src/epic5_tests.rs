#[cfg(test)]
mod tests {
    use crate::common::TestContext;
    use thirtyfour::prelude::*;
    use anyhow::Result;
    use std::time::Duration;

    #[tokio::test]
    async fn test_system_monitor_dashboard() -> Result<()> {
        let ctx: TestContext = TestContext::new().await?;
        ctx.navigate("/system-monitor").await?;

        // Wait for page-specific element (WASM needs time to load and render on full page nav)
        let _page = ctx.driver.query(By::ClassName("system-monitor-page"))
            .wait(Duration::from_secs(15), Duration::from_millis(500))
            .first().await?;

        // 1. Verify "SYSTEM MONITOR" header (scoped to page container to avoid picking up other h1 elements)
        let header: WebElement = _page.find(By::Tag("h1")).await?;
        assert!(header.text().await?.contains("SYSTEM"));
        
        // 2. Verify health indicator panels
        let ch_provider: WebElement = ctx.driver.query(By::XPath("//div[contains(., 'CH (SWX)')]")).first().await?;
        assert!(ch_provider.is_displayed().await?);
        
        let de_provider: WebElement = ctx.driver.query(By::XPath("//div[contains(., 'DE (DAX)')]")).first().await?;
        assert!(de_provider.is_displayed().await?);
        
        let us_provider: WebElement = ctx.driver.query(By::XPath("//div[contains(., 'US (NYSE/NASDAQ)')]")).first().await?;
        assert!(us_provider.is_displayed().await?);

        // 3. Verify status indicator colors/text
        let status: WebElement = ctx.driver.query(By::ClassName("status-online")).first().await?;
        assert!(status.is_displayed().await?);
        
        ctx.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_audit_log_page() -> Result<()> {
        let ctx: TestContext = TestContext::new().await?;
        ctx.navigate("/audit-log").await?;

        // Wait for page-specific element (WASM needs time to load and render on full page nav)
        let _page = ctx.driver.query(By::ClassName("audit-log-page"))
            .wait(Duration::from_secs(15), Duration::from_millis(500))
            .first().await?;

        // 1. Verify Header (scoped to page container to avoid picking up other h1 elements)
        let header: WebElement = _page.find(By::Tag("h1")).await?;
        assert!(header.text().await?.contains("AUDIT"));
        
        // 2. Verify high-density grid existence
        let grid: WebElement = ctx.driver.query(By::ClassName("audit-grid")).first().await?;
        assert!(grid.is_displayed().await?);
        
        // 3. Verify labels in the grid (at least one row header)
        let ticker_label: WebElement = ctx.driver.query(By::XPath("//span[contains(text(), 'Ticker')]")).first().await?;
        assert!(ticker_label.is_displayed().await?);

        // 4. Verify Export CSV button
        let export_btn: WebElement = ctx.driver.query(By::XPath("//button[contains(text(), 'Export CSV')]")).first().await?;
        assert!(export_btn.is_displayed().await?);
        
        ctx.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore = "bloomberg-speed class not yet implemented in frontend"]
    async fn test_system_health_latency_indicator() -> Result<()> {
        let ctx: TestContext = TestContext::new().await?;
        ctx.navigate("/system-monitor").await?;

        // Fulfills AC: Persistent health indicator in footer (Bloomberg Speed)
        let indicator: WebElement = ctx.driver.query(By::ClassName("bloomberg-speed")).first().await?;
        assert!(indicator.is_displayed().await?);

        let text = indicator.text().await?;
        assert!(text.contains("ms"), "Indicator should show render time in ms");

        // Check if it has a color class (either healthy or warning)
        let classes = indicator.class_name().await?.unwrap_or_default();
        assert!(classes.contains("glow-"), "Indicator should have a glow class");

        ctx.cleanup().await?;
        Ok(())
    }
}
