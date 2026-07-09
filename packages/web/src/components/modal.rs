//! Reusable modal overlay. Closes on the `x` button or a backdrop click;
//! clicks inside the content stop propagation so they don't reach the
//! backdrop.
use dioxus::prelude::*;

#[component]
pub fn Modal(on_close: EventHandler<()>, children: Element) -> Element {
    rsx! {
        div { class: "modal-backdrop", onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                onclick: move |evt| evt.stop_propagation(),
                button {
                    class: "modal-close",
                    r#type: "button",
                    "aria-label": "Close",
                    onclick: move |_| on_close.call(()),
                    "\u{2715}"
                }
                {children}
            }
        }
    }
}
