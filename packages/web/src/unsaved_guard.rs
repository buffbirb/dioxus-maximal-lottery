//! Warns before unsaved input is lost. Three escape routes exist:
//!
//! - Tab close/reload or leaving the document: handled by `beforeunload`.
//! - In-app router navigation (`Link` clicks, programmatic): handled by `nav_guard`.
//! - Browser back/forward inside the app: a `popstate` the router does not see,
//!   handled by the `popstate` listener below.
use dioxus::prelude::*;

macro_rules! confirm_message {
    () => {
        "Leave this page? Your changes haven't been saved."
    };
}
#[cfg(feature = "web")]
pub(crate) use confirm_message;

pub static UNSAVED_CHANGES: GlobalSignal<bool> = Signal::global(|| false);

pub const INSTALL_GUARDS_JS: &str = concat!(
    r#"
window.addEventListener('beforeunload', (e) => {
    if (window.__unsavedChanges) {
        e.preventDefault();
    }
});
window.addEventListener('popstate', (e) => {
    if (window.__unsavedChanges && window.__guardedPath
        && window.location.pathname !== window.__guardedPath
        && !window.confirm(""#,
    confirm_message!(),
    r#"")) {
        e.stopImmediatePropagation();
        history.pushState(history.state, '', window.__guardedPath);
    }
});
"#
);

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
    use_drop(mark_clean);
}

pub fn mark_clean() {
    *UNSAVED_CHANGES.write() = false;
    let _ = document::eval("window.__unsavedChanges = false; window.__guardedPath = null;");
}
