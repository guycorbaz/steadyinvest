//! Background job processors.
//!
//! Workers run asynchronously via the Loco background queue and handle
//! long-running tasks that should not block API responses.
//!
//! - [`downloader`] — Fetches historical data from financial API providers

pub mod downloader;
