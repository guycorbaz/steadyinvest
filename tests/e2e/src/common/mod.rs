#![allow(dead_code)]
use thirtyfour::prelude::*;
use std::env;
use anyhow::Result;

pub struct TestContext {
    pub driver: WebDriver,
    pub base_url: String,
}

impl TestContext {
    pub async fn new() -> Result<Self> {
        dotenvy::dotenv().ok();
        
        let caps = DesiredCapabilities::chrome();
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

    pub async fn cleanup(self) -> Result<()> {
        self.driver.quit().await?;
        Ok(())
    }
}
