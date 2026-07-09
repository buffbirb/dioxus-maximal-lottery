use dioxus::prelude::*;

use crate::Route;
use crate::components::{self, *};
use api::*;

fn load_results(
    share_id: String,
    mut results: Signal<Option<ResultsView>>,
    mut error: Signal<Option<String>>,
) {
    spawn(async move {
        match get_results(share_id).await {
            Ok(r) => results.set(Some(r)),
            Err(e) => error.set(Some(format!("Failed to load results: {e}"))),
        }
    });
}

fn load_h2h(
    share_id: String,
    option_id: i64,
    label: String,
    mut h2h: Signal<Option<(String, Vec<HeadToHead>)>>,
    mut error: Signal<Option<String>>,
) {
    spawn(async move {
        match get_head_to_heads(share_id, option_id).await {
            Ok(heads) => {
                h2h.set(Some((label, heads)));
            }
            Err(e) => {
                error.set(Some(format!("Failed: {e}")));
            }
        }
    });
}

#[component]
pub fn Results(share_id: String) -> Element {
    let results = use_signal(|| None::<ResultsView>);
    let error = use_signal(|| None::<String>);
    let mut h2h = use_signal(|| None::<(String, Vec<HeadToHead>)>);

    let sid_clone = share_id.clone();
    let reload = use_callback(move |_| {
        load_results(sid_clone.clone(), results, error);
    });

    use_effect(move || {
        reload(());
    });

    let sid_h2h = share_id.clone();
    let sid_nav = share_id.clone();
    let origin = components::current_origin();

    let content = {
        let r_guard = results.read();
        let r = &*r_guard;
        match r {
            Some(r) if r.hide_results && !r.closed => {
                let deadline_opt = r.deadline;
                let deadline_text = r
                    .deadline
                    .map(|d| format!("{}", d.format("%Y-%m-%d %H:%M UTC")))
                    .unwrap_or_default();
                rsx! {
                    h1 { "{r.title}" }
                    if let Some(ref desc) = r.description {
                        p { class: "poll-description", "{desc}" }
                    }
                    if deadline_opt.is_some() {
                        p { "Deadline: {deadline_text}" }
                    }
                    if let Some(dl) = deadline_opt {
                        Countdown {
                            deadline: dl,
                            live: false,
                            on_deadline: move |_| reload(()),
                        }
                    }
                    p { class: "results-waiting", "Results will be available at the deadline." }
                    Link {
                        to: Route::Vote {
                            share_id: sid_nav.clone(),
                        },
                        class: "btn-link",
                        "Go Vote"
                    }
                }
            }
            Some(r) => {
                let dl_text = r
                    .deadline
                    .map(|d| format!("{}", d.format("%Y-%m-%d %H:%M UTC")))
                    .unwrap_or_default();
                let standings = r.standings.clone();
                let winner = r.winner.clone();
                let is_open = !r.closed;
                let dl_exists = r.deadline.is_some();
                let dl = r.deadline;
                let vote_plural = if r.vote_count != 1 { "s" } else { "" };
                let share_url = format!(
                    "{}/{}",
                    origin,
                    if r.closed {
                        format!("{}/results", share_id)
                    } else {
                        share_id.to_string()
                    }
                );
                rsx! {
                    h1 { "{r.title}" }
                    if let Some(ref desc) = r.description {
                        p { class: "poll-description", "{desc}" }
                    }

                    div { class: "results-meta",
                        span { "{r.vote_count} vote{vote_plural}" }
                        if dl_exists && is_open {
                            if let Some(d) = dl {
                                Countdown {
                                    deadline: d,
                                    live: true,
                                    on_deadline: move |_| reload(()),
                                }
                            }
                        } else if dl_exists {
                            span { class: "deadline-closed", "Closed at {dl_text}" }
                        }
                    }

                    if is_open {
                        Link {
                            to: Route::Vote { share_id: sid_nav },
                            class: "btn-link",
                            "Go Vote"
                        }
                    }

                    if let Some(ref w) = winner {
                        div { class: "winner-section",
                            h2 { "Winner" }
                            div { class: "winner-chip", "{w}" }
                        }
                    }

                    if !standings.is_empty() {
                        h2 { "Standings" }
                        div { class: "standings-list",
                            for slot in &standings {
                                div { class: "standing-slot",
                                    for (i, member) in slot.members.iter().enumerate() {
                                        div { class: "standing-row",
                                            if i == 0 {
                                                span { class: "standing-rank", "{slot.rank_label}" }
                                            } else {
                                                span { class: "standing-rank-spacer" }
                                            }
                                            {
                                                let label = member.label.clone();
                                                let oid = member.option_id;
                                                let sid_clone = sid_h2h.clone();
                                                rsx! {
                                                    div {
                                                        class: "standing-chip",
                                                        onclick: move |_| {
                                                            load_h2h(sid_clone.clone(), oid, label.clone(), h2h, error);
                                                        },
                                                        "{member.label}"
                                                        if let Some(ref pct) = member.probability_pct {
                                                            span { class: "probability-pct", "{pct}" }
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

                    ShareSection { url: share_url, label: String::from("Share:") }
                }
            }
            None => {
                rsx! {
                    p { "Loading results..." }
                }
            }
        }
    };

    let h2h_content = {
        let h = h2h.read();
        match &*h {
            Some((label, heads)) => {
                let l = label.clone();
                let mut hds = heads.clone();
                let r = results.read();
                if let Some(ref rdata) = *r {
                    let mut position_map = std::collections::HashMap::new();
                    let mut pos = 0usize;
                    for slot in &rdata.standings {
                        for m in &slot.members {
                            position_map.insert(m.option_id, pos);
                            pos += 1;
                        }
                    }
                    hds.sort_by_key(|h| {
                        position_map
                            .get(&h.option_id)
                            .copied()
                            .unwrap_or(usize::MAX)
                    });
                }
                let mut rows = Vec::new();
                for head in &hds {
                    let margin_class_str = margin_class(head.margin);
                    let prefix = if head.margin > 0 { "+" } else { "" };
                    let margin_str = format!("{prefix}{}", head.margin);
                    rows.push(rsx! {
                        div { class: "h2h-row",
                            span { class: "h2h-option", "{head.label}" }
                            span { class: "h2h-margin {margin_class_str}", "{margin_str}" }
                        }
                    });
                }
                rsx! {
                    div { class: "h2h-modal",
                        h2 { "{l}" }
                        p { class: "h2h-subtitle", "Head-to-head margins" }
                        div { class: "h2h-list", {rows.into_iter()} }
                    }
                }
            }
            None => rsx! {},
        }
    };

    rsx! {
        div { class: "results-page",
            if let Some(ref err) = *error.read() {
                div { class: "error-banner", "{err}" }
            }
            {content}
        }

        Modal { show: h2h.read().is_some(), onclose: move |_| h2h.set(None), {h2h_content} }
    }
}

fn margin_class(margin: i64) -> &'static str {
    if margin > 0 {
        "margin-positive"
    } else if margin < 0 {
        "margin-negative"
    } else {
        "margin-neutral"
    }
}
