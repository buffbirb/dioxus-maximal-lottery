use dioxus::prelude::*;

use crate::Route;
use crate::components::theme::ThemeToggle;
use crate::nav_guard::use_track_route;

const NAVBAR_CSS: Asset = asset!("/assets/navbar.css");

#[component]
pub fn Navbar() -> Element {
    // Track the current route for the unsaved-changes guard.
    use_track_route();

    rsx! {
        document::Link { rel: "stylesheet", href: NAVBAR_CSS }

        nav { id: "navbar",
            div { class: "navbar-left",
                Link { to: Route::Home {}, class: "navbar-logo", "Maximal Lottery" }
                Link { to: Route::Create {}, class: "navbar-link", "Create" }
            }
            div { class: "navbar-right", ThemeToggle {} }
        }

        Outlet::<Route> {}
    }
}
