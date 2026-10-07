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
            div { class: "navbar-right",
                // Its own boundary so pages never wait on the user lookup.
                SuspenseBoundary { fallback: |_| rsx! {}, UserMenu {} }
                ThemeToggle {}
            }
        }

        Outlet::<Route> {}
    }
}

#[component]
fn UserMenu() -> Element {
    let user = use_server_future(api::auth::current_user)?;
    let route = use_route::<Route>();

    match &*user.read() {
        Some(Ok(Some(user))) => rsx! {
            div { class: "user-menu",
                if let Some(avatar_url) = &user.avatar_url {
                    img {
                        class: "user-avatar",
                        src: "{avatar_url}",
                        alt: "",
                        width: "28",
                        height: "28",
                    }
                }
                span { class: "user-name", "{user.display_name}" }
                // A native POST, not a server function: sign-out must clear
                // the session cookie, which only the server route sets.
                form { method: "post", action: "/logout",
                    button { class: "signout-button", r#type: "submit", "Sign out" }
                }
            }
        },
        // Anonymous, or the lookup failed: offer sign-in either way.
        _ => rsx! {
            Link {
                to: Route::Login {
                    return_to: Some(return_path(&route)),
                },
                class: "navbar-link",
                "Sign in"
            }
        },
    }
}

/// On the login page itself, keep its pending destination.
fn return_path(route: &Route) -> String {
    match route {
        Route::Login { return_to } => return_to.clone().unwrap_or_else(|| "/".to_string()),
        other => other.to_string(),
    }
}
