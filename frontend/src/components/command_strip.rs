use leptos::prelude::*;
use leptos_router::components::A;

/// Command Strip - Persistent vertical navigation sidebar
///
/// Provides access to all main application features in an "Institutional HUD" style.
/// Implements the UX Design specification for persistent navigation.
#[component]
pub fn CommandStrip() -> impl IntoView {
    view! {
        <nav class="command-strip">
            <div class="strip-header">
                <h1 class="strip-title">"NAIC"</h1>
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
