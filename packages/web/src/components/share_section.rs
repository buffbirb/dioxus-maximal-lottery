//! Share URL display + copy-to-clipboard. The absolute URL is assembled on the
//! server from the request's own headers (see `crate::origin`) and handed to
//! the client through the hydration payload, so it is already complete in the
//! server-rendered HTML and the same component still works regardless of what
//! host the app is served from.
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

const COPY_FEEDBACK_MS: u32 = 1200;

/// The origin every share link is built on.
///
/// `use_server_cached` below runs this on the server and replays the result on
/// the client from the hydration payload, so the branches are not two answers
/// to the same question: the server branch is the one that renders, and the
/// client branch only runs when this component mounts *after* hydration - the
/// modal in `views::vote`, which the server never rendered and so has nothing
/// cached for. There `window` is available synchronously and no wait exists to
/// paper over.
fn current_origin() -> String {
    // Ordered `server` first to match `use_server_cached` itself: a build with
    // both features on (`--all-features`) is a server build, and calling into
    // `web_sys` off-wasm would be the wrong answer even where it links.
    #[cfg(feature = "server")]
    {
        dioxus::fullstack::FullstackContext::current()
            .and_then(|context| crate::origin::derive_origin(&context.parts_mut()))
            .unwrap_or_default()
    }

    // Read straight off `web_sys` rather than round-tripping through
    // `document::eval`, a message-passed async call: this way the origin is
    // there on the first render rather than an async hop later.
    #[cfg(all(feature = "web", not(feature = "server")))]
    {
        web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_default()
    }

    #[cfg(not(any(feature = "server", feature = "web")))]
    {
        String::new()
    }
}

#[component]
pub fn ShareSection(path: String) -> Element {
    let mut copied = use_signal(|| false);

    // Empty only if the request carried no host worth trusting, which leaves a
    // path-only URL on display - the same degraded state as a browser that
    // hands back no origin, and better than a blank field or a confidently
    // wrong absolute one.
    let origin = use_server_cached(current_origin);
    let url = format!("{origin}{path}");

    rsx! {
        div { class: "share-section",
            span { class: "share-section-label", "Share this poll" }
            div { class: "share-section-row",
                input {
                    class: "share-url-input",
                    readonly: true,
                    value: "{url}",
                    onclick: move |evt| evt.stop_propagation(),
                }
                button {
                    class: "share-copy-btn",
                    r#type: "button",
                    "aria-label": "Copy link",
                    title: if copied() { "Copied!" } else { "Copy to clipboard" },
                    onclick: move |_| {
                        let url = url.clone();
                        spawn(async move {
                            let eval = document::eval(
                                r#"let url = await dioxus.recv(); await navigator.clipboard.writeText(url);"#,
                            );
                            let _ = eval.send(url);
                            let _ = eval.await;
                            copied.set(true);
                            TimeoutFuture::new(COPY_FEEDBACK_MS).await;
                            copied.set(false);
                        });
                    },
                    span { class: "copy-icon-stack",
                        svg {
                            class: if copied() { "copy-icon copy-icon-fade-out" } else { "copy-icon" },
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            rect {
                                x: "8",
                                y: "2",
                                width: "8",
                                height: "4",
                                rx: "1",
                            }
                            path { d: "M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" }
                        }
                        svg {
                            class: if copied() { "copy-icon" } else { "copy-icon copy-icon-fade-out" },
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            path { d: "M20 6 9 17l-5-5" }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[component]
    fn ShareSectionAtPath() -> Element {
        rsx! {
            ShareSection { path: "/p/abc123" }
        }
    }

    fn render_with_host(host: &str) -> String {
        let parts = http::Request::builder()
            .uri("/p/abc123")
            .header("host", host)
            .body(())
            .expect("failed to build request")
            .into_parts()
            .0;

        let mut dom = VirtualDom::new(ShareSectionAtPath);
        // Exactly how dioxus-server seeds an SSR render (see its ssr.rs).
        dom.provide_root_context(dioxus::fullstack::FullstackContext::new(parts));
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    /// The point of the whole exercise: the URL is finished in the HTML the
    /// server sends, rather than corrected afterwards. Asserted on the shape
    /// rather than the exact string so a `PUBLIC_BASE_URL` in the developer's
    /// shell overrides the origin without failing the test.
    #[test]
    fn server_renders_an_absolute_url() {
        let html = render_with_host("example.com");
        let value = html
            .split_once("value=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(value, _)| value)
            .expect("no value attribute in the rendered share section");

        assert!(
            value.starts_with("http://") || value.starts_with("https://"),
            "expected an absolute URL, got {value:?}"
        );
        assert!(
            value.ends_with("/p/abc123"),
            "expected the share path, got {value:?}"
        );
    }

    /// The fade it replaces is gone, rather than left running against a value
    /// that is already correct.
    #[test]
    fn server_render_has_nothing_left_pending() {
        assert!(!render_with_host("example.com").contains("data-pending"));
    }
}
