//! Warns before unsaved input is lost. Three escape routes exist:
//!
//! - Tab close/reload or leaving the document: handled by `beforeunload`.
//! - In-app router navigation (`Link` clicks, programmatic): handled by `nav_guard`.
//! - Browser back/forward inside the app: a `popstate` the router does not see,
//!   handled by the `popstate` listener below.
use dioxus::prelude::*;

// Shared with `nav_guard` so the router-side prompt and the browser-side one
// read identically. Only the `web` build ever prompts.
#[cfg(feature = "web")]
macro_rules! confirm_message {
    () => {
        "Leave this page? Your changes haven't been saved."
    };
}
#[cfg(feature = "web")]
pub(crate) use confirm_message;

pub static UNSAVED_CHANGES: GlobalSignal<bool> = Signal::global(|| false);

#[cfg(feature = "web")]
pub mod guards {
    use std::cell::{Cell, RefCell};

    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::Closure;

    // The browser fires these listeners outside any Dioxus scope, where reading
    // `UNSAVED_CHANGES` is not available, so the two facts they need are
    // mirrored here. wasm is single-threaded, so `thread_local` is just "a
    // global" - this is the Rust-side stand-in for what used to live on
    // `window` as `__unsavedChanges` / `__guardedPath`.
    thread_local! {
        static DIRTY: Cell<bool> = const { Cell::new(false) };
        static GUARDED_PATH: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    pub(super) fn set_state(dirty: bool, path: Option<String>) {
        DIRTY.set(dirty);
        GUARDED_PATH.with_borrow_mut(|p| *p = path);
    }

    pub(super) fn current_pathname() -> Option<String> {
        web_sys::window().and_then(|w| w.location().pathname().ok())
    }

    /// Registers the `beforeunload` and `popstate` listeners. Call before
    /// `dioxus::launch` so the `popstate` handler is registered ahead of the
    /// router's own - a declined navigation has to stop propagation before the
    /// router acts on the event.
    pub fn install() {
        let Some(window) = web_sys::window() else {
            return;
        };

        // Both closures live for as long as the page does, so they are leaked
        // rather than kept in a handle nothing would ever drop.
        let before_unload = Closure::<dyn FnMut(web_sys::Event)>::new(|evt: web_sys::Event| {
            if DIRTY.get() {
                // The browser supplies its own wording; all a handler can do is
                // ask for the prompt.
                evt.prevent_default();
            }
        });
        let _ = window.add_event_listener_with_callback(
            "beforeunload",
            before_unload.as_ref().unchecked_ref(),
        );
        before_unload.forget();

        let popstate = Closure::<dyn FnMut(web_sys::Event)>::new(|evt: web_sys::Event| {
            if !DIRTY.get() {
                return;
            }
            let Some(guarded) = GUARDED_PATH.with_borrow(Clone::clone) else {
                return;
            };
            let Some(window) = web_sys::window() else {
                return;
            };
            // A popstate that lands back on the guarded page is not a way out
            // of it, so there is nothing to confirm.
            match window.location().pathname() {
                Ok(current) if current != guarded => {}
                _ => return,
            }
            if window
                .confirm_with_message(confirm_message!())
                .unwrap_or(true)
            {
                return;
            }
            evt.stop_immediate_propagation();
            // The history entry is already gone by the time popstate fires, so
            // declining means putting the guarded path back on the stack.
            if let Ok(history) = window.history() {
                let state = history.state().unwrap_or(wasm_bindgen::JsValue::NULL);
                let _ = history.push_state_with_url(&state, "", Some(&guarded));
            }
        });
        let _ =
            window.add_event_listener_with_callback("popstate", popstate.as_ref().unchecked_ref());
        popstate.forget();
    }
}

pub fn use_unsaved_changes_guard(dirty: impl Fn() -> bool + 'static) {
    use_effect(move || {
        let is_dirty = dirty();
        *UNSAVED_CHANGES.write() = is_dirty;
        #[cfg(feature = "web")]
        guards::set_state(is_dirty, guards::current_pathname());
    });
    use_drop(mark_clean);
}

pub fn mark_clean() {
    *UNSAVED_CHANGES.write() = false;
    #[cfg(feature = "web")]
    guards::set_state(false, None);
}
