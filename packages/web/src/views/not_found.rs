use dioxus::prelude::*;

use crate::Route;

/// Shown at the URL the visitor arrived on, for a share link whose
/// poll is gone.
///
/// Not a redirect: the broken URL is the visitor's only copy of it,
/// probably truncated or mistyped in transit. Redirecting away loses
/// that evidence and traps the back button on this same page.
#[component]
pub fn PollNotFound() -> Element {
    use_not_found_status();

    rsx! {
        div { id: "not-found", class: "message-page",
            h1 { "This poll doesn't exist" }
            p { class: "subtitle", "The link may be incomplete, mistyped, or no longer live." }
            NotFoundActions {}
        }
    }
}

/// The catch-all. /:share_id already absorbs every single-segment
/// path into Vote, so this only sees deeper misses: /create/x,
/// /abc123/results/extra, anything with no route of its own.
#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    use_not_found_status();

    let path = segments.join("/");

    rsx! {
        div { id: "not-found", class: "message-page",
            h1 { "This page doesn't exist" }
            p { class: "subtitle", "Nothing lives at /{path}." }
            NotFoundActions {}
        }
    }
}

/// Failed for a reason a retry could fix - dropped connection,
/// server error - unlike PollNotFound.
///
/// Error goes to the console, not the page: ServerFnError's Display
/// is a raw string ("error running server function: ... (details:
/// None)") that reads as a crash to anyone else.
#[component]
pub fn LoadError(what: String, error: String, on_retry: EventHandler<()>) -> Element {
    use_hook({
        let what = what.clone();
        let error = error.clone();
        move || error!("failed to load {what}: {error}")
    });

    rsx! {
        div { id: "load-error", class: "message-page",
            h1 { "Something went wrong" }
            p { class: "subtitle",
                "We couldn't load {what}. This is usually temporary - check your connection "
                "and try again."
            }
            div { class: "message-page-actions",
                button { class: "cta-button", onclick: move |_| on_retry.call(()), "Try again" }
                Link { to: Route::Home {}, class: "secondary-link", "Go home" }
            }
        }
    }
}

/// Sets the HTTP status to match the page. Must run inside the
/// router's initial SSR chunk - the only window it's still settable -
/// or the server answers 200 while the page says "not found," and
/// crawlers trust the status over the prose. No-op on the client.
fn use_not_found_status() {
    use_hook(|| {
        dioxus_fullstack::FullstackContext::commit_http_status(StatusCode::NOT_FOUND, None);
    });
}

#[component]
fn NotFoundActions() -> Element {
    rsx! {
        div { class: "message-page-actions",
            Link { to: Route::Create {}, class: "cta-button", "Create a poll" }
            Link { to: Route::Home {}, class: "secondary-link", "Go home" }
        }
    }
}
