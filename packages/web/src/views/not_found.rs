use dioxus::prelude::*;

use crate::Route;

/// Shown in place, at the address the visitor arrived on, when a share link
/// points at a poll the server does not have.
///
/// Deliberately not a redirect to the home page. The usual cause is a link that
/// got truncated or mistyped in transit, and the URL is the only copy of it the
/// visitor still has - navigating away throws out the evidence they need to fix
/// it, and leaves the back button bouncing them straight back off the page.
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

/// The catch-all route. `/:share_id` already absorbs every single-segment path
/// into [`super::Vote`], so what reaches here is the deeper misses: `/create/x`,
/// `/abc123/results/extra`, and anything else with no route of its own.
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

/// A load that failed for a reason a retry could plausibly fix - a dropped
/// connection, a server error - as opposed to [`PollNotFound`].
///
/// The underlying error goes to the console, not the page: `ServerFnError`'s
/// `Display` is a developer string ("error running server function: ...
/// (details: None)") and reads as a crash to everyone else.
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

/// Make the response say what the page says. Both not-found views resolve
/// inside the router's initial SSR chunk, which is the window in which the
/// status is still settable; without this the server would answer 200 while the
/// body explains that nothing is here, and crawlers and link unfurlers believe
/// the status over the prose. A no-op on the client, so client-side navigation
/// to a dead link is unaffected.
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
