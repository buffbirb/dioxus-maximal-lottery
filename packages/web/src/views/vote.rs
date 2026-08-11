use dioxus::prelude::*;

use api::model::{BallotSubmission, PollView};

use crate::Route;
use crate::components::{Confetti, Countdown, Modal, ShareSection, Skeleton, TierRanker};
use crate::nav_cache::PENDING_POLL;
use crate::unsaved_guard::use_unsaved_changes_guard;
use crate::views::{LoadError, NotFound, PollNotFound};

#[component]
pub fn Vote(share_id: String) -> Element {
    // `/:share_id` doubles as the app's catch-all for single-segment URLs, so
    // most of what arrives here - `/about`, `/robots.txt`, a typo - was never a
    // share link. Those are wrong pages, not dead polls, and the shape check
    // settles it without a round trip to the server.
    if !api::domain::is_share_id_shaped(&share_id) {
        return rsx! {
            NotFound { segments: vec![share_id] }
        };
    }

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
    let mut poll = use_server_future({
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
        // A share link for a poll that isn't there is a different situation
        // from a call that didn't complete, and only one of them is worth
        // retrying. Telling them apart keeps a database hiccup from announcing
        // that someone's poll is gone.
        Some(Err(err)) if api::polls::is_not_found(err) => rsx! {
            PollNotFound {}
        },
        Some(Err(err)) => rsx! {
            LoadError {
                what: "this poll",
                error: err.to_string(),
                on_retry: move |_| poll.restart(),
            }
        },
        None => unreachable!("use_server_future resolves before suspense clears"),
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

    use_unsaved_changes_guard(move || !submitted() && !tiers().is_empty());

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
                    disabled: submitting(),
                    onclick: on_submit,
                    if submitting() {
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
