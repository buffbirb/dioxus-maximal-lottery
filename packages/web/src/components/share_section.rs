use dioxus::prelude::*;

#[component]
pub fn ShareSection(url: String, label: String) -> Element {
    let mut copied = use_signal(|| false);

    let copy_handler = {
        let url = url.clone();
        move |_| {
            let url = url.clone();
            let success = copy_to_clipboard(&url);
            if success {
                copied.set(true);
                let mut c = copied;
                gloo_timers::callback::Timeout::new(3000, move || {
                    c.set(false);
                })
                .forget();
            }
        }
    };

    rsx! {
        div { class: "share-section",
            span { class: "share-label", "{label}" }
            div { class: "share-input-group",
                input { class: "share-input", value: "{url}", readonly: true }
                button {
                    class: "share-copy-btn",
                    onclick: copy_handler,
                    aria_label: "Copy to clipboard",
                    if copied() {
                        svg {
                            view_box: "0 0 24 24",
                            width: "20",
                            height: "20",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            polyline { points: "20 6 9 17 4 12" }
                        }
                    } else {
                        svg {
                            view_box: "0 0 24 24",
                            width: "20",
                            height: "20",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            rect {
                                x: "9",
                                y: "9",
                                width: "13",
                                height: "13",
                                rx: "2",
                            }
                            path { d: "M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" }
                        }
                    }
                }
            }
        }
    }
}

fn copy_to_clipboard(text: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.navigator().clipboard().write_text(text);
            return true;
        }
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = text;
        false
    }
}
