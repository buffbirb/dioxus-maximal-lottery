use dioxus::prelude::*;

#[component]
pub fn OptionChip(
    label: String,
    onclick: Option<EventHandler<()>>,
    class: Option<String>,
) -> Element {
    let cls = class.unwrap_or_default();

    rsx! {
        span {
            class: "option-chip {cls}",
            onclick: move |_| {
                if let Some(ref handler) = onclick {
                    handler.call(());
                }
            },
            "{label}"
        }
    }
}
