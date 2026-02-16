use crate::ActiveLockedAnalysisId;
use leptos::prelude::*;
use leptos_router::components::A;

/// Command Strip - Persistent vertical navigation sidebar
///
/// Provides access to all main application features in an "Institutional HUD" style.
/// Implements the UX Design specification for persistent navigation.
///
/// The "Report" menu item is contextual: it is enabled when a locked analysis
/// snapshot is being viewed (via [`ActiveLockedAnalysisId`] context) and
/// triggers a PDF export download.  When no snapshot is active the item is
/// rendered in a disabled ghost style.
#[component]
pub fn CommandStrip() -> impl IntoView {
    let locked_ctx = use_context::<ActiveLockedAnalysisId>();

    let has_locked = move || {
        locked_ctx
            .map(|ctx| ctx.0.get().is_some())
            .unwrap_or(false)
    };

    let on_export = move |_| {
        if let Some(ctx) = locked_ctx {
            if let Some(id) = ctx.0.get() {
                let url = format!("/api/analyses/export/{}", id);
                if let Err(e) = web_sys::window()
                    .expect("no global window")
                    .location()
                    .set_href(&url)
                {
                    web_sys::console::error_1(
                        &format!("PDF export navigation failed: {:?}", e).into(),
                    );
                }
            }
        }
    };

    view! {
        <nav class="command-strip">
            <div class="strip-header">
                <h1 class="strip-title">"SteadyInvest"</h1>
                <span class="strip-subtitle">"Stock Analysis"</span>
            </div>

            <ul class="strip-menu">
                <li class="menu-item">
                    <div class="menu-link">
                        <A href="/">
                            <span class="menu-icon">"🔍"</span>
                            <span class="menu-label">"Search"</span>
                        </A>
                    </div>
                </li>

                <li class="menu-item">
                    <div class="menu-link">
                        <A href="/library">
                            <span class="menu-icon">"📚"</span>
                            <span class="menu-label">"Library"</span>
                        </A>
                    </div>
                </li>

                <li class="menu-item">
                    <div class="menu-link">
                        <A href="/compare">
                            <span class="menu-icon">"⚖"</span>
                            <span class="menu-label">"Comparison"</span>
                        </A>
                    </div>
                </li>

                <li class="menu-divider"></li>

                <li class="menu-section-title">"Report"</li>

                <li class="menu-item">
                    <button
                        class=move || if has_locked() { "menu-link menu-action" } else { "menu-link menu-action disabled" }
                        disabled=move || !has_locked()
                        on:click=on_export
                        aria-label="Export analysis as PDF"
                    >
                        <span class="menu-icon">"📄"</span>
                        <span class="menu-label">"Export PDF"</span>
                    </button>
                </li>

                <li class="menu-divider"></li>

                <li class="menu-section-title">"Admin"</li>

                <li class="menu-item">
                    <div class="menu-link">
                        <A href="/system-monitor">
                            <span class="menu-icon">"📊"</span>
                            <span class="menu-label">"System Monitor"</span>
                        </A>
                    </div>
                </li>

                <li class="menu-item">
                    <div class="menu-link">
                        <A href="/audit-log">
                            <span class="menu-icon">"📋"</span>
                            <span class="menu-label">"Audit Log"</span>
                        </A>
                    </div>
                </li>
            </ul>

            <div class="strip-footer">
                <span class="version">"v1.0"</span>
            </div>
        </nav>
    }
}
