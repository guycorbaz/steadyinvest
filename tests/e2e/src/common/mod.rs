#![allow(dead_code)]
use thirtyfour::prelude::*;
use std::env;
use std::path::Path;
use anyhow::Result;

// Viewport constants for responsive testing.
pub const DESKTOP_WIDE: (u32, u32) = (1440, 900);
pub const DESKTOP_STD: (u32, u32) = (1100, 900);
pub const TABLET: (u32, u32) = (900, 1024);
pub const MOBILE: (u32, u32) = (375, 812);

pub struct TestContext {
    pub driver: WebDriver,
    pub base_url: String,
}

impl TestContext {
    pub async fn new() -> Result<Self> {
        dotenvy::dotenv().ok();

        let mut caps = DesiredCapabilities::chrome();
        // Enable headless mode for CI environments
        if env::var("HEADLESS").unwrap_or_default() == "true" {
            caps.add_chrome_arg("--headless")?;
            caps.add_chrome_arg("--no-sandbox")?;
            caps.add_chrome_arg("--disable-dev-shm-usage")?;
        }
        // Use CHROME_DRIVER_URL from env or default to localhost:9515
        let webdriver_url = env::var("CHROME_DRIVER_URL")
            .unwrap_or_else(|_| "http://localhost:9515".to_string());

        let driver = WebDriver::new(&webdriver_url, caps).await?;

        let base_url = env::var("BASE_URL")
            .unwrap_or_else(|_| "http://localhost:5173".to_string());

        Ok(Self { driver, base_url })
    }

    pub async fn navigate(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        self.driver.goto(&url).await?;
        Ok(())
    }

    /// Save a screenshot for CI diagnostic purposes.
    /// Screenshots are saved to ./screenshots/{test_name}.png.
    /// Failures are logged but do not propagate — screenshot capture
    /// must never block test cleanup.
    pub async fn save_screenshot(&self, test_name: &str) {
        let dir = Path::new("screenshots");
        if !dir.exists() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                eprintln!("[screenshot] Failed to create screenshots dir: {e}");
                return;
            }
        }
        let path = dir.join(format!("{test_name}.png"));
        match self.driver.screenshot(&path).await {
            Ok(_) => eprintln!("[screenshot] Saved: {}", path.display()),
            Err(e) => eprintln!("[screenshot] Failed to capture {test_name}: {e}"),
        }
    }

    /// Resize the browser window to simulate a specific viewport.
    /// Uses `set_window_rect` which sets the outer window size.
    /// In headless mode the viewport closely matches the requested size.
    pub async fn set_viewport(&self, width: u32, height: u32) -> Result<()> {
        self.driver.set_window_rect(0, 0, width, height).await?;
        // Allow CSS media queries to recalculate after resize.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        Ok(())
    }

    /// Restore the browser window to the default desktop-wide size.
    pub async fn reset_viewport(&self) -> Result<()> {
        self.set_viewport(DESKTOP_WIDE.0, DESKTOP_WIDE.1).await
    }

    pub async fn cleanup(self) -> Result<()> {
        // In CI (headless), capture a screenshot before quitting for diagnostics.
        // Uses the Rust test thread name to derive the test function name.
        if env::var("HEADLESS").unwrap_or_default() == "true" {
            let test_name = std::thread::current()
                .name()
                .unwrap_or("unknown")
                .to_string();
            self.save_screenshot(&test_name).await;
        }
        self.driver.quit().await?;
        Ok(())
    }
}
