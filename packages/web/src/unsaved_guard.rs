//! Warns before unsaved input is lost. Two escape routes exist, and each
//! needs its own guard:
//!
//! - The tab closing or reloading, which `beforeunload` below can see but
//!   in-app navigation can't trigger - the router never unloads the
//!   document.
//! - In-app navigation (a `Link` click, back/forward), which `nav_guard`
//!   handles by reading `UNSAVED_CHANGES` from the router's `on_update`
//!   callback. That's plain Rust, not a browser event, so it needs the dirty
//!   flag in a signal rather than the JS global `beforeunload` reads.
use dioxus::prelude::*;

/// Mirrors the same dirty flag `beforeunload` reads out of
/// `window.__unsavedChanges`, but reachable from `nav_guard`'s router
/// callback, which runs in plain Rust with no DOM to read from.
pub static UNSAVED_CHANGES: GlobalSignal<bool> = Signal::global(|| false);

pub fn use_unsaved_changes_guard(dirty: impl Fn() -> bool + 'static) {
    use_effect(move || {
        let is_dirty = dirty();
        *UNSAVED_CHANGES.write() = is_dirty;
        spawn(async move {
            let script = format!(
                "window.__unsavedChanges = {is_dirty};
                 if (!window.__unsavedChangesHandlerInstalled) {{
                     window.__unsavedChangesHandlerInstalled = true;
                     window.addEventListener('beforeunload', (e) => {{
                         if (window.__unsavedChanges) {{
                             e.preventDefault();
                         }}
                     }});
                 }}"
            );
            let _ = document::eval(&script).join::<()>().await;
        });
    });
}
