//! The MCP server's own log (Story 8.4 AC 12, arch §Phase 4 A12): a daily-rotating
//! `steadyinvest-mcp.log.*` beside the app's log (same directory, its own file prefix — never the
//! app's file). **Stdout carries only JSON-RPC frames**: the subscriber writes to the file only, and
//! the panic hook writes to the log and stderr.
//!
//! **No AI text in the log (G3):** the server logs codes, tool names and field keys only — never a
//! message, a comment, a proposed value or a refusal's data (an AI-written newline would forge a
//! log line). The SDK's own events are kept to errors (`rmcp` target), so a client-sent method or
//! tool name is never echoed either.
//!
//! No usable log directory (none, or not writable) → the server runs without a file (one stderr
//! line), never fails.

use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Initialise the panic hook, then the file log; returns whether a log file is in use.
pub fn init() -> bool {
    install_panic_hook();
    let Some(log_dir) = steadyinvest_paths::log_dir() else {
        crate::warn_stderr("steadyinvest-mcp: no OS data directory, running without a log file");
        return false;
    };
    if let Err(error) = std::fs::create_dir_all(&log_dir) {
        crate::warn_stderr(&format!(
            "steadyinvest-mcp: log directory {} not created ({error}); running without a log file",
            log_dir.display()
        ));
        return false;
    }
    let appender = match tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("steadyinvest-mcp.log")
        .build(&log_dir)
    {
        Ok(a) => a,
        Err(error) => {
            crate::warn_stderr(&format!(
                "steadyinvest-mcp: log file in {} not usable ({error}); running without a log file",
                log_dir.display()
            ));
            return false;
        }
    };
    let filter = Targets::new()
        .with_default(LevelFilter::INFO)
        .with_target("rmcp", LevelFilter::ERROR);
    let layer = tracing_subscriber::fmt::layer()
        .with_writer(appender)
        .with_ansi(false)
        .with_timer(tracing_subscriber::fmt::time::uptime());
    tracing_subscriber::registry()
        .with(layer)
        .with(filter)
        .try_init()
        .is_ok()
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!(target: "panic", "{info}\n{backtrace}");
        // The default hook prints to stderr — never stdout, which carries JSON-RPC only.
        default_hook(info);
    }));
}
