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

/// Direct `web-sys` calls rather than `document::eval`, which is
/// message-passed and not guaranteed to run before the next Dioxus render:
/// the "before" snapshot has to happen before the state mutation that
/// triggers it, and the "after" measurement in `flip_play` exactly when
/// called. These are ordinary synchronous DOM calls, so both hold.
#[cfg(feature = "web")]
mod dom {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use web_sys::{Element, HtmlElement, window};

    use super::DropZone;

    /// How far into a row still counts as aiming at the gutter on that side,
    /// as a fraction of the row's height. The two sides are deliberately
    /// asymmetric.
    ///
    /// Reaching *down* out of an open gutter into the row beneath it is a
    /// commitment to that row, so the gutter shuts the moment the pointer
    /// crosses the boundary. Anticipating that move by 15% keeps the gap -
    /// and therefore everything below it - displaced while the pointer is
    /// already over the row it wants, which reads as the target sliding away
    /// just as you reach it.
    ///
    /// Reaching down for the gutter *below* a row is worth anticipating,
    /// though: there the gap opens ahead of the pointer, into space the
    /// pointer is still travelling toward.
    ///
    /// The band above is not quite zero: the same boundary is crossed going
    /// up, and a few pixels of overlap keep a gutter catchable without having
    /// to land inside its 10px resting height.
    const EDGE_FRACTION_ABOVE: f64 = 0.05;
    const EDGE_FRACTION_BELOW: f64 = 0.15;

    /// How far outside the ranker the pointer may stray and still resolve to
    /// a target. Past this the answer is `None`, which a live drag reads as
    /// "keep the target you have" - so this sets how far out the ranker
    /// carries on re-aiming, not how far a chip may be dragged.
    const OUTSIDE_SLACK_PX: f64 = 24.0;

    /// Paint as well as position is replayed: the thing being lifted is
    /// accent-coloured (the ghost outright, a tap-armed chip by its border
    /// and ring) and what lands is an ordinary chip. Replaying the lifted
    /// colours and letting them transition out turns that swap into a settle
    /// instead of a hard cut.
    const PAINT_PROPERTIES: [&str; 4] = ["background-color", "border-color", "color", "box-shadow"];

    struct Snapshot {
        left: f64,
        top: f64,
        /// Computed values for `PAINT_PROPERTIES`, in the same order.
        paint: [String; 4],
    }

    thread_local! {
        /// wasm is single-threaded, so this is just a module global that the
        /// borrow checker can still reason about.
        static SNAPSHOTS: RefCell<HashMap<String, Snapshot>> = RefCell::new(HashMap::new());
    }

    fn container(root: &str) -> Option<Element> {
        window()?.document()?.query_selector(root).ok().flatten()
    }

    /// `querySelectorAll` as a plain `Vec`, in document order. Anything that
    /// does not cast to `T` is dropped rather than panicking - a stray
    /// non-element node matching one of these selectors is not worth taking
    /// the page down for.
    fn query_all<T: JsCast>(root: &Element, selector: &str) -> Vec<T> {
        let Ok(nodes) = root.query_selector_all(selector) else {
            return Vec::new();
        };
        (0..nodes.length())
            .filter_map(|i| nodes.get(i))
            .filter_map(|node| node.dyn_into::<T>().ok())
            .collect()
    }

    fn zone_of(el: &Element) -> Option<DropZone> {
        DropZone::parse(&el.get_attribute("data-dropzone")?)
    }

    /// Only `focus_id` - the one chip being moved - is recorded, so it is the
    /// only thing `flip_play` animates. Every other chip has no "before" rect
    /// and so gets no transform of its own, which is what keeps it locked to
    /// its slot: rank rows are laid out by flexbox and jump the instant the
    /// gutters collapse, and a chip that glided independently would visibly
    /// detach from the row carrying it.
    pub fn flip_snapshot(root: &str, focus_id: &str) {
        SNAPSHOTS.with_borrow_mut(HashMap::clear);
        snapshot(root, focus_id);
    }

    fn snapshot(root: &str, focus_id: &str) -> Option<()> {
        if focus_id.is_empty() {
            return None;
        }
        let window = window()?;
        let container = container(root)?;

        for el in query_all::<Element>(&container, "[data-flip-id]") {
            let Some(id) = el.get_attribute("data-flip-id") else {
                continue;
            };
            if id != focus_id {
                continue;
            }
            // The drag ghost wears the same flip id as the chip it was lifted
            // from, and always wins the tie regardless of document order: it
            // is where the chip visually *is*. That makes a release
            // interpolate from the pointer instead of from the slot the faded
            // original was still nominally occupying.
            let seen = SNAPSHOTS.with_borrow(|s| s.contains_key(&id));
            if seen && !el.has_attribute("data-flip-lift") {
                continue;
            }
            let rect = el.get_bounding_client_rect();
            let Ok(Some(computed)) = window.get_computed_style(&el) else {
                continue;
            };
            let paint =
                PAINT_PROPERTIES.map(|p| computed.get_property_value(p).unwrap_or_default());
            SNAPSHOTS.with_borrow_mut(|s| {
                s.insert(
                    id,
                    Snapshot {
                        left: rect.left(),
                        top: rect.top(),
                        paint,
                    },
                )
            });
        }
        Some(())
    }

    pub fn flip_play(root: &str) {
        play(root);
        SNAPSHOTS.with_borrow_mut(HashMap::clear);
    }

    fn play(root: &str) -> Option<()> {
        let window = window()?;
        let container = container(root)?;

        // A rank row animating in is mid-zoom, and it is the very row the
        // landing chip sits in - measuring the chip inside a scaled parent
        // would read its "after" rect off a layout that is still growing, and
        // skew where the glide starts. Hold the row's animation off across the
        // measurement and start it in the same frame the chip is released,
        // which also puts the two in step.
        let appearing: Vec<HtmlElement> = query_all(&container, ".rank-appearing");
        for el in &appearing {
            let _ = el.style().set_property("animation", "none");
        }

        let moved = SNAPSHOTS.with_borrow(|snapshots| {
            let mut moved = Vec::new();
            for el in query_all::<HtmlElement>(&container, "[data-flip-id]") {
                let Some(before) = el
                    .get_attribute("data-flip-id")
                    .and_then(|id| snapshots.get(&id))
                else {
                    continue;
                };
                let after = el.get_bounding_client_rect();
                let dx = before.left - after.left();
                let dy = before.top - after.top();
                if dx.abs() < 0.5 && dy.abs() < 0.5 {
                    continue;
                }
                let style = el.style();
                let _ = style.set_property("transition", "none");
                let _ = style.set_property("transform", &format!("translate({dx}px, {dy}px)"));
                for (property, value) in PAINT_PROPERTIES.iter().zip(&before.paint) {
                    let _ = style.set_property(property, value);
                }
                moved.push(el);
            }
            moved
        });

        // Removing a property is the CSSOM equivalent of assigning `''`: the
        // inline override goes away and the stylesheet's transition takes the
        // element the rest of the way on its own.
        let release = Closure::once_into_js(move || {
            for el in &moved {
                let style = el.style();
                let _ = style.remove_property("transition");
                let _ = style.remove_property("transform");
                for property in PAINT_PROPERTIES {
                    let _ = style.remove_property(property);
                }
            }
            for el in &appearing {
                let _ = el.style().remove_property("animation");
            }
        });
        let _ = window.request_animation_frame(release.unchecked_ref());
        Some(())
    }

    /// The pointer handlers live on the ranker, so without capture a drag
    /// that wanders off it stops receiving pointermove - the ghost sticks
    /// where it left - and the pointerup lands on some other element
    /// entirely, so the gesture never ends. Capturing retargets every later
    /// event for this pointer to the ranker, letting a drag range over the
    /// whole page; the browser releases the capture itself on
    /// pointerup/pointercancel. The chip does not become droppable anywhere
    /// by way of this - a release off the ranker still resolves to the last
    /// target the pointer was over, and glides back to it.
    pub fn capture_pointer(root: &str, pointer_id: i32) {
        let Some(container) = container(root) else {
            return;
        };
        // Throws if the pointer is already gone, in which case the gesture is
        // over anyway.
        let _ = container.set_pointer_capture(pointer_id);
    }

    /// Resolving the target against the *live* layout is a feedback loop: the
    /// hovered gutter opens, everything below it slides down, the pointer
    /// suddenly finds itself over a different zone, that closes the gutter,
    /// and the layout snaps back - a visible bounce, worst at the seam
    /// between the last rank and the unranked pile where a dead margin used
    /// to resolve to nothing at all.
    ///
    /// So the target is resolved in "rest space": the layout as it would be
    /// with every gutter collapsed. `tiers` cannot change mid-drag, so rest
    /// geometry is fixed for the whole gesture, which makes this a pure
    /// function of the pointer position - the loop is gone by construction
    /// rather than damped by a fudge factor.
    pub fn dropzone_at(root: &str, x: f64, y: f64) -> Option<DropZone> {
        let container = container(root)?;
        // Nothing is rendered yet - `query_selector` rather than `query_all`
        // since this runs on every pointermove and only existence matters.
        container.query_selector("[data-dropzone]").ok().flatten()?;

        // The single open gutter, and how much taller it is than a resting
        // one. Everything below its bottom edge is displaced by exactly that
        // much.
        let open = container
            .query_selector(".insert-gutter.drop-hover")
            .ok()
            .flatten();
        let mut extra = 0.0;
        let mut open_top = 0.0;
        let mut open_bottom = f64::INFINITY;
        if let Some(open) = &open {
            let rect = open.get_bounding_client_rect();
            // While the pointer is inside the gap it opened, hold it open.
            // This only ever repeats the current answer, so it cannot
            // introduce a flip of its own - it just stops the gap from
            // snapping shut the moment the pointer drifts a pixel past a
            // neighbouring rank's edge band.
            if x >= rect.left() && x <= rect.right() && y >= rect.top() && y <= rect.bottom() {
                return zone_of(open);
            }
            // Any gutter but the open one - only one is ever `drop-hover`
            // during a drag, since `hover` holds a single zone.
            let rest_height = container
                .query_selector("[data-dropzone].insert-gutter:not(.drop-hover)")
                .ok()
                .flatten()
                .map_or(0.0, |el| el.get_bounding_client_rect().height());
            extra = rect.height() - rest_height;
            open_top = rect.top();
            open_bottom = rect.bottom();
        }

        let bounds = container.get_bounding_client_rect();
        if x < bounds.left() - OUTSIDE_SLACK_PX
            || x > bounds.right() + OUTSIDE_SLACK_PX
            || y < bounds.top() - OUTSIDE_SLACK_PX
            || y > bounds.bottom() - extra + OUTSIDE_SLACK_PX
        {
            return None;
        }

        let rest_y = match open {
            Some(_) if y >= open_bottom => y - extra,
            Some(_) if y > open_top => open_top,
            _ => y,
        };
        // With no open gutter `open_bottom` is infinite, so nothing shifts.
        let rest_rect = |el: &Element| {
            let rect = el.get_bounding_client_rect();
            let shift = if rect.top() >= open_bottom {
                extra
            } else {
                0.0
            };
            (rect.top() - shift, rect.bottom() - shift)
        };

        // Gutters are never hit directly here - they are implied by the edges
        // of the rows around them. The empty-state slot is not a gutter, so it
        // stays in the running.
        //
        // Nearest row rather than strictly-containing row, so the thin seams
        // between rows always resolve to something. A pointer that resolves to
        // nothing is what let the gap collapse mid-drag.
        let mut best: Option<(Element, f64, f64)> = None;
        let mut best_dist = f64::INFINITY;
        for el in query_all::<Element>(&container, "[data-dropzone]:not(.insert-gutter)") {
            let (top, bottom) = rest_rect(&el);
            let dist = if rest_y < top {
                top - rest_y
            } else if rest_y > bottom {
                rest_y - bottom
            } else {
                0.0
            };
            if dist < best_dist {
                best_dist = dist;
                best = Some((el, top, bottom));
            }
            if dist == 0.0 {
                break;
            }
        }
        let (el, top, bottom) = best?;

        let edge_above = (bottom - top) * EDGE_FRACTION_ABOVE;
        let edge_below = (bottom - top) * EDGE_FRACTION_BELOW;
        // Not `f64::clamp`, which panics when the bounds cross or either is
        // NaN; a degenerate rect should just resolve somewhere harmless.
        let clamped = rest_y.max(top).min(bottom);
        // `<=` so that the band catches the pointer sitting in the collapsed
        // gutter itself even if it is narrowed to zero: nothing there is
        // inside a row, so it clamps to exactly the row's top edge.
        let above = clamped - top <= edge_above;

        match zone_of(&el)? {
            DropZone::MergeInto(idx) if above => Some(DropZone::InsertAt(idx)),
            DropZone::MergeInto(idx) if bottom - clamped < edge_below => {
                Some(DropZone::InsertAt(idx + 1))
            }
            // The top edge of the unranked pile reads as "new last rank", not
            // "put it back in the pile".
            DropZone::Unranked if above => Some(DropZone::InsertAt(
                query_all::<Element>(&container, r#"[data-dropzone^="merge:"]"#).len(),
            )),
            zone => Some(zone),
        }
    }
}

#[cfg_attr(not(feature = "web"), allow(dead_code))]
const RANKER_SELECTOR: &str = ".tier-ranker";

/// The FLIP identity of a chip, stable across re-parenting - and shared with
/// the drag ghost, which is how a release knows to interpolate from the
/// pointer instead of from the slot the faded original still occupies.
fn flip_id(option_id: i64) -> String {
    format!("chip-{option_id}")
}

#[cfg(feature = "web")]
fn flip_snapshot(option_id: i64) {
    dom::flip_snapshot(RANKER_SELECTOR, &flip_id(option_id));
}
#[cfg(not(feature = "web"))]
fn flip_snapshot(_option_id: i64) {}

#[cfg(feature = "web")]
fn flip_play() {
    dom::flip_play(RANKER_SELECTOR);
}
#[cfg(not(feature = "web"))]
fn flip_play() {}

#[cfg(feature = "web")]
fn dropzone_at(x: f64, y: f64) -> Option<DropZone> {
    dom::dropzone_at(RANKER_SELECTOR, x, y)
}
#[cfg(not(feature = "web"))]
fn dropzone_at(_x: f64, _y: f64) -> Option<DropZone> {
    None
}

#[cfg(feature = "web")]
fn capture_pointer(pointer_id: i32) {
    dom::capture_pointer(RANKER_SELECTOR, pointer_id);
}
#[cfg(not(feature = "web"))]
fn capture_pointer(_pointer_id: i32) {}

/// Removes `option_id` from wherever it currently lives in `tiers` (dropping
/// the tier if that empties it) and inserts it at `zone`.
///
/// Returns the index of a rank that came into existence as a result, if any,
/// for the caller to animate in.
#[must_use]
fn move_option(tiers: &mut Vec<Vec<i64>>, option_id: i64, zone: DropZone) -> Option<usize> {
    let mut removed_idx = None;
    for (i, tier) in tiers.iter_mut().enumerate() {
        if let Some(pos) = tier.iter().position(|&id| id == option_id) {
            tier.remove(pos);
            removed_idx = Some(i);
            break;
        }
    }

    let mut target = zone;
    // The index of the chip's own rank, if moving out of it left it empty
    // and dropped it.
    let mut vacated = None;
    if let Some(removed_idx) = removed_idx
        && tiers[removed_idx].is_empty()
    {
        tiers.remove(removed_idx);
        vacated = Some(removed_idx);
        target = match target {
            DropZone::MergeInto(i) if i > removed_idx => DropZone::MergeInto(i - 1),
            DropZone::MergeInto(i) if i == removed_idx => DropZone::InsertAt(i),
            DropZone::InsertAt(i) if i > removed_idx => DropZone::InsertAt(i - 1),
            other => other,
        };
    }

    let created = match target {
        DropZone::Unranked => None,
        DropZone::MergeInto(i) => match tiers.get_mut(i) {
            Some(tier) => {
                tier.push(option_id);
                None
            }
            None => {
                tiers.push(vec![option_id]);
                Some(tiers.len() - 1)
            }
        },
        DropZone::InsertAt(i) => {
            let at = i.min(tiers.len());
            tiers.insert(at, vec![option_id]);
            Some(at)
        }
    };

    // A rank landing at the index its own emptied rank just vacated takes
    // over the row that went away - the list comes out unchanged, so nothing
    // arrived and there is nothing to animate in.
    created.filter(|&at| vacated != Some(at))
}

#[component]
pub fn TierRanker(options: Vec<OptionView>, tiers: Signal<Vec<Vec<i64>>>) -> Element {
    let mut tiers = tiers;
    let mut press = use_signal(|| None::<DragState>);
    let mut dragging = use_signal(|| None::<DragState>);
    let mut hover = use_signal(|| None::<DropZone>);
    let mut selected = use_signal(|| None::<i64>);
    let mut drops = use_signal(|| 0u32);
    // Set for the render that lands a chip, cleared when the next
    // interaction begins - see `.tier-ranker.landing` in the stylesheet.
    let mut landing = use_signal(|| false);
    // The rank that landing just brought into existence, if any. Cleared on
    // the same schedule, so re-creating a rank at the same index re-triggers
    // its entrance rather than finding the class already applied.
    let mut new_rank = use_signal(|| None::<usize>);

    // Re-plays the FLIP animation any time the ranking itself changes,
    // regardless of whether the change came from a drag or a tap - both
    // paths call `flip_snapshot()` immediately before mutating `tiers`.
    // `drops` covers the one case where a chip moves on screen without
    // `tiers` changing: a release that puts it back where it started.
    use_effect(move || {
        let _ = tiers();
        let _ = drops();
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
    // `picking` is tap-to-rank: a chip is armed and waiting to be told where
    // to go, with no pointer held down to open a gap by hovering. Every
    // gutter opens at once so all of them can be tapped. A drag deliberately
    // does not get this - there the single gap opening under the chip is the
    // affordance, and a column of identical targets would compete with it.
    // Crossing the drag threshold clears the selection, so the two are
    // already mutually exclusive; the check on `dragging` is a backstop.
    let mut ranker_class = String::from("tier-ranker");
    if dragging.read().is_none() && selected.read().is_some() {
        ranker_class.push_str(" picking");
    }
    if landing() {
        ranker_class.push_str(" landing");
    }

    let mut commit_move = move |option_id: i64, zone: DropZone| {
        landing.set(true);
        flip_snapshot(option_id);
        new_rank.set(tiers.with_mut(|t| move_option(t, option_id, zone)));
    };

    let mut place_selected = move |zone: DropZone| {
        if let Some(id) = selected() {
            commit_move(id, zone);
            selected.set(None);
            hover.set(None);
        }
    };

    let mut on_pointer_move = move |x: f64, y: f64| {
        if let Some(p) = press()
            && dragging.read().is_none()
        {
            let (dx, dy) = (x - p.x, y - p.y);
            if (dx * dx + dy * dy).sqrt() > DRAG_THRESHOLD_PX {
                dragging.set(Some(DragState { x, y, ..p }));
                press.set(None);
                landing.set(false);
                new_rank.set(None);
                // A tap that turns into a drag was never a tap: drop out of
                // tap-to-rank rather than leaving a column of + targets
                // open behind the chip now under the pointer. `hover` needs
                // no reset - the drag re-derives it further down this same
                // call, from the position that just crossed the threshold.
                selected.set(None);
            }
        }
        if dragging.read().is_some() {
            // The ghost follows the pointer wherever it goes, off the ranker
            // included: taking the chip away from the pointer at the boundary
            // reads as losing control of it.
            dragging.with_mut(|d| {
                if let Some(d) = d {
                    d.x = x;
                    d.y = y;
                }
            });
            // Sticky, so the drag never runs out of a destination: once out
            // of bounds resolves to nothing, the chip keeps the last target
            // it was over and stays committed to it until the pointer finds
            // another. Dragged off the bottom, that is the unranked pile -
            // the lowest thing there is to be over.
            if let Some(zone) = dropzone_at(x, y) {
                hover.set(Some(zone));
            }
        } else if selected.read().is_some() {
            // Lets a mouse preview (and the gutters spread open toward) the
            // target the current tap-selection would land on, without a
            // second click.
            hover.set(dropzone_at(x, y));
        }
    };

    // Placement lives here rather than on a per-zone `onclick` for two
    // reasons: it lets drag and tap share one hit-test (`dropzone_at`), and
    // once gutters resize under the pointer a mousedown/mouseup pair can
    // land on different elements, so no `click` event would fire at all.
    let mut on_pointer_up = move |x: f64, y: f64| {
        if let Some(drag) = dragging() {
            // Snapshot while the ghost is still mounted, so the chip glides
            // in from wherever the pointer let go of it - including from off
            // the ranker entirely - rather than from the slot its faded
            // original was still nominally occupying.
            landing.set(true);
            flip_snapshot(drag.option_id);
            if let Some(zone) = hover() {
                new_rank.set(tiers.with_mut(|t| move_option(t, drag.option_id, zone)));
            }
            // Re-runs the FLIP effect even in the corner where `hover` is
            // empty and `tiers` therefore does not change, so the chip still
            // glides back to where it came from rather than blinking there.
            drops += 1;
            dragging.set(None);
            hover.set(None);
            press.set(None);
            return;
        }

        // `press` is only ever set by a chip's `onpointerdown` (which stops
        // propagation), so its presence here means this gesture started on
        // a chip - i.e. this pointerup is a tap-to-select, not a placement.
        if let Some(p) = press() {
            press.set(None);
            if selected() == Some(p.option_id) {
                selected.set(None);
                hover.set(None);
            } else {
                selected.set(Some(p.option_id));
                landing.set(false);
                new_rank.set(None);
            }
            return;
        }

        // The gesture started on the ranker itself (a zone or gutter): if a
        // chip is selected, this tap places it.
        if selected().is_some() {
            match dropzone_at(x, y) {
                Some(zone) => place_selected(zone),
                None => {
                    selected.set(None);
                    hover.set(None);
                }
            }
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

    let zone_class = move |idx: usize| -> &'static str {
        if hover() == Some(DropZone::MergeInto(idx)) {
            "tier-zone drop-hover"
        } else if new_rank() == Some(idx) {
            "tier-zone rank-appearing"
        } else {
            "tier-zone"
        }
    };

    let gutter_class = move |idx: usize| -> &'static str {
        if hover() == Some(DropZone::InsertAt(idx)) {
            "insert-gutter drop-hover"
        } else {
            "insert-gutter"
        }
    };

    rsx! {
        div {
            class: ranker_class,
            onpointerdown: move |_| {
                // Chips stop_propagation on their own onpointerdown, so this
                // only fires when the gesture did NOT start on a chip - it
                // guards against a stale `press` being mistaken for a tap on
                // the next gesture.
                press.set(None);
            },
            onpointermove: move |evt| {
                let p = evt.data().client_coordinates();
                on_pointer_move(p.x, p.y);
            },
            onpointerup: move |evt| {
                let p = evt.data().client_coordinates();
                on_pointer_up(p.x, p.y);
            },
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
                        class: gutter_class(idx),
                        "data-dropzone": DropZone::InsertAt(idx).attr(),
                    }
                    div {
                        class: zone_class(idx),
                        "data-dropzone": DropZone::MergeInto(idx).attr(),
                        div { class: "tier-zone-label", "Rank {rank_label}" }
                        div { class: "chip-row",
                            for id in tier {
                                OptionChip {
                                    key: "{id}",
                                    label: label_of(id),
                                    flip_id: flip_id(id),
                                    class: chip_class(id),
                                    onpointerdown: move |evt: PointerEvent| {
                                        evt.stop_propagation();
                                        capture_pointer(evt.data().pointer_id());
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

            if tier_count > 0 {
                div {
                    class: gutter_class(tier_count),
                    "data-dropzone": DropZone::InsertAt(tier_count).attr(),
                }
            } else {
                div {
                    class: if hover() == Some(DropZone::InsertAt(0)) { "empty-drop-slot drop-hover" } else { "empty-drop-slot" },
                    "data-dropzone": DropZone::InsertAt(0).attr(),
                    div { class: "tier-zone-label", "Drag items here" }
                    div { class: "chip-row" }
                }
            }

            div {
                class: if hover() == Some(DropZone::Unranked) { "tier-zone unranked-zone drop-hover" } else { "tier-zone unranked-zone" },
                "data-dropzone": DropZone::Unranked.attr(),
                div { class: "tier-zone-label", "Unranked options" }
                div { class: "chip-row",
                    for id in unranked {
                        OptionChip {
                            key: "{id}",
                            label: label_of(id),
                            flip_id: flip_id(id),
                            class: chip_class(id),
                            onpointerdown: move |evt: PointerEvent| {
                                evt.stop_propagation();
                                capture_pointer(evt.data().pointer_id());
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
                    class: "option-chip chip-ghost",
                    // Not a FLIP subject itself (it is gone by the time
                    // `flip_play` runs) - the id is here so the release
                    // snapshot reads the chip's position off the pointer.
                    "data-flip-id": flip_id(drag.option_id),
                    "data-flip-lift": "true",
                    style: "left: {drag.x}px; top: {drag.y}px;",
                    span { class: "option-chip-label", "{label_of(drag.option_id)}" }
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
        assert_eq!(move_option(&mut tiers, 1, DropZone::InsertAt(0)), Some(0));
        assert_eq!(tiers, vec![vec![1]]);
    }

    #[test]
    fn moving_the_only_member_out_deletes_the_tier() {
        let mut tiers = vec![vec![1], vec![2]];
        assert_eq!(move_option(&mut tiers, 1, DropZone::MergeInto(1)), None);
        assert_eq!(tiers, vec![vec![2, 1]]);
    }

    #[test]
    fn moving_back_onto_its_own_now_empty_tier_recreates_it() {
        let mut tiers = vec![vec![1]];
        // The row reappears exactly where it was, so nothing arrives.
        assert_eq!(move_option(&mut tiers, 1, DropZone::MergeInto(0)), None);
        assert_eq!(tiers, vec![vec![1]]);
    }

    #[test]
    fn dropping_into_unranked_removes_without_reinserting() {
        let mut tiers = vec![vec![1, 2]];
        assert_eq!(move_option(&mut tiers, 1, DropZone::Unranked), None);
        assert_eq!(tiers, vec![vec![2]]);
    }

    #[test]
    fn moving_across_tiers_shifts_later_indices_down() {
        let mut tiers = vec![vec![1], vec![2], vec![3]];
        // Move the sole member of tier 0 into tier 2; tier 0 disappears so
        // the old "tier 2" target shifts down to index 1.
        assert_eq!(move_option(&mut tiers, 1, DropZone::MergeInto(2)), None);
        assert_eq!(tiers, vec![vec![2], vec![3, 1]]);
    }

    #[test]
    fn insert_at_gap_creates_a_new_tier_between_others() {
        let mut tiers = vec![vec![1], vec![2]];
        assert_eq!(move_option(&mut tiers, 3, DropZone::InsertAt(1)), Some(1));
        assert_eq!(tiers, vec![vec![1], vec![3], vec![2]]);
    }

    #[test]
    fn insert_at_start_creates_a_new_top_tier() {
        let mut tiers = vec![vec![1], vec![2]];
        assert_eq!(move_option(&mut tiers, 3, DropZone::InsertAt(0)), Some(0));
        assert_eq!(tiers, vec![vec![3], vec![1], vec![2]]);
    }

    #[test]
    fn insert_at_end_appends_a_new_tier() {
        let mut tiers = vec![vec![1], vec![2]];
        assert_eq!(move_option(&mut tiers, 3, DropZone::InsertAt(2)), Some(2));
        assert_eq!(tiers, vec![vec![1], vec![2], vec![3]]);
    }

    #[test]
    fn moving_sole_member_below_its_own_tier_shifts_gap_index_down() {
        let mut tiers = vec![vec![1], vec![2]];
        // "Insert after tier 0" (gap index 1) once tier 0 vanishes becomes
        // gap index 0 - i.e. still directly before what's now the only tier.
        // The list comes out unchanged, so no rank is reported as arriving.
        assert_eq!(move_option(&mut tiers, 1, DropZone::InsertAt(1)), None);
        assert_eq!(tiers, vec![vec![1], vec![2]]);
    }

    #[test]
    fn leaving_a_rank_empty_to_start_a_new_one_below_it_does_arrive() {
        let mut tiers = vec![vec![1], vec![2]];
        // Tier 0 empties and vanishes, and the chip lands one row further
        // down than the row that went away - so a rank genuinely arrives.
        assert_eq!(move_option(&mut tiers, 1, DropZone::InsertAt(2)), Some(1));
        assert_eq!(tiers, vec![vec![2], vec![1]]);
    }

    #[test]
    fn merging_past_the_end_appends_and_reports_the_new_tier() {
        let mut tiers = vec![vec![1]];
        assert_eq!(move_option(&mut tiers, 2, DropZone::MergeInto(9)), Some(1));
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
