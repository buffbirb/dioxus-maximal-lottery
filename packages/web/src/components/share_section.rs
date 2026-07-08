//! Share URL display + copy-to-clipboard. The absolute URL is assembled
//! client-side from `window.location.origin` + the given path so the same
//! component works regardless of what host the app is served from.
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

#[component]
pub fn ShareSection(path: String) -> Element {
    let mut origin = use_signal(String::new);
    let mut copied = use_signal(|| false);

    use_effect(move || {
        spawn(async move {
            let result: Result<String, _> =
                document::eval("return window.location.origin;").join().await;
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
                            TimeoutFuture::new(3000).await;
                            copied.set(false);
                        });
                    },
                    if copied() { "\u{2713}" } else { "\u{29C9}" }
                }
            }
        }
    }
}
