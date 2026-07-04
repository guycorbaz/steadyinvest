//! File logging + a panic hook — the deferred ADD15 rotating logs (previously the `tracing` events
//! emitted across the app had no sink at all).
//!
//! Wires every `tracing` event already instrumented in the codebase (keychain / normalize / provider
//! / holdings-read failures, the provider-fetch info line, config load warnings) to a **daily-rotating
//! file** under the OS data dir, and installs a **panic hook** that records the panic message, its
//! location and a captured backtrace to the same log *before* the default stderr behaviour runs — so
//! a crash like the Story-4.4 price-refresh `RefCell already borrowed` is diagnosable from a file,
//! not just a vanished window.
//!
//! Discipline: never blocks launch, never panics itself, never touches the journal. If the OS exposes
//! no data directory the app simply runs without a log file (a line to stderr says so).

use std::path::PathBuf;

/// Initialise file logging and the panic hook. Call once, as the first thing in `main()` (before any
/// work that might log or panic). Returns the log directory when one was set up (for a startup
/// notice); `None` when no OS data directory exists or the directory could not be created.
pub fn init() -> Option<PathBuf> {
    let log_dir = directories::ProjectDirs::from("", "", "steadyinvest")
        .map(|dirs| dirs.data_dir().join("logs"))?;
    if let Err(error) = std::fs::create_dir_all(&log_dir) {
        eprintln!(
            "steadyinvest: log directory {} not created: {error}",
            log_dir.display()
        );
        return None;
    }

    // A daily-rolling file `steadyinvest.log.YYYY-MM-DD` — the date lives in the filename, and within
    // a file the uptime timer (seconds since launch, dependency-free) correlates events. The appender
    // writes blocking (low volume; no `non_blocking` worker/guard to keep alive).
    let appender = tracing_appender::rolling::daily(&log_dir, "steadyinvest.log");
    tracing_subscriber::fmt()
        .with_writer(appender)
        .with_ansi(false)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_max_level(tracing::Level::INFO)
        .init();

    // Log every panic (message + location + backtrace) to the file, then chain the previous hook so
    // the default stderr print is preserved. `force_capture` ignores RUST_BACKTRACE so the trace is
    // always there when it matters.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!(target: "panic", "{info}\n{backtrace}");
        default_hook(info);
    }));

    Some(log_dir)
}
