//! Shimmering placeholder block shown while a page suspends on data it
//! doesn't have yet (client-side navigation only - a cold load resolves the
//! fetch during SSR and never renders this, see `use_server_future` in
//! `views::vote` and `views::results`).
use dioxus::prelude::*;

#[component]
pub fn Skeleton(#[props(default)] class: String) -> Element {
    rsx! {
        div { class: "skeleton {class}" }
    }
}
