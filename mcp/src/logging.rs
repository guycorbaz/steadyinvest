//! The MCP server's own log (Story 8.4 AC 12, arch §Phase 4 A12): a daily-rotating
//! `steadyinvest-mcp.log.*` beside the app's log (same directory, its own file prefix — never the
//! app's file). **Stdout carries only JSON-RPC frames**: the subscriber writes to the file only, and
//! the panic hook writes to the log and stderr. No log directory → the server runs without a file
//! (one stderr line), never fails.

/// Initialise the file log and the panic hook; returns whether a log file is in use.
pub fn init() -> bool {
    let Some(log_dir) = steadyinvest_paths::log_dir() else {
        crate::warn_stderr("steadyinvest-mcp: no OS data directory, running without a log file");
        return false;
    };
    if let Err(error) = std::fs::create_dir_all(&log_dir) {
        crate::warn_stderr(&format!(
            "steadyinvest-mcp: log directory {} not created: {error}",
            log_dir.display()
        ));
        return false;
    }
    let appender = tracing_appender::rolling::daily(&log_dir, "steadyinvest-mcp.log");
    let installed = tracing_subscriber::fmt()
        .with_writer(appender)
        .with_ansi(false)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .is_ok();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!(target: "panic", "{info}\n{backtrace}");
        // The default hook prints to stderr — never stdout, which carries JSON-RPC only.
        default_hook(info);
    }));
    installed
}
