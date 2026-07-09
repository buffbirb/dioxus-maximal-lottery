use dioxus::prelude::*;

use views::*;

mod components;
mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(WebNavbar)]
    #[route("/")]
    Home {},
    #[route("/create")]
    Create {},
    #[route("/:share_id")]
    Vote { share_id: String },
    #[route("/:share_id/results")]
    Results { share_id: String },
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    let _provider = api::init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://127.0.0.1:5432/postgres".into());
    api::init_pool(&database_url);
    api::run_migrations().await;

    let app = dioxus::server::router(App);
    let address = dioxus::cli_config::fullstack_address_or_localhost();
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    dioxus::server::axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[component]
fn WebNavbar() -> Element {
    let mut theme_signal = components::use_theme();

    rsx! {
        document::Link { rel: "stylesheet", href: asset!("/assets/navbar.css") }

        nav { id: "navbar",
            div { class: "navbar-left",
                Link { to: Route::Home {}, class: "navbar-logo", "MaxiLot" }
                Link { to: Route::Create {}, class: "navbar-link", "Create" }
            }
            button {
                class: "theme-toggle",
                onclick: move |_| {
                    let mut theme = theme_signal.write();
                    components::cycle_theme(&mut theme);
                },
                aria_label: "Toggle theme",
                {components::theme_icon(theme_signal())}
            }
        }

        main { id: "main-content", Outlet::<Route> {} }
    }
}
