use dioxus::prelude::*;

use components::Navbar;
use views::{Create, Home, Results, Vote};

#[cfg(feature = "server")]
mod basic_auth;
mod components;
mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    #[route("/create")]
    Create {},
    #[route("/:share_id/results")]
    Results { share_id: String },
    #[route("/:share_id")]
    Vote { share_id: String },
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    let _provider = api::init();
    api::db::init_pool()
        .await
        .expect("failed to initialize database pool");

    let mut app = dioxus::server::router(App);

    // Enable Basic Auth for non-prd and non-local environments
    if let (Ok(username), Ok(password)) = (
        std::env::var("BASIC_AUTH_USERNAME"),
        std::env::var("BASIC_AUTH_PASSWORD"),
    ) {
        app = app.layer(tower_http::auth::AsyncRequireAuthorizationLayer::new(
            basic_auth::BasicAuth::new(username, password),
        ));
    }

    let app = app.route("/health", dioxus::server::axum::routing::get(health));
    let address = dioxus::cli_config::fullstack_address_or_localhost();
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    dioxus::server::axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(feature = "server")]
async fn health() -> &'static str {
    "OK"
}

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        // Global app resources
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }

        Router::<Route> {}
    }
}
