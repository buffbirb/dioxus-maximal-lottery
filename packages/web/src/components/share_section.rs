//! Share URL display + copy-to-clipboard. The absolute URL is assembled
//! client-side from `window.location.origin` + the given path so the same
//! component works regardless of what host the app is served from.
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

const COPY_FEEDBACK_MS: u32 = 3000;

#[component]
pub fn ShareSection(path: String) -> Element {
    let mut origin = use_signal(String::new);
    let mut copied = use_signal(|| false);

    use_effect(move || {
        spawn(async move {
            let result: Result<String, _> = document::eval("return window.location.origin;")
                .join()
                .await;
            if let Ok(value) = result {
                origin.set(value);
            }
        });
    });

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
