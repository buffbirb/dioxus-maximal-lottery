//! Drag-and-drop tier ranking built on raw pointer events (not HTML5
//! dragstart/drop) so it works on touch devices too. All pointermove/up
//! listeners live on the root container rather than using
//! `setPointerCapture`, which is fine as long as the drag stays inside the
//! ranker's bounds - true for both mouse and touch since pointer events
//! bubble even when a touch's target is implicitly captured.
use dioxus::prelude::*;

use api::model::OptionView;

use crate::components::option_chip::OptionChip;

#[derive(Clone, Copy, PartialEq)]
enum DropZone {
    Unranked,
    /// Drop onto an existing tier: join it as a tie.
    MergeInto(usize),
    /// Drop between tiers (or at the very start/end): create a new tier at
    /// this gap index, shifting later tiers down.
    InsertAt(usize),
}

#[derive(Clone, Copy)]
struct DragState {
    option_id: i64,
    x: f64,
    y: f64,
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
    let mut dragging = use_signal(|| None::<DragState>);
    let mut hover = use_signal(|| None::<DropZone>);

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

    let mut on_drop = move || {
        if let (Some(drag), Some(zone)) = (dragging(), hover()) {
            tiers.with_mut(|t| move_option(t, drag.option_id, zone));
        }
        dragging.set(None);
        hover.set(None);
    };

    rsx! {
        div {
            class: "tier-ranker",
            onpointermove: move |evt| {
                if dragging.read().is_some() {
                    let p = evt.data().client_coordinates();
                    dragging
                        .with_mut(|d| {
                            if let Some(d) = d {
                                d.x = p.x;
                                d.y = p.y;
                            }
                        });
                }
            },
            onpointerup: move |_| on_drop(),
            onpointercancel: move |_| {
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
                        onpointerenter: move |_| {
                            if dragging.read().is_some() {
                                hover.set(Some(DropZone::InsertAt(idx)));
                            }
                        },
                        onpointerleave: move |_| {
                            if hover() == Some(DropZone::InsertAt(idx)) {
                                hover.set(None);
                            }
                        },
                    }
                    div {
                        class: if hover() == Some(DropZone::MergeInto(idx)) { "tier-zone drop-hover" } else { "tier-zone" },
                        onpointerenter: move |_| {
                            if dragging.read().is_some() {
                                hover.set(Some(DropZone::MergeInto(idx)));
                            }
                        },
                        onpointerleave: move |_| {
                            if hover() == Some(DropZone::MergeInto(idx)) {
                                hover.set(None);
                            }
                        },
                        div { class: "tier-zone-label", "Rank {rank_label}" }
                        div { class: "chip-row",
                            for id in tier {
                                OptionChip {
                                    key: "{id}",
                                    label: label_of(id),
                                    class: if dragging().map(|d| d.option_id) == Some(id) { "dragging-source".to_string() } else { String::new() },
                                    onpointerdown: move |evt: PointerEvent| {
                                        evt.stop_propagation();
                                        let p = evt.data().client_coordinates();
                                        dragging
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
                class: if hover() == Some(DropZone::InsertAt(tier_count)) { "tier-zone new-tier-zone drop-hover" } else { "tier-zone new-tier-zone" },
                onpointerenter: move |_| {
                    if dragging.read().is_some() {
                        hover.set(Some(DropZone::InsertAt(tier_count)));
                    }
                },
                onpointerleave: move |_| {
                    if hover() == Some(DropZone::InsertAt(tier_count)) {
                        hover.set(None);
                    }
                },
                if tier_count == 0 {
                    span { class: "placeholder", "Drag options here to rank" }
                }
            }

            div {
                class: if hover() == Some(DropZone::Unranked) { "tier-zone unranked-zone drop-hover" } else { "tier-zone unranked-zone" },
                onpointerenter: move |_| {
                    if dragging.read().is_some() {
                        hover.set(Some(DropZone::Unranked));
                    }
                },
                onpointerleave: move |_| {
                    if hover() == Some(DropZone::Unranked) {
                        hover.set(None);
                    }
                },
                div { class: "tier-zone-label", "Unranked options" }
                div { class: "chip-row",
                    if unranked.is_empty() {
                        span { class: "placeholder", "All options ranked" }
                    }
                    for id in unranked {
                        OptionChip {
                            key: "{id}",
                            label: label_of(id),
                            class: if dragging().map(|d| d.option_id) == Some(id) { "dragging-source".to_string() } else { String::new() },
                            onpointerdown: move |evt: PointerEvent| {
                                evt.stop_propagation();
                                let p = evt.data().client_coordinates();
                                dragging
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
}
