//! steadyinvest-app — thin Slint desktop UI (binary).
//!
//! Charts will be drawn natively in Slint (`Path` + `TouchArea`); there is no web view and no egui.
//! This scaffold entry point opens a placeholder window. The application shell, faithful study
//! screen, app-config (`directories`), OS keychain (`keyring`) and provider fetch wiring arrive in
//! Epic 2 / Epic 3.

// The internal crates and platform integrations are wired up in later stories; declared here so the
// dependency graph and crate boundaries are fixed from story one.
#![allow(unused_crate_dependencies)]

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let window = MainWindow::new()?;
    window.run()
}
