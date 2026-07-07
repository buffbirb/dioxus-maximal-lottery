use dioxus::prelude::*;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct TierRankerOutput {
    pub tiers: Vec<Vec<i64>>,
    pub unranked: Vec<i64>,
}

#[derive(Clone, PartialEq)]
pub struct OptionInfo {
    pub id: i64,
    pub label: String,
}

#[component]
pub fn TierRanker(
    options: Vec<OptionInfo>,
    initial_tiers: Option<Vec<Vec<i64>>>,
    onchange: EventHandler<TierRankerOutput>,
) -> Element {
    let mut tiers = use_signal(|| initial_tiers.unwrap_or_default());
    let mut drag_state = use_signal(|| DragState::None);
    let mut drag_over = use_signal(|| DragOverTarget::None);

    let all_option_ids: HashSet<i64> = options.iter().map(|o| o.id).collect();
    let label_map: Vec<(i64, String)> = options.iter().map(|o| (o.id, o.label.clone())).collect();
    let label_map_sort = label_map.clone();

    let ranked_ids = use_memo(move || {
        let mut set = HashSet::new();
        for tier in tiers.read().iter() {
            for id in tier {
                set.insert(*id);
            }
        }
        set
    });

    let unranked = use_memo(move || {
        let mut ids: Vec<i64> = all_option_ids
            .difference(&ranked_ids())
            .copied()
            .collect();
        ids.sort_by_key(|id| {
            label_map_sort
                .iter()
                .find(|(i, _)| *i == *id)
                .map(|(_, l)| l.to_lowercase())
        });
        ids
    });

    use_effect(move || {
        onchange.call(TierRankerOutput {
            tiers: tiers.read().clone(),
            unranked: unranked.read().clone(),
        });
    });

    let tiers_snapshot = tiers.read().clone();
    let unranked_snapshot = unranked.read().clone();

    let ranked_rsx = if tiers_snapshot.is_empty() {
        rsx! {
            div {
                class: "ranked-placeholder",
                ondragover: move |e| e.prevent_default(),
                ondrop: move |_| {
                    if let DragState::Dragging(id) = drag_state() {
                        create_tier_at(&mut tiers, 0, id);
                        drag_state.set(DragState::None);
                    }
                },
                "Drag options here to rank"
            }
        }
    } else {
        let mut rows = Vec::new();
        for (tier_idx, tier) in tiers_snapshot.iter().enumerate() {
            let rank = ranked_count_before(&tiers_snapshot, tier_idx) + 1;
            let mut chip_elements = Vec::new();
            for opt_id in tier.iter() {
                let label = label_map.iter()
                    .find(|(i, _)| *i == *opt_id)
                    .map(|(_, l)| l.clone())
                    .unwrap_or_else(|| "?".into());
                let oid = *opt_id;
                let t_idx = tier_idx;
                chip_elements.push(rsx! {
                    div {
                        class: "tier-chip",
                        draggable: "true",
                        ondragstart: move |_| {
                            drag_state.set(DragState::Dragging(oid));
                        },
                        ondragend: move |_| {
                            drag_state.set(DragState::None);
                        },
                        "{label}"
                        span {
                            class: "tier-chip-remove",
                            onclick: move |e| {
                                e.stop_propagation();
                                remove_from_tier(&mut tiers, t_idx, oid);
                            },
                            "x"
                        }
                    }
                });
            }
            let gap_class = if drag_over() == DragOverTarget::GapBefore(tier_idx) {
                "tier-gap drag-over"
            } else {
                "tier-gap"
            };
            let tier_class = if drag_over() == DragOverTarget::TierContents(tier_idx) {
                "tier-options drag-over"
            } else {
                "tier-options"
            };
            rows.push(rsx! {
                div {
                    class: "{gap_class}",
                    ondragover: move |e| e.prevent_default(),
                    ondragenter: move |_| drag_over.set(DragOverTarget::GapBefore(tier_idx)),
                    ondragleave: move |_| drag_over.set(DragOverTarget::None),
                    ondrop: move |_| {
                        if let DragState::Dragging(id) = drag_state() {
                            create_tier_at(&mut tiers, tier_idx, id);
                            drag_state.set(DragState::None);
                        }
                    }
                }
                div { class: "tier-row",
                    span { class: "tier-rank", "{rank}" }
                    div {
                        class: "{tier_class}",
                        ondragover: move |e| e.prevent_default(),
                        ondragenter: move |_| drag_over.set(DragOverTarget::TierContents(tier_idx)),
                        ondragleave: move |_| drag_over.set(DragOverTarget::None),
                        ondrop: move |_| {
                            if let DragState::Dragging(id) = drag_state() {
                                move_to_tier(&mut tiers, tier_idx, id);
                                drag_state.set(DragState::None);
                            }
                        },
                        {chip_elements.into_iter()}
                    }
                }
            });
        }
        let len = tiers_snapshot.len();
        let last_gap_class = if drag_over() == DragOverTarget::GapAfter(len) {
            "tier-gap last-gap drag-over"
        } else {
            "tier-gap last-gap"
        };
        rows.push(rsx! {
            div {
                class: "{last_gap_class}",
                ondragover: move |e| e.prevent_default(),
                ondragenter: move |_| drag_over.set(DragOverTarget::GapAfter(len)),
                ondragleave: move |_| drag_over.set(DragOverTarget::None),
                ondrop: move |_| {
                    if let DragState::Dragging(id) = drag_state() {
                        create_tier_at(&mut tiers, len, id);
                        drag_state.set(DragState::None);
                    }
                }
            }
        });
        rsx! { {rows.into_iter()} }
    };

    let mut unranked_chips = Vec::new();
    for opt_id in unranked_snapshot.iter() {
        let label = label_map.iter()
            .find(|(i, _)| *i == *opt_id)
            .map(|(_, l)| l.clone())
            .unwrap_or_else(|| "?".into());
        let oid = *opt_id;
        unranked_chips.push(rsx! {
            div {
                class: "unranked-chip",
                draggable: "true",
                ondragstart: move |_| {
                    drag_state.set(DragState::Dragging(oid));
                },
                ondragend: move |_| {
                    drag_state.set(DragState::None);
                },
                "{label}"
            }
        });
    }

    let unranked_class = if drag_over() == DragOverTarget::Unranked {
        "unranked-options drag-over"
    } else {
        "unranked-options"
    };
    rsx! {
        div { class: "tier-ranker",
            div { class: "ranked-area", {ranked_rsx} }
            div { class: "unranked-area",
                h3 { class: "unranked-header", "Unranked" }
                div {
                    class: "{unranked_class}",
                    ondragover: move |e| e.prevent_default(),
                    ondragenter: move |_| drag_over.set(DragOverTarget::Unranked),
                    ondragleave: move |_| drag_over.set(DragOverTarget::None),
                    ondrop: move |_| {
                        if let DragState::Dragging(id) = drag_state() {
                            move_to_unranked(&mut tiers, id);
                            drag_state.set(DragState::None);
                        }
                    },
                    {unranked_chips.into_iter()}
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
enum DragState {
    None,
    Dragging(i64),
}

#[derive(Clone, PartialEq)]
enum DragOverTarget {
    None,
    GapBefore(usize),
    GapAfter(usize),
    TierContents(usize),
    Unranked,
}

fn ranked_count_before(tiers: &[Vec<i64>], tier_idx: usize) -> usize {
    tiers.iter().take(tier_idx).map(|t| t.len()).sum()
}

fn remove_from_tier(tiers: &mut Signal<Vec<Vec<i64>>>, tier_idx: usize, opt_id: i64) {
    if let Some(tier) = tiers.write().get_mut(tier_idx) {
        tier.retain(|id| *id != opt_id);
    }
    cleanup_empty_tiers(tiers);
}

fn move_to_unranked(tiers: &mut Signal<Vec<Vec<i64>>>, opt_id: i64) {
    for tier in tiers.write().iter_mut() {
        tier.retain(|id| *id != opt_id);
    }
    cleanup_empty_tiers(tiers);
}

fn move_to_tier(tiers: &mut Signal<Vec<Vec<i64>>>, tier_idx: usize, opt_id: i64) {
    for tier in tiers.write().iter_mut() {
        tier.retain(|id| *id != opt_id);
    }
    if tier_idx < tiers.read().len() {
        tiers.write()[tier_idx].push(opt_id);
    }
    cleanup_empty_tiers(tiers);
}

fn create_tier_at(tiers: &mut Signal<Vec<Vec<i64>>>, idx: usize, opt_id: i64) {
    for tier in tiers.write().iter_mut() {
        tier.retain(|id| *id != opt_id);
    }
    let len = tiers.read().len();
    let idx = idx.min(len);
    tiers.write().insert(idx, vec![opt_id]);
    cleanup_empty_tiers(tiers);
}

fn cleanup_empty_tiers(tiers: &mut Signal<Vec<Vec<i64>>>) {
    tiers.write().retain(|t| !t.is_empty());
}
