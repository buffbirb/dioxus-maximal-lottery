use dioxus::prelude::*;

use api::model::{BallotSubmission, PollView};

use crate::Route;
use crate::components::{Modal, ShareSection, TierRanker};

#[component]
pub fn Vote(share_id: String) -> Element {
    let poll = use_resource({
        let share_id = share_id.clone();
        move || {
            let share_id = share_id.clone();
            async move { api::polls::get_poll(share_id).await }
        }
    });

    match &*poll.read() {
        Some(Ok(poll_view)) => rsx! {
            VoteForm { share_id: share_id.clone(), poll: poll_view.clone() }
        },
        Some(Err(err)) => rsx! {
            p { class: "form-error", "Couldn't load this poll: {err}" }
        },
        None => rsx! {
            p { "Loading..." }
        },
    }
}

#[component]
fn VoteForm(share_id: String, poll: PollView) -> Element {
    let tiers = use_signal(Vec::<Vec<i64>>::new);
    let mut submitted = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut submitting = use_signal(|| false);

    if poll.closed {
        return rsx! {
            div { id: "vote",
                h1 { "{poll.title}" }
                p { class: "closed-notice",
                    "Voting closed at {poll.deadline.map(|d| d.to_rfc2822()).unwrap_or_default()}."
                }
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
                    Ok(()) => submitted.set(true),
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
                p { class: "deadline-notice", "Voting closes {deadline.to_rfc2822()}" }
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

            ShareSection { path: "/{share_id}" }

            if submitted() {
                Modal { on_close: move |_| submitted.set(false),
                    h2 { "Vote recorded" }
                    p { "Your ranking has been submitted." }
                    Link {
                        to: Route::Results {
                            share_id: share_id.clone(),
                        },
                        class: "cta-button",
                        "View results"
                    }
                }
            }
        }
    }
}
