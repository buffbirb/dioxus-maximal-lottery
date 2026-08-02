//! Warns before unsaved input is lost. Three escape routes exist, and each
//! needs its own guard:
//!
//! - The tab closing, reloading, or leaving for another document, which the
//!   `beforeunload` listener below can see.
//! - In-app navigation through the router (a `Link` click, programmatic
//!   nav), which `nav_guard` handles from the router's `on_update` callback.
//! - The browser's back/forward buttons within the app - a same-document
//!   `popstate`, which `on_update` never sees: the router's own popstate
//!   listener only schedules a re-render, and `on_update` runs solely for
//!   router-initiated navigations. The `popstate` listener below covers it.
use dioxus::prelude::*;

/// The one prompt every guard shows, so declining reads the same whether the
/// escape route was a `Link` click (Rust) or a back press (JS).
pub const CONFIRM_MESSAGE: &str = "Leave this page? Your changes haven't been saved.";

/// Mirrors the same dirty flag the JS listeners read out of
/// `window.__unsavedChanges`, but reachable from `nav_guard`'s router
/// callback, which runs in plain Rust with no DOM to read from.
pub static UNSAVED_CHANGES: GlobalSignal<bool> = Signal::global(|| false);

/// Installs the `beforeunload` and `popstate` listeners. Render as an
/// inline `<head>` script (a `document::Script` in the app root), which runs
/// while the HTML parses - before the wasm loads, and so before the router
/// registers its own popstate listener. Registration order is what lets a
/// declined back/forward press call `stopImmediatePropagation()` before the
/// router's listener schedules a re-render - otherwise that re-render can
/// slip in (listeners are separated by a microtask checkpoint) and paint the
/// page being left before the URL is restored.
///
/// The prompt text is `CONFIRM_MESSAGE`; a test asserts the two stay in
/// sync, since a `const` can't be spliced into this literal.
pub const INSTALL_GUARDS_JS: &str = r#"
window.addEventListener('beforeunload', (e) => {
    if (window.__unsavedChanges) {
        e.preventDefault();
    }
});
window.addEventListener('popstate', (e) => {
    if (window.__unsavedChanges && window.__guardedPath
        && window.location.pathname !== window.__guardedPath
        && !window.confirm("Leave this page? Your changes haven't been saved.")) {
        e.stopImmediatePropagation();
        history.pushState(history.state, '', window.__guardedPath);
    }
});
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_script_prompts_with_confirm_message() {
        assert!(INSTALL_GUARDS_JS.contains(&format!("window.confirm({CONFIRM_MESSAGE:?})")));
    }
}

pub fn use_unsaved_changes_guard(dirty: impl Fn() -> bool + 'static) {
    use_effect(move || {
        let is_dirty = dirty();
        *UNSAVED_CHANGES.write() = is_dirty;
        spawn(async move {
            let script = format!(
                "window.__unsavedChanges = {is_dirty};
                 window.__guardedPath = window.location.pathname;"
            );
            let _ = document::eval(&script).join::<()>().await;
        });
    });

    // The listeners reading those flags live in `INSTALL_GUARDS_JS`, run
    // from the document head - not here - so they exist on every page from
    // before the router does.

    // A confirmed navigation unmounts the page, and nothing above re-runs to
    // notice - without this, the flags stay armed and every later tab close
    // or back press anywhere in the app warns about input that no longer
    // exists.
    use_drop(mark_clean);
}

/// Disarms every guard immediately - unlike flipping the signals feeding
/// `use_unsaved_changes_guard`, whose effect only re-runs on the *next*
/// render. Call this before a programmatic navigation that follows a
/// successful save, which would otherwise still see the dirty flag and ask
/// about leaving input that was just submitted.
pub fn mark_clean() {
    *UNSAVED_CHANGES.write() = false;
    let _ = document::eval("window.__unsavedChanges = false; window.__guardedPath = null;");
}
