//! Drag-and-drop (and tap-to-rank) tier ranking built on raw pointer events
//! (not HTML5 dragstart/drop) so it works on touch devices too.
//!
//! Touch pointers implicitly capture to whatever element received
//! `pointerdown`, which would break the usual "which zone is the pointer
//! over" trick of listening for `pointerenter`/`pointerleave` on each zone -
//! those never fire once a touch is captured to the dragged chip. To sidestep
//! that entirely, hover is derived every `pointermove` by hit-testing the
//! DOM at the pointer's current coordinates (`dropzone_at`) rather than by
//! which element received the event, so it works identically for mouse and
//! touch and never depends on capture semantics.
use dioxus::prelude::*;

use api::model::OptionView;

use crate::components::option_chip::OptionChip;

/// Below this movement (in CSS px) from the initial `pointerdown`, a
/// pointerup is treated as a tap rather than a drag.
const DRAG_THRESHOLD_PX: f64 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum DropZone {
    Unranked,
    /// Drop onto an existing tier: join it as a tie.
    MergeInto(usize),
    /// Drop between tiers (or at the very start/end): create a new tier at
    /// this gap index, shifting later tiers down.
    InsertAt(usize),
}

impl DropZone {
    /// Only reachable through `dropzone_at`, which is compiled out entirely
    /// on non-web targets (server SSR never drags anything).
    #[cfg_attr(not(feature = "web"), allow(dead_code))]
    fn parse(s: &str) -> Option<Self> {
        if s == "unranked" {
            return Some(DropZone::Unranked);
        }
        if let Some(rest) = s.strip_prefix("merge:") {
            return rest.parse().ok().map(DropZone::MergeInto);
        }
        if let Some(rest) = s.strip_prefix("insert:") {
            return rest.parse().ok().map(DropZone::InsertAt);
        }
        None
    }

    fn attr(self) -> String {
        match self {
            DropZone::Unranked => "unranked".to_string(),
            DropZone::MergeInto(i) => format!("merge:{i}"),
            DropZone::InsertAt(i) => format!("insert:{i}"),
        }
    }
}

#[derive(Clone, Copy)]
struct DragState {
    option_id: i64,
    x: f64,
    y: f64,
}

#[cfg(feature = "web")]
mod ffi {
    use wasm_bindgen::prelude::*;

    // Plain synchronous FFI (not `document::eval`, which is message-passed
    // and not guaranteed to run before the next Dioxus render) so the
    // "before" snapshot is guaranteed to happen before the state mutation
    // that triggers it, and the "after" measurement in `flip_play` happens
    // exactly when called.
    #[wasm_bindgen(inline_js = r#"
        let snapshots = new Map();

        export function flip_snapshot(root) {
            const container = document.querySelector(root);
            snapshots.clear();
            if (!container) return;
            for (const el of container.querySelectorAll('[data-flip-id]')) {
                const id = el.dataset.flipId;
                if (id) snapshots.set(id, el.getBoundingClientRect());
            }
        }

        export function flip_play(root) {
            const container = document.querySelector(root);
            if (!container) { snapshots.clear(); return; }
            for (const el of container.querySelectorAll('[data-flip-id]')) {
                const id = el.dataset.flipId;
                const before = id && snapshots.get(id);
                if (!before) continue;
                const after = el.getBoundingClientRect();
                const dx = before.left - after.left;
                const dy = before.top - after.top;
                if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) continue;
                el.style.transition = 'none';
                el.style.transform = `translate(${dx}px, ${dy}px)`;
                requestAnimationFrame(() => {
                    el.style.transition = '';
                    el.style.transform = '';
                });
            }
            snapshots.clear();
        }

        export function dropzone_at(root, x, y) {
            const container = document.querySelector(root);
            if (!container) return null;
            const el = document.elementFromPoint(x, y);
            const zone = el && el.closest('[data-dropzone]');
            if (!zone || !container.contains(zone)) return null;
            return zone.getAttribute('data-dropzone');
        }
    "#)]
    extern "C" {
        pub fn flip_snapshot(root: &str);
        pub fn flip_play(root: &str);
        pub fn dropzone_at(root: &str, x: f64, y: f64) -> Option<String>;
    }
}

#[cfg_attr(not(feature = "web"), allow(dead_code))]
const RANKER_SELECTOR: &str = ".tier-ranker";

#[cfg(feature = "web")]
fn flip_snapshot() {
    ffi::flip_snapshot(RANKER_SELECTOR);
}
#[cfg(not(feature = "web"))]
fn flip_snapshot() {}

#[cfg(feature = "web")]
fn flip_play() {
    ffi::flip_play(RANKER_SELECTOR);
}
#[cfg(not(feature = "web"))]
fn flip_play() {}

#[cfg(feature = "web")]
fn dropzone_at(x: f64, y: f64) -> Option<DropZone> {
    ffi::dropzone_at(RANKER_SELECTOR, x, y).and_then(|s| DropZone::parse(&s))
}
#[cfg(not(feature = "web"))]
fn dropzone_at(_x: f64, _y: f64) -> Option<DropZone> {
    None
}

/// Removes `option_id` from wherever it currently lives in `tiers` (dropping
/// the tier if that empties it) and inserts it at `zone`.
fn move_option(tiers: &mut Vec<Vec<i64>>, option_id: i64, zone: DropZone) {
    let mut removed_idx = None;
    for (i, tier) in tiers.iter_mut().enumerate() {
        if let Some(pos) = tier.iter().position(|&id| id == option_id) {
            tier.remove(pos);
            removed_idx = Some(i);
            break;
        }
    }

    let mut target = zone;
    if let Some(removed_idx) = removed_idx
        && tiers[removed_idx].is_empty()
    {
        tiers.remove(removed_idx);
        target = match target {
            DropZone::MergeInto(i) if i > removed_idx => DropZone::MergeInto(i - 1),
            DropZone::MergeInto(i) if i == removed_idx => DropZone::InsertAt(i),
            DropZone::InsertAt(i) if i > removed_idx => DropZone::InsertAt(i - 1),
            other => other,
        };
    }

    match target {
        DropZone::Unranked => {}
        DropZone::MergeInto(i) => match tiers.get_mut(i) {
            Some(tier) => tier.push(option_id),
            None => tiers.push(vec![option_id]),
        },
        DropZone::InsertAt(i) => tiers.insert(i.min(tiers.len()), vec![option_id]),
    }
}

#[component]
pub fn TierRanker(options: Vec<OptionView>, tiers: Signal<Vec<Vec<i64>>>) -> Element {
    let mut tiers = tiers;
    let mut press = use_signal(|| None::<DragState>);
    let mut dragging = use_signal(|| None::<DragState>);
    let mut hover = use_signal(|| None::<DropZone>);
    let mut selected = use_signal(|| None::<i64>);

    // Re-plays the FLIP animation any time the ranking itself changes,
    // regardless of whether the change came from a drag or a tap - both
    // paths call `flip_snapshot()` immediately before mutating `tiers`.
    use_effect(move || {
        let _ = tiers();
        flip_play();
    });

    let label_of = {
        let options = options.clone();
        move |id: i64| {
            options
                .iter()
                .find(|o| o.id == id)
                .map(|o| o.label.clone())
                .unwrap_or_default()
        }
    };

    let unranked: Vec<i64> = {
        let ranked: std::collections::HashSet<i64> = tiers().iter().flatten().copied().collect();
        options
            .iter()
            .map(|o| o.id)
            .filter(|id| !ranked.contains(id))
            .collect()
    };

    let tier_count = tiers().len();
    let interacting = dragging.read().is_some() || selected.read().is_some();

    let mut commit_move = move |option_id: i64, zone: DropZone| {
        flip_snapshot();
        tiers.with_mut(|t| move_option(t, option_id, zone));
    };

    let mut on_pointer_move = move |x: f64, y: f64| {
        if let Some(p) = press()
            && dragging.read().is_none()
        {
            let (dx, dy) = (x - p.x, y - p.y);
            if (dx * dx + dy * dy).sqrt() > DRAG_THRESHOLD_PX {
                dragging.set(Some(DragState { x, y, ..p }));
                press.set(None);
            }
        }
        if dragging.read().is_some() {
            dragging.with_mut(|d| {
                if let Some(d) = d {
                    d.x = x;
                    d.y = y;
                }
            });
            hover.set(dropzone_at(x, y));
        }
    };

    let mut on_pointer_up = move || {
        if let Some(drag) = dragging() {
            if let Some(zone) = hover() {
                commit_move(drag.option_id, zone);
            }
            dragging.set(None);
            hover.set(None);
        } else if let Some(p) = press() {
            selected.set(if selected() == Some(p.option_id) {
                None
            } else {
                Some(p.option_id)
            });
            press.set(None);
        }
    };

    let mut place_selected = move |zone: DropZone| {
        if let Some(id) = selected() {
            commit_move(id, zone);
            selected.set(None);
        }
    };

    let chip_class = move |id: i64| -> String {
        let mut classes = Vec::new();
        let is_pressed =
            press().map(|d| d.option_id) == Some(id) || dragging().map(|d| d.option_id) == Some(id);
        if is_pressed {
            classes.push("dragging-source");
        }
        if selected() == Some(id) {
            classes.push("chip-selected");
        }
        classes.join(" ")
    };

    rsx! {
        div {
            class: if interacting { "tier-ranker targets-active" } else { "tier-ranker" },
            onpointermove: move |evt| {
                let p = evt.data().client_coordinates();
                on_pointer_move(p.x, p.y);
            },
            onpointerup: move |_| on_pointer_up(),
            onpointercancel: move |_| {
                press.set(None);
                dragging.set(None);
                hover.set(None);
            },

            for (idx, rank_label, tier) in {
                let mut placed = 0usize;
                tiers()
                    .into_iter()
                    .enumerate()
                    .map(move |(idx, tier)| {
                        let rank_label = placed + 1;
                        placed += tier.len();
                        (idx, rank_label, tier)
                    })
                    .collect::<Vec<_>>()
            }
            {
                div { key: "{idx}", class: "tier-zone-wrap",
                    div {
                        class: if hover() == Some(DropZone::InsertAt(idx)) { "insert-gutter drop-hover" } else { "insert-gutter" },
                        "data-dropzone": DropZone::InsertAt(idx).attr(),
                        onclick: move |_| place_selected(DropZone::InsertAt(idx)),
                        span { class: "insert-gutter-label", "New rank here" }
                    }
                    div {
                        class: if hover() == Some(DropZone::MergeInto(idx)) { "tier-zone drop-hover" } else { "tier-zone" },
                        "data-dropzone": DropZone::MergeInto(idx).attr(),
                        onclick: move |_| place_selected(DropZone::MergeInto(idx)),
                        div { class: "tier-zone-label", "Rank {rank_label}" }
                        div { class: "chip-row",
                            for id in tier {
                                OptionChip {
                                    key: "{id}",
                                    label: label_of(id),
                                    flip_id: "chip-{id}",
                                    class: chip_class(id),
                                    onpointerdown: move |evt: PointerEvent| {
                                        evt.stop_propagation();
                                        let p = evt.data().client_coordinates();
                                        press
                                            .set(
                                                Some(DragState {
                                                    option_id: id,
                                                    x: p.x,
                                                    y: p.y,
                                                }),
                                            );
                                    },
                                }
                            }
                        }
                    }
                }
            }

            div {
                class: {
                    let mut c = String::from("tier-zone new-tier-zone");
                    if tier_count == 0 && !interacting {
                        c.push_str(" new-tier-zone-persistent");
                    }
                    if hover() == Some(DropZone::InsertAt(tier_count)) {
                        c.push_str(" drop-hover");
                    }
                    c
                },
                "data-dropzone": DropZone::InsertAt(tier_count).attr(),
                onclick: move |_| place_selected(DropZone::InsertAt(tier_count)),
                span { class: "placeholder", "Drag options here to rank, or tap one to get started" }
            }

            div {
                class: if hover() == Some(DropZone::Unranked) { "tier-zone unranked-zone drop-hover" } else { "tier-zone unranked-zone" },
                "data-dropzone": DropZone::Unranked.attr(),
                onclick: move |_| place_selected(DropZone::Unranked),
                div { class: "tier-zone-label", "Unranked options" }
                div { class: "chip-row",
                    for id in unranked {
                        OptionChip {
                            key: "{id}",
                            label: label_of(id),
                            flip_id: "chip-{id}",
                            class: chip_class(id),
                            onpointerdown: move |evt: PointerEvent| {
                                evt.stop_propagation();
                                let p = evt.data().client_coordinates();
                                press
                                    .set(
                                        Some(DragState {
                                            option_id: id,
                                            x: p.x,
                                            y: p.y,
                                        }),
                                    );
                            },
                        }
                    }
                }
            }

            if let Some(drag) = dragging() {
                div {
                    class: "chip-ghost",
                    style: "left: {drag.x}px; top: {drag.y}px;",
                    "{label_of(drag.option_id)}"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_drop_creates_a_tier() {
        let mut tiers: Vec<Vec<i64>> = vec![];
        move_option(&mut tiers, 1, DropZone::InsertAt(0));
        assert_eq!(tiers, vec![vec![1]]);
    }

    #[test]
    fn moving_the_only_member_out_deletes_the_tier() {
        let mut tiers = vec![vec![1], vec![2]];
        move_option(&mut tiers, 1, DropZone::MergeInto(1));
        assert_eq!(tiers, vec![vec![2, 1]]);
    }

    #[test]
    fn moving_back_onto_its_own_now_empty_tier_recreates_it() {
        let mut tiers = vec![vec![1]];
        move_option(&mut tiers, 1, DropZone::MergeInto(0));
        assert_eq!(tiers, vec![vec![1]]);
    }

    #[test]
    fn dropping_into_unranked_removes_without_reinserting() {
        let mut tiers = vec![vec![1, 2]];
        move_option(&mut tiers, 1, DropZone::Unranked);
        assert_eq!(tiers, vec![vec![2]]);
    }

    #[test]
    fn moving_across_tiers_shifts_later_indices_down() {
        let mut tiers = vec![vec![1], vec![2], vec![3]];
        // Move the sole member of tier 0 into tier 2; tier 0 disappears so
        // the old "tier 2" target shifts down to index 1.
        move_option(&mut tiers, 1, DropZone::MergeInto(2));
        assert_eq!(tiers, vec![vec![2], vec![3, 1]]);
    }

    #[test]
    fn insert_at_gap_creates_a_new_tier_between_others() {
        let mut tiers = vec![vec![1], vec![2]];
        move_option(&mut tiers, 3, DropZone::InsertAt(1));
        assert_eq!(tiers, vec![vec![1], vec![3], vec![2]]);
    }

    #[test]
    fn insert_at_start_creates_a_new_top_tier() {
        let mut tiers = vec![vec![1], vec![2]];
        move_option(&mut tiers, 3, DropZone::InsertAt(0));
        assert_eq!(tiers, vec![vec![3], vec![1], vec![2]]);
    }

    #[test]
    fn insert_at_end_appends_a_new_tier() {
        let mut tiers = vec![vec![1], vec![2]];
        move_option(&mut tiers, 3, DropZone::InsertAt(2));
        assert_eq!(tiers, vec![vec![1], vec![2], vec![3]]);
    }

    #[test]
    fn moving_sole_member_below_its_own_tier_shifts_gap_index_down() {
        let mut tiers = vec![vec![1], vec![2]];
        // "Insert after tier 0" (gap index 1) once tier 0 vanishes becomes
        // gap index 0 - i.e. still directly before what's now the only tier.
        move_option(&mut tiers, 1, DropZone::InsertAt(1));
        assert_eq!(tiers, vec![vec![1], vec![2]]);
    }

    #[test]
    fn dropzone_parses_all_variants() {
        assert_eq!(DropZone::parse("unranked"), Some(DropZone::Unranked));
        assert_eq!(DropZone::parse("merge:3"), Some(DropZone::MergeInto(3)));
        assert_eq!(DropZone::parse("insert:0"), Some(DropZone::InsertAt(0)));
        assert_eq!(DropZone::parse("garbage"), None);
    }
}
