use dioxus::prelude::*;

#[component]
pub fn Modal(show: bool, onclose: EventHandler<()>, children: Element) -> Element {
    if !show {
        return rsx! {};
    }

    rsx! {
        div { class: "modal-backdrop", onclick: move |_| onclose.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                button { class: "modal-close", onclick: move |_| onclose.call(()), "×" }
                {children}
            }
        }
    }
}
