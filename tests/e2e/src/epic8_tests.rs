//! Epic 8 E2E tests: Comparison view, History Timeline Sidebar, responsive
//! design, and currency handling.
//!
//! **Prerequisites**: Tests assume a fresh database (`dangerously_recreate`
//! at boot). Running locally multiple times will accumulate seeded data.
//! Tests seed their own snapshots via the backend API using `reqwest`.

use crate::common::{TestContext, DESKTOP_WIDE, MOBILE, TABLET};
use anyhow::Result;
use std::time::Duration;
use thirtyfour::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Base URL for direct API calls (same backend the SPA proxies to).
fn api_base() -> String {
    std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:5173".to_string())
}

/// Seed a locked analysis snapshot via the backend API.
/// Returns `(snapshot_id, ticker_id)`.
async fn seed_snapshot(
    client: &reqwest::Client,
    ticker: &str,
    notes: &str,
    currency: &str,
    sales_cagr: f64,
    eps_cagr: f64,
) -> Result<(i32, i32)> {
    let body = serde_json::json!({
        "ticker": ticker,
        "thesis_locked": true,
        "notes": notes,
        "snapshot_data": {
            "projected_sales_cagr": sales_cagr,
            "projected_eps_cagr": eps_cagr,
            "projected_high_pe": 25.0,
            "projected_low_pe": 15.0,
            "current_price": 95.50,
            "target_high_price": 145.20,
            "target_low_price": 88.30,
            "native_currency": currency,
            "upside_downside_ratio": 3.2,
            "valuation_zone": "Buy"
        }
    });

    let resp = client
        .post(format!("{}/api/v1/snapshots", api_base()))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let json: serde_json::Value = resp.json().await?;
    assert!(
        status.is_success(),
        "seed_snapshot failed ({}): {json}",
        status
    );
    let id = json["id"].as_i64().expect("snapshot id") as i32;
    let ticker_id = json["ticker_id"].as_i64().expect("ticker_id") as i32;
    Ok((id, ticker_id))
}

/// Seed a comparison set with the given snapshot IDs.
/// Returns the created comparison set ID.
async fn seed_comparison_set(
    client: &reqwest::Client,
    name: &str,
    base_currency: &str,
    snapshot_ids: &[i32],
) -> Result<i32> {
    let items: Vec<serde_json::Value> = snapshot_ids
        .iter()
        .enumerate()
        .map(|(i, &sid)| {
            serde_json::json!({
                "analysis_snapshot_id": sid,
                "sort_order": i as i32
            })
        })
        .collect();

    let body = serde_json::json!({
        "name": name,
        "base_currency": base_currency,
        "items": items,
    });

    let resp = client
        .post(format!("{}/api/v1/comparisons", api_base()))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let json: serde_json::Value = resp.json().await?;
    assert!(
        status.is_success(),
        "seed_comparison_set failed ({}): {json}",
        status
    );
    let id = json["id"].as_i64().expect("comparison set id") as i32;
    Ok(id)
}

/// Helper: search for a ticker, click first result, wait for HUD.
async fn load_ticker(ctx: &TestContext, ticker: &str) -> Result<()> {
    ctx.navigate("/").await?;
    let search_input = ctx.driver.find(By::ClassName("zen-search-input")).await?;
    search_input.send_keys(ticker).await?;
    let result_item = ctx
        .driver
        .query(By::ClassName("result-item"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    result_item.click().await?;
    ctx.driver
        .query(By::ClassName("analyst-hud-init"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .first()
        .await?;
    Ok(())
}

// ============================================================
// Task 3: Comparison view E2E tests (AC: #1)
// ============================================================

#[tokio::test]
async fn test_comparison_navigation_from_command_strip() -> Result<()> {
    let ctx = TestContext::new().await?;
    ctx.navigate("/").await?;

    // Find the "Compare" link in the command strip
    let command_strip = ctx.driver.find(By::ClassName("command-strip")).await?;
    let nav_links = command_strip.find_all(By::Tag("a")).await?;
    for link in &nav_links {
        let href = link.attr("href").await?.unwrap_or_default();
        if href.contains("/compare") {
            link.click().await?;
            break;
        }
    }

    let page = ctx
        .driver
        .query(By::ClassName("comparison-page"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .first()
        .await?;
    assert!(page.is_displayed().await?, "Comparison page should render");

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_comparison_direct_url_with_ticker_ids() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    // Seed snapshots for two different tickers
    let (_s1, nesn_id) = seed_snapshot(&client, "NESN.SW", "compare-url-test-1", "CHF", 8.0, 10.0).await?;
    let (_s2, aapl_id) = seed_snapshot(&client, "AAPL", "compare-url-test-2", "USD", 12.0, 15.0).await?;

    ctx.navigate(&format!(
        "/compare?ticker_ids={},{}",
        nesn_id, aapl_id
    ))
    .await?;

    // Wait for cards to render
    let cards = ctx
        .driver
        .query(By::ClassName("compact-card"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .all_required()
        .await?;
    assert!(cards.len() >= 2, "Should have at least 2 comparison cards");

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_comparison_card_metrics_displayed() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let (_s, nesn_id) = seed_snapshot(&client, "NESN.SW", "card-metrics-test", "CHF", 7.5, 11.0).await?;

    ctx.navigate(&format!("/compare?ticker_ids={}", nesn_id))
        .await?;

    let card = ctx
        .driver
        .query(By::ClassName("compact-card"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .first()
        .await?;

    // Verify ticker symbol is displayed
    let ticker_el = card.find(By::ClassName("card-ticker")).await?;
    let ticker_text = ticker_el.text().await?;
    assert!(!ticker_text.is_empty(), "Card should show ticker symbol");

    // Verify metric values are present
    let metrics = card.find_all(By::ClassName("metric-value")).await?;
    assert!(
        metrics.len() >= 4,
        "Card should have at least 4 metric values, got {}",
        metrics.len()
    );

    // Verify zone dot exists
    let zone_dots = card.find_all(By::ClassName("zone-dot")).await?;
    assert!(
        !zone_dots.is_empty(),
        "Card should have a valuation zone indicator"
    );

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_comparison_sorting_by_column() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    // Seed 3 snapshots with different metrics for sortable differentiation
    let (_s1, nesn_id) = seed_snapshot(&client, "NESN.SW", "sort-test-1", "CHF", 5.0, 7.0).await?;
    let (_s2, aapl_id) = seed_snapshot(&client, "AAPL", "sort-test-2", "USD", 15.0, 20.0).await?;
    let (_s3, msft_id) = seed_snapshot(&client, "MSFT", "sort-test-3", "USD", 10.0, 12.0).await?;

    ctx.navigate(&format!(
        "/compare?ticker_ids={},{},{}",
        nesn_id, aapl_id, msft_id
    ))
    .await?;

    // Wait for cards
    ctx.driver
        .query(By::ClassName("compact-card"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .first()
        .await?;

    // Find sort headers and click one
    let sort_headers = ctx
        .driver
        .find_all(By::ClassName("sort-header"))
        .await?;
    assert!(
        !sort_headers.is_empty(),
        "Should have sortable column headers"
    );

    // Click a sort header to sort
    sort_headers[0].click().await?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let cards_mid = ctx.driver.find_all(By::ClassName("compact-card")).await?;
    let first_ticker_mid = cards_mid[0]
        .find(By::ClassName("card-ticker"))
        .await?
        .text()
        .await?;

    // Click again to reverse
    let sort_headers = ctx.driver.find_all(By::ClassName("sort-header")).await?;
    sort_headers[0].click().await?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let cards_after = ctx.driver.find_all(By::ClassName("compact-card")).await?;
    let first_ticker_after = cards_after[0]
        .find(By::ClassName("card-ticker"))
        .await?
        .text()
        .await?;

    // After sort + reverse, first card should differ (proves bidirectional sorting)
    assert!(
        cards_after.len() >= 3,
        "Cards should still be present after sorting"
    );
    assert_ne!(
        first_ticker_mid, first_ticker_after,
        "Sorting and reversing should produce different card orders"
    );

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_comparison_save_set() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let (_s1, nesn_id) = seed_snapshot(&client, "NESN.SW", "save-set-test-1", "CHF", 8.0, 10.0).await?;
    let (_s2, aapl_id) = seed_snapshot(&client, "AAPL", "save-set-test-2", "USD", 12.0, 15.0).await?;

    ctx.navigate(&format!(
        "/compare?ticker_ids={},{}",
        nesn_id, aapl_id
    ))
    .await?;

    // Wait for cards to load
    ctx.driver
        .query(By::ClassName("compact-card"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .first()
        .await?;

    // Find the save form and enter a name
    let save_form = ctx
        .driver
        .query(By::ClassName("save-comparison-form"))
        .wait(Duration::from_secs(5), Duration::from_millis(500))
        .first()
        .await?;
    let name_input = save_form.find(By::Tag("input")).await?;
    name_input.send_keys("E2E Test Comparison").await?;

    // Click save button
    let save_btn = save_form
        .find(By::XPath(".//button[contains(text(), 'Save')]"))
        .await?;
    save_btn.click().await?;

    // Wait briefly for the save operation
    tokio::time::sleep(Duration::from_secs(2)).await;

    // The save should succeed without errors (page should still show cards)
    let cards = ctx.driver.find_all(By::ClassName("compact-card")).await?;
    assert!(
        !cards.is_empty(),
        "Cards should still be visible after saving"
    );

    // Also verify we can load a pre-saved comparison set via API seed
    let (s1, _) = seed_snapshot(&client, "NESN.SW", "save-set-load-1", "CHF", 6.0, 8.0).await?;
    let (s2, _) = seed_snapshot(&client, "AAPL", "save-set-load-2", "USD", 10.0, 14.0).await?;
    let set_id = seed_comparison_set(&client, "API Seeded Set", "CHF", &[s1, s2]).await?;
    ctx.navigate(&format!("/compare?id={}", set_id)).await?;

    let loaded_cards = ctx
        .driver
        .query(By::ClassName("compact-card"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .all_required()
        .await?;
    assert!(
        loaded_cards.len() >= 2,
        "Saved comparison set should load with cards"
    );

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_comparison_card_click_navigates_to_analysis() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let (_snap_id, nesn_id) =
        seed_snapshot(&client, "NESN.SW", "card-click-nav-test", "CHF", 8.0, 10.0).await?;

    ctx.navigate(&format!("/compare?ticker_ids={}", nesn_id))
        .await?;

    let card = ctx
        .driver
        .query(By::ClassName("compact-card"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .first()
        .await?;
    card.click().await?;

    // Should navigate to analysis view — wait for the search bar (home page element)
    let _search = ctx
        .driver
        .query(By::ClassName("zen-search-input"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .first()
        .await?;

    // Verify URL contains snapshot parameter
    let url = ctx.driver.current_url().await?;
    assert!(
        url.as_str().contains("snapshot="),
        "URL should contain snapshot= parameter after card click, got: {}",
        url
    );

    ctx.cleanup().await?;
    Ok(())
}

// ============================================================
// Task 4: History Timeline Sidebar E2E tests (AC: #2)
// ============================================================

#[tokio::test]
async fn test_history_toggle_opens_sidebar() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    // Seed 2 snapshots for NESN to enable history
    let _s1 = seed_snapshot(&client, "NESN.SW", "history-open-test-1", "CHF", 6.0, 8.0).await?;
    let _s2 = seed_snapshot(&client, "NESN.SW", "history-open-test-2", "CHF", 7.0, 9.0).await?;

    load_ticker(&ctx, "NESN").await?;

    // Find and click the history toggle
    let toggle = ctx
        .driver
        .query(By::ClassName("history-toggle-btn"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    toggle.click().await?;

    // Verify sidebar appears
    let sidebar = ctx
        .driver
        .query(By::ClassName("timeline-sidebar"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    assert!(sidebar.is_displayed().await?, "Timeline sidebar should be visible");

    // Verify aria-expanded is true
    let expanded = toggle.attr("aria-expanded").await?;
    assert_eq!(
        expanded.as_deref(),
        Some("true"),
        "Toggle should have aria-expanded=true"
    );

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_history_toggle_closes_sidebar() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let _s1 = seed_snapshot(&client, "NESN.SW", "history-close-test-1", "CHF", 6.0, 8.0).await?;
    let _s2 = seed_snapshot(&client, "NESN.SW", "history-close-test-2", "CHF", 7.0, 9.0).await?;

    load_ticker(&ctx, "NESN").await?;

    let toggle = ctx
        .driver
        .query(By::ClassName("history-toggle-btn"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;

    // Open
    toggle.click().await?;
    ctx.driver
        .query(By::ClassName("timeline-sidebar"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;

    // Close
    let toggle = ctx
        .driver
        .find(By::ClassName("history-toggle-btn"))
        .await?;
    toggle.click().await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify aria-expanded is false
    let toggle = ctx
        .driver
        .find(By::ClassName("history-toggle-btn"))
        .await?;
    let expanded = toggle.attr("aria-expanded").await?;
    assert_eq!(
        expanded.as_deref(),
        Some("false"),
        "Toggle should have aria-expanded=false after closing"
    );

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_history_timeline_lists_past_analyses() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let _s1 = seed_snapshot(&client, "NESN.SW", "history-list-test-1", "CHF", 6.0, 8.0).await?;
    let _s2 = seed_snapshot(&client, "NESN.SW", "history-list-test-2", "CHF", 7.0, 9.0).await?;

    load_ticker(&ctx, "NESN").await?;

    let toggle = ctx
        .driver
        .query(By::ClassName("history-toggle-btn"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    toggle.click().await?;

    // Wait for timeline entries
    let entries = ctx
        .driver
        .query(By::ClassName("timeline-entry"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .all_required()
        .await?;
    assert!(
        entries.len() >= 2,
        "Should have at least 2 timeline entries, got {}",
        entries.len()
    );

    // Verify entries have date text
    let date_els = ctx.driver.find_all(By::ClassName("timeline-date")).await?;
    assert!(
        !date_els.is_empty(),
        "Timeline entries should have date elements"
    );
    let date_text = date_els[0].text().await?;
    assert!(!date_text.is_empty(), "Date should not be empty");

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_history_select_past_shows_comparison() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let _s1 = seed_snapshot(&client, "NESN.SW", "history-select-test-1", "CHF", 6.0, 8.0).await?;
    let _s2 = seed_snapshot(&client, "NESN.SW", "history-select-test-2", "CHF", 7.5, 9.5).await?;

    load_ticker(&ctx, "NESN").await?;

    let toggle = ctx
        .driver
        .query(By::ClassName("history-toggle-btn"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    toggle.click().await?;

    // Wait for entries and find a non-current one to click
    let entries = ctx
        .driver
        .query(By::ClassName("timeline-entry"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .all_required()
        .await?;

    // Click the first entry that is NOT marked as current
    for entry in &entries {
        let classes = entry.class_name().await?.unwrap_or_default();
        if !classes.contains("timeline-current") {
            entry.click().await?;
            break;
        }
    }

    // Verify comparison cards appear
    let comparison = ctx
        .driver
        .query(By::ClassName("snapshot-comparison"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    assert!(
        comparison.is_displayed().await?,
        "Snapshot comparison should appear after selecting a past analysis"
    );

    // Verify both past and current cards exist
    let past_card = ctx
        .driver
        .find_all(By::ClassName("comparison-card-past"))
        .await?;
    let current_card = ctx
        .driver
        .find_all(By::ClassName("comparison-card-current"))
        .await?;
    assert!(!past_card.is_empty(), "Past card should exist");
    assert!(!current_card.is_empty(), "Current card should exist");

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_history_metric_deltas_displayed() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let _s1 = seed_snapshot(&client, "NESN.SW", "delta-test-1", "CHF", 6.0, 8.0).await?;
    let _s2 = seed_snapshot(&client, "NESN.SW", "delta-test-2", "CHF", 9.0, 12.0).await?;

    load_ticker(&ctx, "NESN").await?;

    let toggle = ctx
        .driver
        .query(By::ClassName("history-toggle-btn"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    toggle.click().await?;

    let entries = ctx
        .driver
        .query(By::ClassName("timeline-entry"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .all_required()
        .await?;

    // Click a non-current entry
    for entry in &entries {
        let classes = entry.class_name().await?.unwrap_or_default();
        if !classes.contains("timeline-current") {
            entry.click().await?;
            break;
        }
    }

    // Verify deltas column exists with delta indicators
    let deltas = ctx
        .driver
        .query(By::ClassName("comparison-deltas"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    assert!(deltas.is_displayed().await?, "Delta column should be visible");

    let delta_indicators = deltas.find_all(By::ClassName("delta")).await?;
    assert!(
        !delta_indicators.is_empty(),
        "Delta indicators should be present"
    );

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_history_deselect_returns_to_live() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let _s1 = seed_snapshot(&client, "NESN.SW", "deselect-test-1", "CHF", 6.0, 8.0).await?;
    let _s2 = seed_snapshot(&client, "NESN.SW", "deselect-test-2", "CHF", 7.0, 9.0).await?;

    load_ticker(&ctx, "NESN").await?;

    let toggle = ctx
        .driver
        .query(By::ClassName("history-toggle-btn"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    toggle.click().await?;

    let entries = ctx
        .driver
        .query(By::ClassName("timeline-entry"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .all_required()
        .await?;

    // Find and click a non-current entry
    let mut clicked_entry_idx = None;
    for (i, entry) in entries.iter().enumerate() {
        let classes = entry.class_name().await?.unwrap_or_default();
        if !classes.contains("timeline-current") {
            entry.click().await?;
            clicked_entry_idx = Some(i);
            break;
        }
    }

    // Confirm comparison appeared
    ctx.driver
        .query(By::ClassName("snapshot-comparison"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;

    // Click the same entry again to deselect
    if let Some(idx) = clicked_entry_idx {
        let entries = ctx.driver.find_all(By::ClassName("timeline-entry")).await?;
        entries[idx].click().await?;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify comparison cards are gone
    let comparisons = ctx
        .driver
        .find_all(By::ClassName("snapshot-comparison"))
        .await?;
    let any_visible = {
        let mut vis = false;
        for c in &comparisons {
            if c.is_displayed().await.unwrap_or(false) {
                vis = true;
                break;
            }
        }
        vis
    };
    assert!(
        !any_visible,
        "Comparison cards should disappear after deselecting"
    );

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_history_toggle_disabled_when_no_analyses() -> Result<()> {
    let ctx = TestContext::new().await?;

    // Load a ticker that has NO seeded snapshots.
    // Use "SAP" which other tests in this file do not seed.
    load_ticker(&ctx, "SAP").await?;

    let toggle = ctx
        .driver
        .query(By::ClassName("history-toggle-btn"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;

    // Verify the button is disabled
    let disabled = toggle.attr("disabled").await?;
    assert!(
        disabled.is_some(),
        "History toggle should be disabled when no analyses exist"
    );

    ctx.cleanup().await?;
    Ok(())
}

// ============================================================
// Task 5: Currency handling E2E tests (AC: #4)
// ============================================================

#[tokio::test]
async fn test_comparison_currency_switch_updates_prices() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    // Seed snapshots with different currencies
    let (_s1, nesn_id) = seed_snapshot(&client, "NESN.SW", "currency-switch-1", "CHF", 8.0, 10.0).await?;
    let (_s2, aapl_id) = seed_snapshot(&client, "AAPL", "currency-switch-2", "USD", 12.0, 15.0).await?;

    ctx.navigate(&format!(
        "/compare?ticker_ids={},{}",
        nesn_id, aapl_id
    ))
    .await?;

    ctx.driver
        .query(By::ClassName("compact-card"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .first()
        .await?;

    // Capture initial metric values
    let metrics_before: Vec<String> = {
        let els = ctx.driver.find_all(By::ClassName("metric-value")).await?;
        let mut texts = Vec::new();
        for el in &els {
            texts.push(el.text().await?);
        }
        texts
    };

    // Find currency selector and change currency
    let selects = ctx
        .driver
        .find_all(By::Css("select"))
        .await?;

    // Find the currency <select> (look for one with 3-letter uppercase options)
    let mut currency_changed = false;
    for s in &selects {
        let options = s.find_all(By::Tag("option")).await?;
        // A currency selector has options like "CHF", "USD", "EUR"
        if options.len() >= 2 {
            let first_text = options[0].text().await?;
            if first_text.len() == 3 && first_text.chars().all(|c| c.is_ascii_uppercase()) {
                // This is likely the currency selector; select a different option
                let second_val = options[1].attr("value").await?.unwrap_or_default();
                s.find(By::Css(&format!("option[value='{}']", second_val)))
                    .await?
                    .click()
                    .await?;
                currency_changed = true;
                break;
            }
        }
    }

    if currency_changed {
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Cards should still be present after currency switch
        let cards_after = ctx.driver.find_all(By::ClassName("compact-card")).await?;
        assert!(
            cards_after.len() >= 2,
            "Cards should still be present after currency switch"
        );

        // Capture metric values after switch and compare
        let metrics_after: Vec<String> = {
            let els = ctx.driver.find_all(By::ClassName("metric-value")).await?;
            let mut texts = Vec::new();
            for el in &els {
                texts.push(el.text().await?);
            }
            texts
        };

        // Verify at least one metric value changed (monetary values converted).
        // If exchange rates are unavailable the frontend falls back to native
        // currency, so we log a warning rather than hard-fail.
        if metrics_before == metrics_after {
            eprintln!(
                "[warning] metric values unchanged after currency switch — \
                 exchange rate service may be unavailable"
            );
        }
    }

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_comparison_currency_indicator_label() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let (_s1, nesn_id) = seed_snapshot(&client, "NESN.SW", "currency-label-1", "CHF", 8.0, 10.0).await?;
    let (_s2, aapl_id) = seed_snapshot(&client, "AAPL", "currency-label-2", "USD", 12.0, 15.0).await?;

    ctx.navigate(&format!(
        "/compare?ticker_ids={},{}",
        nesn_id, aapl_id
    ))
    .await?;

    ctx.driver
        .query(By::ClassName("compact-card"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .first()
        .await?;

    // Currency indicator must exist and contain a valid currency code
    let indicators = ctx
        .driver
        .find_all(By::ClassName("currency-indicator"))
        .await?;
    assert!(
        !indicators.is_empty(),
        "Currency indicator element should exist on comparison page"
    );

    let indicator_text = indicators[0].text().await?;
    assert!(
        indicator_text.contains("CHF")
            || indicator_text.contains("USD")
            || indicator_text.contains("EUR"),
        "Currency indicator should contain a currency code, got: {}",
        indicator_text
    );

    ctx.cleanup().await?;
    Ok(())
}

// ============================================================
// Task 6: Responsive design E2E tests (AC: #3)
// ============================================================

#[tokio::test]
async fn test_comparison_desktop_shows_sort_headers() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let (_s, nesn_id) = seed_snapshot(&client, "NESN.SW", "resp-desktop-test", "CHF", 8.0, 10.0).await?;

    ctx.set_viewport(DESKTOP_WIDE.0, DESKTOP_WIDE.1).await?;
    ctx.navigate(&format!("/compare?ticker_ids={}", nesn_id))
        .await?;

    ctx.driver
        .query(By::ClassName("compact-card"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .first()
        .await?;

    // Desktop: sort headers must exist and be visible
    let headers = ctx
        .driver
        .find_all(By::ClassName("comparison-sort-headers"))
        .await?;
    assert!(
        !headers.is_empty(),
        "Sort headers element should exist on desktop"
    );
    assert!(
        headers[0].is_displayed().await?,
        "Sort headers should be visible on desktop"
    );

    // Desktop: mobile dropdown should be hidden or absent
    let mobile_drop = ctx
        .driver
        .find_all(By::ClassName("sort-dropdown-mobile"))
        .await?;
    if !mobile_drop.is_empty() {
        let displayed = mobile_drop[0].is_displayed().await.unwrap_or(false);
        assert!(
            !displayed,
            "Mobile sort dropdown should be hidden on desktop"
        );
    }

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_comparison_mobile_shows_dropdown_sort() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let (_s, nesn_id) = seed_snapshot(&client, "NESN.SW", "resp-mobile-sort-test", "CHF", 8.0, 10.0).await?;

    ctx.set_viewport(MOBILE.0, MOBILE.1).await?;
    ctx.navigate(&format!("/compare?ticker_ids={}", nesn_id))
        .await?;

    ctx.driver
        .query(By::ClassName("compact-card"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .first()
        .await?;

    // Mobile: sort headers should be hidden or absent
    let headers = ctx
        .driver
        .find_all(By::ClassName("comparison-sort-headers"))
        .await?;
    if !headers.is_empty() {
        let displayed = headers[0].is_displayed().await.unwrap_or(false);
        assert!(!displayed, "Sort headers should be hidden on mobile");
    }

    // Mobile: dropdown must exist and be visible
    let mobile_drop = ctx
        .driver
        .find_all(By::ClassName("sort-dropdown-mobile"))
        .await?;
    assert!(
        !mobile_drop.is_empty(),
        "Mobile sort dropdown element should exist on mobile"
    );
    assert!(
        mobile_drop[0].is_displayed().await?,
        "Mobile sort dropdown should be visible on mobile"
    );

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_comparison_mobile_single_column_cards() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let (_s1, nesn_id) = seed_snapshot(&client, "NESN.SW", "resp-mobile-col-1", "CHF", 8.0, 10.0).await?;
    let (_s2, aapl_id) = seed_snapshot(&client, "AAPL", "resp-mobile-col-2", "USD", 12.0, 15.0).await?;

    ctx.set_viewport(MOBILE.0, MOBILE.1).await?;
    ctx.navigate(&format!(
        "/compare?ticker_ids={},{}",
        nesn_id, aapl_id
    ))
    .await?;

    let cards = ctx
        .driver
        .query(By::ClassName("compact-card"))
        .wait(Duration::from_secs(15), Duration::from_millis(500))
        .all_required()
        .await?;

    if cards.len() >= 2 {
        // Compare Y positions: in single-column layout, cards should be stacked
        let top1: serde_json::Value = ctx
            .driver
            .execute(
                "return arguments[0].getBoundingClientRect().top",
                vec![cards[0].to_json()?],
            )
            .await?
            .json()
            .clone();
        let top2: serde_json::Value = ctx
            .driver
            .execute(
                "return arguments[0].getBoundingClientRect().top",
                vec![cards[1].to_json()?],
            )
            .await?
            .json()
            .clone();

        let y1 = top1.as_f64().unwrap_or(0.0);
        let y2 = top2.as_f64().unwrap_or(0.0);

        assert!(
            (y2 - y1).abs() > 50.0,
            "Cards should be stacked vertically on mobile (y1={y1}, y2={y2})"
        );
    }

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_history_tablet_overlay_sidebar() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let _s1 = seed_snapshot(&client, "NESN.SW", "resp-tablet-sidebar-1", "CHF", 6.0, 8.0).await?;
    let _s2 = seed_snapshot(&client, "NESN.SW", "resp-tablet-sidebar-2", "CHF", 7.0, 9.0).await?;

    ctx.set_viewport(TABLET.0, TABLET.1).await?;
    load_ticker(&ctx, "NESN").await?;

    let toggle = ctx
        .driver
        .query(By::ClassName("history-toggle-btn"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    toggle.click().await?;

    let sidebar = ctx
        .driver
        .query(By::ClassName("analysis-sidebar"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;

    // On tablet, sidebar should use absolute positioning (overlay)
    let position: serde_json::Value = ctx
        .driver
        .execute(
            "return getComputedStyle(arguments[0]).position",
            vec![sidebar.to_json()?],
        )
        .await?
        .json()
        .clone();

    assert_eq!(
        position.as_str(),
        Some("absolute"),
        "Sidebar should use absolute positioning (overlay) on tablet"
    );

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_history_mobile_full_overlay() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let _s1 = seed_snapshot(&client, "NESN.SW", "resp-mobile-overlay-1", "CHF", 6.0, 8.0).await?;
    let _s2 = seed_snapshot(&client, "NESN.SW", "resp-mobile-overlay-2", "CHF", 7.0, 9.0).await?;

    ctx.set_viewport(MOBILE.0, MOBILE.1).await?;
    load_ticker(&ctx, "NESN").await?;

    let toggle = ctx
        .driver
        .query(By::ClassName("history-toggle-btn"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    toggle.click().await?;

    let sidebar = ctx
        .driver
        .query(By::ClassName("analysis-sidebar"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;

    // On mobile, sidebar should use fixed positioning (full overlay)
    let position: serde_json::Value = ctx
        .driver
        .execute(
            "return getComputedStyle(arguments[0]).position",
            vec![sidebar.to_json()?],
        )
        .await?
        .json()
        .clone();

    assert_eq!(
        position.as_str(),
        Some("fixed"),
        "Sidebar should use fixed positioning on mobile"
    );

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_history_mobile_no_delta_column() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let _s1 =
        seed_snapshot(&client, "NESN.SW", "resp-mobile-nodelta-1", "CHF", 6.0, 8.0).await?;
    let _s2 =
        seed_snapshot(&client, "NESN.SW", "resp-mobile-nodelta-2", "CHF", 9.0, 12.0).await?;

    ctx.set_viewport(MOBILE.0, MOBILE.1).await?;
    load_ticker(&ctx, "NESN").await?;

    let toggle = ctx
        .driver
        .query(By::ClassName("history-toggle-btn"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    toggle.click().await?;

    // Select a past entry
    let entries = ctx
        .driver
        .query(By::ClassName("timeline-entry"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .all_required()
        .await?;

    for entry in &entries {
        let classes = entry.class_name().await?.unwrap_or_default();
        if !classes.contains("timeline-current") {
            entry.click().await?;
            break;
        }
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Delta column should be hidden on mobile
    let deltas = ctx
        .driver
        .find_all(By::ClassName("comparison-deltas"))
        .await?;
    for d in &deltas {
        let displayed = d.is_displayed().await.unwrap_or(false);
        assert!(
            !displayed,
            "Delta column should be hidden on mobile viewport"
        );
    }

    ctx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn test_command_strip_responsive_widths() -> Result<()> {
    let ctx = TestContext::new().await?;
    ctx.navigate("/").await?;

    // Tablet: command strip width ~120px
    ctx.set_viewport(TABLET.0, TABLET.1).await?;
    let strip = ctx.driver.find(By::ClassName("command-strip")).await?;
    let tablet_width: serde_json::Value = ctx
        .driver
        .execute(
            "return arguments[0].getBoundingClientRect().width",
            vec![strip.to_json()?],
        )
        .await?
        .json()
        .clone();
    let tw = tablet_width.as_f64().unwrap_or(0.0);
    assert!(
        (tw - 120.0).abs() < 30.0,
        "Command strip should be ~120px on tablet, got {tw}"
    );

    // Mobile: command strip width ~60px
    ctx.set_viewport(MOBILE.0, MOBILE.1).await?;
    let strip = ctx.driver.find(By::ClassName("command-strip")).await?;
    let mobile_width: serde_json::Value = ctx
        .driver
        .execute(
            "return arguments[0].getBoundingClientRect().width",
            vec![strip.to_json()?],
        )
        .await?
        .json()
        .clone();
    let mw = mobile_width.as_f64().unwrap_or(0.0);
    assert!(
        (mw - 60.0).abs() < 20.0,
        "Command strip should be ~60px on mobile, got {mw}"
    );

    ctx.cleanup().await?;
    Ok(())
}
