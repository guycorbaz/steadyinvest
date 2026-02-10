//! # NAIC Backend
//!
//! Loco-based REST API backend for the NAIC Stock Selection Guide application.
//!
//! ## Module Layout
//!
//! - [`controllers`] — Axum route handlers for all `/api/v1/` endpoints
//! - [`services`]    — Business logic services (harvesting, exchange rates, auditing)
//! - [`models`]      — SeaORM entity wrappers and domain model helpers
//! - [`workers`]     — Background job processors (data downloads)
//! - [`views`]       — Response DTO serialization types
//! - [`middlewares`]  — Request pipeline middleware (IP allowlisting)
//! - [`mailers`]     — Email notification templates
//! - [`initializers`] — Application startup hooks
//! - [`data`]        — Data access utilities

pub mod app;
pub mod controllers;
pub mod data;
pub mod initializers;
pub mod mailers;
pub mod models;
pub mod tasks;
pub mod views;
pub mod workers;
pub mod services;
pub mod middlewares;
