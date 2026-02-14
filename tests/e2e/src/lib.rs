mod common;
#[cfg(test)]
mod epic3_tests;
#[cfg(test)]
mod epic4_tests;
#[cfg(test)]
mod epic5_tests;
#[cfg(test)]
mod epic6_tests;

#[cfg(test)]
mod search_tests {
    use super::common::TestContext;
    use thirtyfour::prelude::*;
    use anyhow::Result;

    #[tokio::test]
    async fn test_ticker_search_autocomplete() -> Result<()> {
        let ctx = TestContext::new().await?;
        
        // 1. Navigate to home page (Zen Search)
        ctx.navigate("/").await?;
        
        // 2. Find search input
        let search_input = ctx.driver.find(By::ClassName("zen-search-input")).await?;
        
        // 3. Type "AAPL" (triggers autocomplete)
        search_input.send_keys("AAPL").await?;
        
        // 4. Wait for results to appear
        let result_item = ctx.driver.query(By::ClassName("result-item")).first().await?;
        assert!(result_item.is_displayed().await?);
        
        // 5. Verify result content
        let ticker_code = result_item.find(By::ClassName("ticker-code")).await?;
        assert_eq!(ticker_code.text().await?, "AAPL");
        
        // 6. Click result and verify transition
        result_item.click().await?;
        
        // 7. Verify HUD reveal
        let hud = ctx.driver.query(By::ClassName("analyst-hud-init")).first().await?;
        assert!(hud.is_displayed().await?);
        
        ctx.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_historical_data_retrieval() -> Result<()> {
        let ctx = TestContext::new().await?;
        ctx.navigate("/").await?;
        
        let search_input = ctx.driver.find(By::ClassName("zen-search-input")).await?;
        search_input.send_keys("NESN").await?;
        
        let result_item = ctx.driver.query(By::ClassName("result-item")).first().await?;
        result_item.click().await?;
        
        // Wait for One-Click data to populate
        let data_ready = ctx.driver.query(By::ClassName("data-ready")).first().await?;
        assert!(data_ready.is_displayed().await?);
        
        let table = data_ready.find(By::Tag("table")).await?;
        let rows = table.find_all(By::Tag("tr")).await?;
        // 1 header + 10 rows
        assert_eq!(rows.len(), 11);
        
        ctx.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_harvest_error_handling() -> Result<()> {
        let ctx = TestContext::new().await?;
        ctx.navigate("/").await?;
        
        let search_input = ctx.driver.find(By::ClassName("zen-search-input")).await?;
        search_input.send_keys("UNKNOWN_TICKER").await?;
        
        // Verify that the UI reflects that no such ticker was found
        // or that it doesn't crash.
        let results = ctx.driver.find_all(By::ClassName("result-item")).await?;
        assert_eq!(results.len(), 0);
        
        ctx.cleanup().await?;
        Ok(())
    }
    #[tokio::test]
    async fn test_split_adjustment_indicator() -> Result<()> {
        let ctx = TestContext::new().await?;
        ctx.navigate("/").await?;
        
        let search_input = ctx.driver.find(By::ClassName("zen-search-input")).await?;
        // AAPL triggers a mock split in the backend
        search_input.send_keys("AAPL").await?;
        
        let result_item = ctx.driver.query(By::ClassName("result-item")).first().await?;
        result_item.click().await?;
        
        // Wait for data to populate
        let data_ready = ctx.driver.query(By::ClassName("data-ready")).first().await?;
        assert!(data_ready.is_displayed().await?);
        
        // Verify split-badge exists (AC 4)
        let badge = data_ready.find(By::ClassName("split-badge")).await?;
        assert!(badge.is_displayed().await?);
        assert_eq!(badge.text().await?, "Split-Adjusted");
        
        ctx.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_no_split_adjustment_indicator() -> Result<()> {
        let ctx = TestContext::new().await?;
        ctx.navigate("/").await?;
        
        let search_input = ctx.driver.find(By::ClassName("zen-search-input")).await?;
        search_input.send_keys("MSFT").await?;
        
        let result_item = ctx.driver.query(By::ClassName("result-item")).first().await?;
        result_item.click().await?;
        
        let data_ready = ctx.driver.query(By::ClassName("data-ready")).first().await?;
        assert!(data_ready.is_displayed().await?);
        
        // Verify split-badge does NOT exist
        let badges = data_ready.find_all(By::ClassName("split-badge")).await?;
        assert_eq!(badges.len(), 0);
        
        ctx.cleanup().await?;
        Ok(())
    }
}
