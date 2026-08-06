use dioxus::prelude::*;

use components::Navbar;
use views::{Create, Home, Results, Vote};

#[cfg(feature = "server")]
mod basic_auth;
mod components;
mod nav_cache;
mod nav_guard;
mod unsaved_guard;
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
const DX_COMPONENTS_THEME_CSS: Asset = asset!("/assets/dx-components-theme.css");

#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    let provider = api::init();
    api::db::init_pool()
        .await
        .expect("failed to initialize database pool");

    let mut app = dioxus::server::router(App);

    // Basic Auth is enabled in every environment except production. It is
    // disabled only when BASIC_AUTH_ENABLED is explicitly set to "false".
    if !matches!(std::env::var("BASIC_AUTH_ENABLED").as_deref(), Ok("false")) {
        let username = std::env::var("BASIC_AUTH_USERNAME")
            .ok()
            .filter(|s| !s.is_empty());
        let password = std::env::var("BASIC_AUTH_PASSWORD")
            .ok()
            .filter(|s| !s.is_empty());
        let (Some(username), Some(password)) = (username, password) else {
            panic!(
                "Basic Auth is enabled for this environment. Both \
                 BASIC_AUTH_USERNAME and BASIC_AUTH_PASSWORD must be set and \
                 non-empty, or set BASIC_AUTH_ENABLED=false to disable it."
            );
        };
        app = app.layer(tower_http::auth::AsyncRequireAuthorizationLayer::new(
            basic_auth::BasicAuth::new(username, password),
        ));
    }

    // Trace every request with an OpenTelemetry span. Registered before the
    // /health route below so that health checks are not traced.
    app = app.layer(
        tower_http::trace::TraceLayer::new_for_http()
            .make_span_with(|request: &http::Request<_>| {
                let path = request.uri().path();
                if path.starts_with("/assets") || path.starts_with("/wasm") {
                    return tracing::Span::none();
                }
                tracing::info_span!(
                    "http.request",
                    http.request.method = %request.method(),
                    url.path = %path,
                    http.response.status_code = tracing::field::Empty,
                )
            })
            .on_response(
                |response: &http::Response<_>,
                 _latency: std::time::Duration,
                 span: &tracing::Span| {
                    span.record("http.response.status_code", response.status().as_u16());
                },
            ),
    );

    let app = app.route("/health", dioxus::server::axum::routing::get(health));
    let address = dioxus::cli_config::fullstack_address_or_localhost();
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    dioxus::server::axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    if let Some(provider) = provider
        && let Err(e) = provider.shutdown()
    {
        eprintln!("tracer provider shutdown failed: {e}");
    }
}

#[cfg(feature = "server")]
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(feature = "server")]
async fn health() -> &'static str {
    "OK"
}

#[cfg(not(feature = "server"))]
fn main() {
    // Before `launch`, so the unsaved-changes `popstate` listener is registered
    // ahead of the one the router installs when it first renders.
    #[cfg(feature = "web")]
    unsaved_guard::guards::install();

    dioxus::launch(App);
}

/// Applies the saved theme to `<html>` before first paint. Runs as an inline
/// `<head>` script (part of the server-rendered HTML, not a post-hydration
/// `document::eval`) so a page load never flashes the wrong theme -
/// `components::theme::ThemeToggle` only needs to keep this in sync after
/// the user changes it.
const THEME_PREVENT_FLASH_JS: &str = r#"
(function () {
    try {
        var theme = localStorage.getItem("theme");
        if (theme === "light" || theme === "dark") {
            document.documentElement.setAttribute("data-theme", theme);
        }
    } catch (e) {}
})();
"#;

#[component]
fn App() -> Element {
    rsx! {
        // Global app resources
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: DX_COMPONENTS_THEME_CSS }
        document::Script { "{THEME_PREVENT_FLASH_JS}" }

        Router::<Route> { config: nav_guard::config }
    }
}
