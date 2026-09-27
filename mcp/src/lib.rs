//! `steadyinvest-mcp` — the local MCP stdio server of SteadyInvest (Story 8.4, Epic 8 [P4],
//! arch §Phase 4 A1–A12).
//!
//! The owner's AI client launches this binary over stdio. Through it, the AI reads the dossier's
//! studies (with their computed outputs) and deposits **propositions** (drafts) that change nothing
//! until the owner validates them in the app. The portfolio, the watchlist, the keys and the
//! configuration are never exposed; no provider call is reachable (the crate's dependency closure
//! excludes every network and keychain crate — `tests/closure.rs`).
//!
//! Stdout carries JSON-RPC only (`print_stdout` is denied); logs go to the server's own file.

#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod dto;
pub mod logging;
pub mod messages;
pub mod seed_guard;
pub mod server;
pub mod tools;

/// One diagnostic line to stderr — the only place the server writes outside JSON-RPC and its log.
#[expect(
    clippy::print_stderr,
    reason = "stderr is the diagnostic channel of a stdio server; stdout is JSON-RPC only"
)]
pub fn warn_stderr(line: &str) {
    eprintln!("{line}");
}
