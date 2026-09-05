use dioxus::prelude::*;

use api::model::{BallotSubmission, PollView};

use crate::Route;
use crate::components::{Confetti, Countdown, Modal, ShareSection, Skeleton, TierRanker};
use crate::nav_cache::PENDING_POLL;
use crate::unsaved_guard::use_unsaved_changes_guard;

#[component]
pub fn Vote(share_id: String) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_| rsx! {
                VoteSkeleton {}
            },
            VoteLoader { share_id }
        }
    }
}

#[component]
fn VoteSkeleton() -> Element {
    rsx! {
        div { id: "vote",
            Skeleton { class: "skeleton-title" }
            Skeleton { class: "skeleton-line" }
            Skeleton { class: "skeleton-ranker" }
        }
    }
}

#[component]
fn VoteLoader(share_id: String) -> Element {
    let poll = use_server_future({
        let share_id = share_id.clone();
        move || {
            let share_id = share_id.clone();
            async move {
                if let Some(seeded) = PENDING_POLL.write().take_if(|p| p.share_id == share_id) {
                    return Ok(seeded);
                }
                api::polls::get_poll(share_id).await
            }
        }
    })?;

    match &*poll.read() {
        Some(Ok(poll_view)) => rsx! {
            VoteForm { share_id: share_id.clone(), poll: poll_view.clone() }
        },
        Some(Err(err)) => rsx! {
            p { class: "form-error", "Couldn't load this poll: {err}" }
        },
        None => unreachable!("use_server_future resolves before suspense clears"),
    }
}

/// Whether this session has already voted on the poll, read from the vote
/// cookie.
///
/// `use_server_cached` below runs this on the server and replays the result on
/// the client from the hydration payload, so the branches are not two answers
/// to the same question: the server branch is the one that renders, and the
/// client branch only runs when this component mounts *after* hydration - a
/// client-side navigation from `views::create`, say, which the server never
/// rendered and so has nothing cached for. There `document.cookie` is the
/// whole story, and it carries the same cookie the server branch read off the
/// request headers.
fn session_has_voted(share_id: &str) -> bool {
    // Ordered `server` first to match `use_server_cached` itself: a build with
    // both features on (`--all-features`) is a server build, and calling into
    // `web_sys` off-wasm would be the wrong answer even where it links.
    #[cfg(feature = "server")]
    {
        dioxus::fullstack::FullstackContext::current()
            .map(|ctx| api::cookies::has_voted(&ctx.parts_mut(), share_id))
            .unwrap_or(false)
    }

    // The vote cookie is not `HttpOnly` on purpose (see `api::cookies`), so
    // the client can read it straight off `document.cookie` with no request.
    #[cfg(all(feature = "web", not(feature = "server")))]
    {
        use wasm_bindgen::JsCast;
        web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.dyn_into::<web_sys::HtmlDocument>().ok())
            .and_then(|document| document.cookie().ok())
            .map(|cookies| api::cookies::parse_voted(&cookies, share_id))
            .unwrap_or(false)
    }

    #[cfg(not(any(feature = "server", feature = "web")))]
    {
        false
    }
}

#[component]
fn VoteForm(share_id: String, poll: PollView) -> Element {
    let tiers = use_signal(Vec::<Vec<i64>>::new);
    let mut submitted = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut submitting = use_signal(|| false);
    // Separate from `submitted` on purpose: the modal is dismissible at any
    // moment and the burst should still finish falling if it is.
    let mut celebrating = use_signal(|| false);
    // Set when this session votes: keeps the button disabled after the modal
    // is dismissed, without another server round-trip.
    let mut voted = use_signal(|| false);

    let session_voted = use_server_cached(|| session_has_voted(&share_id));

    let already_voted_local = use_memo(move || voted() || session_voted);

    use_unsaved_changes_guard(move || {
        !submitted() && !already_voted_local() && !tiers().is_empty()
    });

    if poll.closed {
        let closed_reason = match poll.deadline {
            Some(deadline) if deadline <= chrono::Utc::now() => {
                format!("Voting closed at {}.", deadline.to_rfc2822())
            }
            _ => "Voting closed after reaching its vote cap.".to_string(),
        };
        return rsx! {
            div { id: "vote",
                h1 { "{poll.title}" }
                p { class: "closed-notice", "{closed_reason}" }
                Link {
                    to: Route::Results {
                        share_id: share_id.clone(),
                    },
                    class: "cta-button",
                    "View results"
                }
            }
        };
    }

    let on_submit = {
        let share_id = share_id.clone();
        move |_| {
            let share_id = share_id.clone();
            submitting.set(true);
            error.set(None);
            spawn(async move {
                let ballot = BallotSubmission {
                    share_id,
                    tiers: tiers(),
                };
                match api::polls::submit_vote(ballot).await {
                    Ok(()) => {
                        submitted.set(true);
                        celebrating.set(true);
                        voted.set(true);
                    }
                    Err(err) => error.set(Some(err.to_string())),
                }
                submitting.set(false);
            });
        }
    };

    rsx! {
        div { id: "vote",
            h1 { "{poll.title}" }
            if let Some(description) = &poll.description {
                p { class: "subtitle", "{description}" }
            }
            if let Some(deadline) = poll.deadline {
                p { class: "deadline-notice",
                    "Voting closes in "
                    Countdown { deadline, on_elapsed: move |_| {} }
                }
            }
            if let Some(cap) = poll.vote_cap {
                p { class: "deadline-notice", "{poll.vote_count} of {cap} votes" }
            }

            TierRanker { options: poll.options.clone(), tiers }

            if let Some(err) = error() {
                p { class: "form-error", "{err}" }
            }

            div { class: "vote-actions",
                button {
                    class: "cta-button",
                    r#type: "button",
                    disabled: submitting() || already_voted_local(),
                    onclick: on_submit,
                    if already_voted_local() {
                        "Already voted"
                    } else if submitting() {
                        "Submitting..."
                    } else {
                        "Submit vote"
                    }
                }
                Link {
                    to: Route::Results {
                        share_id: share_id.clone(),
                    },
                    class: "secondary-link",
                    "View results"
                }
            }

            if celebrating() {
                Confetti { on_done: move |_| celebrating.set(false) }
            }

            if submitted() {
                Modal { on_close: move |_| submitted.set(false),
                    h2 { "Vote recorded" }
                    Link {
                        to: Route::Results {
                            share_id: share_id.clone(),
                        },
                        class: "cta-button",
                        "View results"
                    }
                    ShareSection { path: "/{share_id}" }
                }
            }
        }
    }
}
