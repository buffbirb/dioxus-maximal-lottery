use dioxus::prelude::*;

use crate::Route;
use crate::components::theme::ThemeToggle;
use crate::nav_guard::use_track_route;

const NAVBAR_CSS: Asset = asset!("/assets/navbar.css");

#[component]
pub fn Navbar() -> Element {
    // Every route renders through this layout, so it's the one place
    // guaranteed to see each navigation - including the very first, which
    // the router's `on_update` callback (the rest of the navigate-away
    // guard) never gets called for.
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
