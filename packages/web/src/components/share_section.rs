//! Share URL display + copy-to-clipboard. The absolute URL is assembled
//! client-side from `window.location.origin` + the given path so the same
//! component works regardless of what host the app is served from.
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

const COPY_FEEDBACK_MS: u32 = 1200;

// Read synchronously off `web_sys` rather than round-tripping through
// `document::eval` (a message-passed async call), so the origin lands on the
// render right after hydration instead of an async hop later.
#[cfg(feature = "web")]
fn window_origin() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default()
}
#[cfg(not(feature = "web"))]
fn window_origin() -> String {
    String::new()
}

#[component]
pub fn ShareSection(path: String) -> Element {
    let mut origin = use_signal(String::new);
    let mut copied = use_signal(|| false);

    // Deliberately an effect, not the signal's initializer. The server has no
    // `window`, so it renders `value="{path}"`; the client then hydrates with
    // `skip_mutations` set, dropping every write the first rebuild produces -
    // so an origin computed there would sit in the VirtualDom while the DOM
    // kept the server's path-only URL, and no later diff would ever correct it
    // (both sides already agree). Effects run after that rebuild, so this
    // "" -> origin change is a real diff and does reach the input.
    use_effect(move || origin.set(window_origin()));

    let url = format!("{}{}", origin(), path);

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
