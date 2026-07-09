use dioxus::prelude::*;

use crate::Route;
use crate::components::{self, *};
use api::*;

#[component]
pub fn Vote(share_id: String) -> Element {
    let mut poll = use_signal(|| None::<PollView>);
    let mut error = use_signal(|| None::<String>);
    let mut show_modal = use_signal(|| false);
    let mut submitting = use_signal(|| false);
    let mut current_tiers = use_signal(|| TierRankerOutput {
        tiers: vec![],
        unranked: vec![],
    });

    let sid = share_id.clone();
    use_effect(move || {
        let sid = sid.clone();
        spawn(async move {
            match get_poll(sid).await {
                Ok(p) => poll.set(Some(p)),
                Err(e) => error.set(Some(format!("Failed to load poll: {e}"))),
            }
        });
    });

    let submit = {
        let sid = share_id.clone();
        move |_| {
            let tiers = current_tiers.read().tiers.clone();
            let sid = sid.clone();
            submitting.set(true);
            spawn(async move {
                let submission = BallotSubmission {
                    share_id: sid,
                    tiers,
                };
                match submit_vote(submission).await {
                    Ok(()) => {
                        show_modal.set(true);
                        submitting.set(false);
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to submit: {e}")));
                        submitting.set(false);
                    }
                }
            });
        }
    };

    let origin = components::current_origin();
    let share_url = format!("{}/{}", origin, share_id);

    let content = match &*poll.read() {
        Some(p) if p.closed => {
            let deadline_text = p
                .deadline
                .map(|d| format!("{}", d.format("%Y-%m-%d %H:%M UTC")))
                .unwrap_or_default();
            rsx! {
                h1 { "{p.title}" }
                if let Some(ref desc) = p.description {
                    p { class: "poll-description", "{desc}" }
                }
                if p.deadline.is_some() {
                    p { class: "poll-deadline", "Deadline: {deadline_text}" }
                }
                div { class: "closed-notice",
                    p { "This poll closed at {deadline_text}" }
                    Link {
                        to: Route::Results {
                            share_id: share_id.clone(),
                        },
                        class: "btn-link",
                        "View Results"
                    }
                }
                ShareSection {
                    url: share_url.clone(),
                    label: String::from("Share this poll:"),
                }
            }
        }
        Some(p) => {
            let deadline_text = p
                .deadline
                .map(|d| format!("{}", d.format("%Y-%m-%d %H:%M UTC")))
                .unwrap_or_default();
            let options_info: Vec<OptionInfo> = p
                .options
                .iter()
                .map(|o| OptionInfo {
                    id: o.id,
                    label: o.label.clone(),
                })
                .collect();
            rsx! {
                h1 { "{p.title}" }
                if let Some(ref desc) = p.description {
                    p { class: "poll-description", "{desc}" }
                }
                if p.deadline.is_some() {
                    p { class: "poll-deadline", "Deadline: {deadline_text}" }
                }

                TierRanker {
                    options: options_info,
                    onchange: move |output: TierRankerOutput| {
                        current_tiers.set(output);
                    },
                }

                div { class: "vote-actions",
                    button {
                        class: "submit-btn",
                        onclick: submit,
                        disabled: *submitting.read(),
                        if submitting() {
                            "Submitting..."
                        } else {
                            "Submit Vote"
                        }
                    }
                    Link {
                        to: Route::Results {
                            share_id: share_id.clone(),
                        },
                        class: "btn-link",
                        "View Results"
                    }
                }

                ShareSection {
                    url: share_url.clone(),
                    label: String::from("Share this poll:"),
                }
            }
        }
        None => {
            rsx! {
                p { "Loading poll..." }
            }
        }
    };

    rsx! {
        div { class: "vote-page",
            if let Some(ref err) = *error.read() {
                div { class: "error-banner", "{err}" }
            }
            {content}
        }

        Modal { show: show_modal(), onclose: move |_| show_modal.set(false),
            div { class: "success-modal",
                h2 { "Vote Recorded!" }
                p { "Your vote has been successfully submitted." }
                Link {
                    to: Route::Results {
                        share_id: share_id.clone(),
                    },
                    class: "btn-link",
                    "View Results"
                }
            }
        }
    }
}
