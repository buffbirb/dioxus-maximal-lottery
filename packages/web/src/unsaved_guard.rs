//! Warns before the tab closes or reloads while unsaved input exists. Does
//! not intercept in-app back/forward navigation - the router never unloads
//! the document, so `beforeunload` can't see that case.
use dioxus::prelude::*;

pub fn use_unsaved_changes_guard(dirty: impl Fn() -> bool + 'static) {
    use_effect(move || {
        let is_dirty = dirty();
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
