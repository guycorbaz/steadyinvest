//! `steadyinvest-mcp` — launched by the owner's AI client over stdio (Story 8.4).
//!
//! Usage: `steadyinvest-mcp [--dossier <path>]` · `steadyinvest-mcp --version`.
//! Without `--dossier`, each call resolves the dossier the app last opened (arch §Phase 4 A10).
//! During Epic 8, always pass `--dossier` explicitly (docs/mcp-registration.md).

#![deny(clippy::print_stdout, clippy::print_stderr)]

use rmcp::ServiceExt;
use std::path::PathBuf;
use std::process::ExitCode;
use steadyinvest_mcp::{logging, server::SteadyMcp, warn_stderr};

/// What the command line asks for.
enum Command {
    Serve { dossier: Option<PathBuf> },
    Version,
}

fn parse_args(args: impl Iterator<Item = std::ffi::OsString>) -> Result<Command, String> {
    let mut dossier: Option<PathBuf> = None;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--dossier") => {
                if dossier.is_some() {
                    return Err("--dossier given twice".to_string());
                }
                let Some(value) = args.next() else {
                    return Err("--dossier needs a path".to_string());
                };
                let path = PathBuf::from(value);
                let path = if path.is_relative() {
                    std::env::current_dir()
                        .map_err(|e| format!("current directory unreadable: {e}"))?
                        .join(path)
                } else {
                    path
                };
                dossier = Some(path);
            }
            Some("--version") => return Ok(Command::Version),
            _ => {
                return Err(format!(
                    "unknown argument {}; usage: steadyinvest-mcp [--dossier <path>] | --version",
                    arg.to_string_lossy()
                ));
            }
        }
    }
    Ok(Command::Serve { dossier })
}

fn main() -> ExitCode {
    let dossier = match parse_args(std::env::args_os().skip(1)) {
        Ok(Command::Version) => {
            warn_stderr(concat!("steadyinvest-mcp ", env!("CARGO_PKG_VERSION")));
            return ExitCode::SUCCESS;
        }
        Ok(Command::Serve { dossier }) => dossier,
        Err(message) => {
            warn_stderr(&format!("steadyinvest-mcp: {message}"));
            return ExitCode::from(2);
        }
    };
    logging::init();
    tracing::info!(
        explicit_dossier = ?dossier,
        version = env!("CARGO_PKG_VERSION"),
        "steadyinvest-mcp starting"
    );
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            warn_stderr(&format!("steadyinvest-mcp: runtime not started: {e}"));
            return ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(async move {
        let service = SteadyMcp::new(dossier)
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| format!("initialisation failed: {e}"))?;
        service
            .waiting()
            .await
            .map_err(|e| format!("server stopped: {e}"))?;
        Ok::<(), String>(())
    });
    match result {
        Ok(()) => {
            tracing::info!("steadyinvest-mcp stopped");
            ExitCode::SUCCESS
        }
        Err(message) => {
            // The detail (possibly from client input) goes to stderr only, not to the log (G3).
            tracing::error!("steadyinvest-mcp stopped with an error");
            warn_stderr(&format!("steadyinvest-mcp: {message}"));
            ExitCode::FAILURE
        }
    }
}
