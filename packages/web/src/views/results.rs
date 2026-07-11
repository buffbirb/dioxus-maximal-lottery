use dioxus::prelude::*;

use api::model::{HeadToHead, ResultsView};

use crate::Route;
use crate::components::{Countdown, Modal, OptionChip, ShareSection};

#[component]
pub fn Results(share_id: String) -> Element {
    let mut refresh = use_signal(|| 0u32);
    let results = use_resource({
        let share_id = share_id.clone();
        move || {
            let share_id = share_id.clone();
            let _ = refresh();
            async move { api::polls::get_results(share_id).await }
        }
    });

    match &*results.read() {
        Some(Ok(view)) => rsx! {
            ResultsBody {
                share_id: share_id.clone(),
                view: view.clone(),
                on_reveal: move |_| refresh += 1,
            }
        },
        Some(Err(err)) => rsx! {
            p { class: "form-error", "Couldn't load results: {err}" }
        },
        None => rsx! {
            p { "Loading..." }
        },
    }
}

#[component]
fn ResultsBody(share_id: String, view: ResultsView, on_reveal: EventHandler<()>) -> Element {
    let mut selected = use_signal(|| None::<(String, Vec<HeadToHead>)>);

    if !view.results_visible {
        let deadline = view
            .deadline
            .expect("hidden results always carry a deadline");
        return rsx! {
            div { id: "results",
                h1 { "{view.title}" }
                p { class: "hidden-results-notice", "Results are hidden until the poll closes." }
                p { class: "deadline-notice", "Reveals {deadline.to_rfc2822()}" }
                div { class: "countdown-row",
                    Countdown { deadline, on_elapsed: move |_| on_reveal.call(()) }
                }
                Link {
                    to: Route::Vote {
                        share_id: share_id.clone(),
                    },
                    class: "cta-button",
                    "Go vote"
                }
            }
        };
    }

    rsx! {
        div { id: "results",
            h1 { "{view.title}" }
            if let Some(description) = &view.description {
                p { class: "subtitle", "{description}" }
            }
            p { class: "vote-count", "{view.vote_count} votes cast" }

            div { class: "live-row",
                if let Some(deadline) = view.deadline {
                    if view.closed {
                        span { class: "deadline-notice", "Closed at {deadline.to_rfc2822()}" }
                    } else {
                        span { class: "live-dot" }
                        Countdown {
                            deadline,
                            on_elapsed: move |_| on_reveal.call(()),
                        }
                    }
                } else {
                    span { class: "live-dot" }
                    span { "Live" }
                }
            }

            match &view.winner {
                Some(winner) => rsx! {
                    p { class: "winner-line",
                        if view.closed {
                            "Winner: "
                        } else {
                            "Current winner: "
                        }
                        strong { "{winner}" }
                    }
                },
                None => rsx! {
                    p { class: "winner-line", "No single winner yet - see standings below." }
                },
            }

            if !view.closed {
                Link {
                    to: Route::Vote {
                        share_id: share_id.clone(),
                    },
                    class: "secondary-link",
                    "Go vote"
                }
            }

            div { class: "standings",
                for slot in view.standings.clone() {
                    div { class: "standing-slot", key: "{slot.rank_label}",
                        span { class: "standing-rank", "{slot.rank_label}" }
                        div { class: "standing-members",
                            for member in slot.members.clone() {
                                OptionChip {
                                    key: "{member.option_id}",
                                    label: member.label.clone(),
                                    percentage: member.probability_pct.clone(),
                                    onclick: {
                                        let share_id = share_id.clone();
                                        let option_id = member.option_id;
                                        let label = member.label.clone();
                                        move |_| {
                                            let share_id = share_id.clone();
                                            let label = label.clone();
                                            spawn(async move {
                                                let response = api::polls::get_head_to_head(share_id, option_id).await;
                                                if let Ok(margins) = response {
                                                    selected.set(Some((label, margins)));
                                                }
                                            });
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
            }

            ShareSection { path: if view.closed { format!("/{share_id}/results") } else { format!("/{share_id}") } }

            if let Some((label, margins)) = selected() {
                Modal { on_close: move |_| selected.set(None),
                    h2 { "{label}" }
                    p { class: "modal-subtitle", "Head-to-head margins" }
                    div { class: "head-to-head-list",
                        for m in margins {
                            div {
                                key: "{m.option_id}",
                                class: if m.margin > 0 { "h2h-row h2h-win" } else if m.margin < 0 { "h2h-row h2h-loss" } else { "h2h-row h2h-tie" },
                                span { class: "h2h-label", "{m.label}" }
                                span { class: "h2h-margin",
                                    {if m.margin > 0 { format!("+{}", m.margin) } else { m.margin.to_string() }}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
