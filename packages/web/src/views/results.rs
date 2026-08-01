use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

use api::model::ResultsView;

use crate::Route;
use crate::components::{Countdown, Modal, OptionChip, ShareSection, Skeleton};

#[component]
pub fn Results(share_id: String) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_| rsx! {
                ResultsSkeleton {}
            },
            ResultsLoader { share_id }
        }
    }
}

#[component]
fn ResultsSkeleton() -> Element {
    rsx! {
        div { id: "results",
            Skeleton { class: "skeleton-title" }
            Skeleton { class: "skeleton-line" }
            Skeleton { class: "skeleton-ranker" }
        }
    }
}

/// A poll's deadline drives its own reveal via `Countdown` in `ResultsBody`,
/// but a vote cap has no client-side clock to watch - re-check every few
/// seconds while results are hidden behind an as-yet-unreached cap, so the
/// page reveals without a manual reload. Self-terminating: once results
/// become visible, this stops rescheduling itself.
const HIDDEN_CAP_POLL_MS: u32 = 5000;

#[component]
fn ResultsLoader(share_id: String) -> Element {
    let mut refresh = use_signal(|| 0u32);
    let results = use_server_future({
        let share_id = share_id.clone();
        move || {
            let share_id = share_id.clone();
            let _ = refresh();
            async move { api::polls::get_results(share_id).await }
        }
    })?;

    use_effect(move || {
        let awaiting_cap_reveal = matches!(
            &*results.read(),
            Some(Ok(view)) if !view.results_visible && view.vote_cap.is_some()
        );
        if awaiting_cap_reveal {
            spawn(async move {
                TimeoutFuture::new(HIDDEN_CAP_POLL_MS).await;
                refresh += 1;
            });
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
        None => unreachable!("use_server_future resolves before suspense clears"),
    }
}

#[component]
fn ResultsBody(share_id: String, view: ResultsView, on_reveal: EventHandler<()>) -> Element {
    let mut selected = use_signal(|| None::<i64>);

    // Both the label and margins were already computed server-side alongside
    // `standings`, so opening the modal is a synchronous lookup rather than a
    // fetch - no loading state needed.
    let selected_data = selected().and_then(|option_id| {
        let label = view.options.iter().find(|o| o.id == option_id)?.label.clone();
        let margins = view
            .head_to_head
            .iter()
            .find(|(id, _)| *id == option_id)
            .map(|(_, margins)| margins.clone())
            .unwrap_or_default();
        Some((label, margins))
    });

    if !view.results_visible {
        return rsx! {
            div { id: "results",
                h1 { "{view.title}" }
                p { class: "hidden-results-notice", "Results are hidden until the poll closes." }
                if let Some(deadline) = view.deadline {
                    div { class: "countdown-row",
                        "Reveals in "
                        Countdown {
                            deadline,
                            on_elapsed: move |_| on_reveal.call(()),
                        }
                    }
                }
                if let Some(cap) = view.vote_cap {
                    p { class: "deadline-notice", "Reveals at {cap} votes ({view.vote_count} so far)" }
                }
                Link {
                    to: Route::Vote {
                        share_id: share_id.clone(),
                    },
                    class: "cta-button back-to-vote",
                    "\u{2190} Back to vote"
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
                if view.closed {
                    span { class: "status-badge status-closed",
                        {
                            match view.deadline {
                                Some(deadline) if deadline <= chrono::Utc::now() => {
                                    format!("Closed at {}", deadline.to_rfc2822())
                                }
                                _ => "Closed after reaching its vote cap".to_string(),
                            }
                        }
                    }
                } else if let Some(deadline) = view.deadline {
                    span { class: "status-badge status-live",
                        span { class: "live-dot" }
                        "Live"
                    }
                    "Closes in "
                    Countdown { deadline, on_elapsed: move |_| on_reveal.call(()) }
                } else {
                    span { class: "status-badge status-live",
                        span { class: "live-dot" }
                        "Live"
                    }
                }
            }

            if !view.closed {
                Link {
                    to: Route::Vote {
                        share_id: share_id.clone(),
                    },
                    class: "secondary-link back-to-vote",
                    "\u{2190} Back to vote"
                }
            }

            if view.vote_count == 0 || view.standings.is_empty() {
                p { class: "empty-note", "No votes yet." }
            } else {
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
                                            let option_id = member.option_id;
                                            move |_| selected.set(Some(option_id))
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }

            ShareSection { path: if view.closed { format!("/{share_id}/results") } else { format!("/{share_id}") } }

            if let Some((label, margins)) = selected_data {
                Modal { on_close: move |_| selected.set(None),
                    h2 { "{label}" }
                    p { class: "modal-subtitle", "Head-to-head margins" }
                    if margins.is_empty() {
                        p { class: "empty-note", "No votes yet." }
                    } else {
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
}
