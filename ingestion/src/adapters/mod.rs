//! Provider adapters. EODHD (CH/EU+US coverage, Story 3.1) + Twelve Data (price-led second source,
//! Story 7.4). Shared HTTP/JSON plumbing (client, `get_json`, status classification, exact-decimal
//! parsing, per-year high/low reduction) lives in the crate-private [`common`].

pub(crate) mod common;
pub mod eodhd;
pub mod twelvedata;
