use crate::common::TestContext;
use thirtyfour::prelude::*;
use anyhow::Result;

#[tokio::test]
#[ignore = "thesis lock flow requires modal+API+snapshot transition, flaky in headless CI"]
async fn test_thesis_locking_flow() -> Result<()> {
    let ctx = TestContext::new().await?;
    ctx.navigate("/").await?;
    
    // 1. Search for a ticker
    let search_input = ctx.driver.find(By::ClassName("zen-search-input")).await?;
    search_input.send_keys("NESN").await?;
    
    let result_item = ctx.driver.query(By::ClassName("result-item")).first().await?;
    result_item.click().await?;
    
    // 2. Wait for Analyst HUD
    let data_ready = ctx.driver.query(By::ClassName("data-ready")).first().await?;
    assert!(data_ready.is_displayed().await?);

    // 3. Click Lock Thesis
    let lock_btn = ctx.driver.find(By::XPath("//button[contains(text(), 'Lock Thesis')]")).await?;
    lock_btn.click().await?;

    // 4. Fill Modal (wait for it to become visible — CSS transitions may delay display)
    let modal = ctx.driver.query(By::ClassName("modal-content"))
        .wait(std::time::Duration::from_secs(5), std::time::Duration::from_millis(500))
        .and_displayed()
        .first().await?;

    let note_area = modal.find(By::Tag("textarea")).await?;
    note_area.send_keys("E2E Institutional Thesis - Quality Compounder").await?;

    let confirm_btn = modal.find(By::XPath("//button[contains(text(), 'Lock Permanent Snapshot')]")).await?;
    confirm_btn.click().await?;

    // 5. Verify transition to Snapshot view
    let snapshot_banner = ctx.driver.query(By::ClassName("snapshot-header")).first().await?;
    assert!(snapshot_banner.is_displayed().await?);
    
    let meta = ctx.driver.find(By::ClassName("snapshot-meta")).await?;
    assert!(meta.text().await?.contains("Captured on"));

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_persistence_ui_controls() -> Result<()> {
    let ctx = TestContext::new().await?;
    ctx.navigate("/").await?;
    
    // 1. Verify Folder Icon in Search Bar (Import)
    let import_btn = ctx.driver.query(By::ClassName("open-analysis-btn")).first().await?;
    assert!(import_btn.is_displayed().await?);
    assert_eq!(import_btn.attr("title").await?, Some("Open Analysis from File".to_string()));

    // 2. Navigate to a ticker
    let search_input = ctx.driver.find(By::ClassName("zen-search-input")).await?;
    search_input.send_keys("MSFT").await?;
    let result_item = ctx.driver.query(By::ClassName("result-item")).first().await?;
    result_item.click().await?;

    // 3. Verify Save to File button in AnalystHUD
    let save_btn = ctx.driver.query(By::ClassName("save-btn")).first().await?;
    assert!(save_btn.is_displayed().await?);
    assert!(save_btn.text().await?.contains("Save to File"));

    ctx.cleanup().await?;
    Ok(())
}
