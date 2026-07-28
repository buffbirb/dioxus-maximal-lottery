//! Shared option "chip" used both as a draggable tile in the tier ranker and
//! as a clickable row in the results standings.
use dioxus::prelude::*;

#[component]
pub fn OptionChip(
    label: String,
    #[props(default)] class: String,
    #[props(default)] percentage: Option<String>,
    /// Stable id for the FLIP-animated tier ranker to track this chip across
    /// re-renders. Left empty for non-draggable uses (e.g. results standings).
    #[props(default)]
    flip_id: String,
    #[props(default)] onclick: Option<EventHandler<MouseEvent>>,
    #[props(default)] onpointerdown: Option<EventHandler<PointerEvent>>,
) -> Element {
    rsx! {
        div {
            class: "option-chip {class}",
            "data-flip-id": "{flip_id}",
            onclick: move |evt| {
                if let Some(handler) = &onclick {
                    handler.call(evt);
                }
            },
            onpointerdown: move |evt| {
                if let Some(handler) = &onpointerdown {
                    handler.call(evt);
                }
            },
            span { class: "option-chip-label", "{label}" }
            if let Some(pct) = &percentage {
                span { class: "option-chip-pct", "{pct}" }
            }
        }
    }
}
