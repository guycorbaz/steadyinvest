//! # SteadyInvest Frontend
//!
//! Leptos 0.8 CSR (Client-Side Rendered) single-page application for
//! interactive NAIC Stock Selection Guide analysis.
//!
//! ## Architecture
//!
//! - [`components`] — Reusable UI components (chart, panels, modals, navigation)
//! - [`pages`]      — Route-level page components (`/`, `/system-monitor`, `/audit-log`)
//! - [`types`]      — Frontend-specific type definitions and DTOs
//! - [`persistence`] — Browser-based file save/load via the File API

use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{components::*, path};

mod components;
use crate::components::footer::Footer;
use crate::components::command_strip::CommandStrip;
mod pages;
pub mod types;
pub mod persistence;

use crate::pages::home::Home;
use crate::pages::system_monitor::SystemMonitor;
use crate::pages::audit_log::AuditLog;
use crate::pages::library::Library;

/// Reactive context for the currently viewed locked analysis ID.
///
/// When `Some(id)`, a locked analysis snapshot with the given database ID is
/// being displayed.  The Command Strip reads this to enable the PDF export
/// action.  Set to `None` when viewing live analysis or no analysis at all.
#[derive(Clone, Copy)]
pub struct ActiveLockedAnalysisId(pub RwSignal<Option<i32>>);

/// Root application component — sets up metadata, the router, and global layout.
#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    // Provide a shared signal so child components (Home, CommandStrip) can
    // communicate which locked analysis (if any) is currently displayed.
    let locked_id = ActiveLockedAnalysisId(RwSignal::new(None));
    provide_context(locked_id);

    view! {
        <Html attr:lang="en" attr:dir="ltr" attr:data-theme="light" />

        // sets the document title
        <Title text="Welcome to Leptos CSR" />

        // injects metadata in the <head> of the page
        <Meta charset="UTF-8" />
        <Meta name="viewport" content="width=device-width, initial-scale=1.0" />

        <Router>
            <CommandStrip />
            <Routes fallback=|| view! { NotFound }>
                <Route path=path!("/") view=Home />
                <Route path=path!("/library") view=Library />
                <Route path=path!("/system-monitor") view=SystemMonitor />
                <Route path=path!("/audit-log") view=AuditLog />
            </Routes>
            <Footer />
        </Router>
    }
}
