use chrono::{DateTime, Utc};
use dioxus::prelude::*;

use api::model::CreatePollRequest;

use crate::Route;

const MAX_TITLE_LEN: usize = 200;
const MAX_DESCRIPTION_LEN: usize = 2000;
const MIN_OPTIONS: usize = 2;
const MAX_OPTION_LEN: usize = 200;

async fn parse_local_datetime(value: &str) -> Option<DateTime<Utc>> {
    let script = format!(
        r#"const d = new Date("{}"); return isNaN(d.getTime()) ? null : d.getTime();"#,
        value.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let millis: Option<i64> = document::eval(&script).join().await.ok()?;
    millis.and_then(DateTime::from_timestamp_millis)
}

#[component]
pub fn Create() -> Element {
    let mut title = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut options = use_signal(|| vec![String::new(), String::new()]);
    let mut show_additional = use_signal(|| false);
    let mut deadline_enabled = use_signal(|| false);
    let mut deadline_input = use_signal(String::new);
    let mut hide_results = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut submitting = use_signal(|| false);

    let navigator = use_navigator();

    let on_submit = move |_| {
        let title_value = title().trim().to_string();
        let description_value = description().trim().to_string();
        let option_values: Vec<String> = options()
            .iter()
            .map(|o| o.trim().to_string())
            .filter(|o| !o.is_empty())
            .collect();

        if title_value.is_empty() || title_value.chars().count() > MAX_TITLE_LEN {
            error.set(Some(format!("title must be 1-{MAX_TITLE_LEN} characters")));
            return;
        }
        if description_value.chars().count() > MAX_DESCRIPTION_LEN {
            error.set(Some(format!(
                "description must be at most {MAX_DESCRIPTION_LEN} characters"
            )));
            return;
        }
        if option_values.len() < MIN_OPTIONS {
            error.set(Some(format!("a poll needs at least {MIN_OPTIONS} options")));
            return;
        }
        if deadline_enabled() && deadline_input().is_empty() {
            error.set(Some("a deadline is required when enabled".to_string()));
            return;
        }

        error.set(None);
        submitting.set(true);

        spawn(async move {
            let deadline = if deadline_enabled() {
                match parse_local_datetime(&deadline_input()).await {
                    Some(dt) => Some(dt),
                    None => {
                        error.set(Some("please enter a valid deadline".to_string()));
                        submitting.set(false);
                        return;
                    }
                }
            } else {
                None
            };

            let request = CreatePollRequest {
                title: title_value,
                description: (!description_value.is_empty()).then_some(description_value),
                options: option_values,
                deadline,
                hide_results: deadline_enabled() && hide_results(),
            };

            match api::polls::create_poll(request).await {
                Ok(share_id) => {
                    navigator.push(Route::Vote { share_id });
                }
                Err(err) => {
                    error.set(Some(err.to_string()));
                    submitting.set(false);
                }
            }
        });
    };

    rsx! {
        div { id: "create",
            h1 { "Create a poll" }
            p { class: "subtitle",
                "Add your options, share the link, and let the maximal lottery pick a fair winner."
            }

            div { class: "field",
                label { r#for: "title", "Title" }
                input {
                    id: "title",
                    maxlength: "{MAX_TITLE_LEN}",
                    value: "{title}",
                    oninput: move |evt| title.set(evt.value()),
                }
                span { class: "char-counter", "{title().chars().count()}/{MAX_TITLE_LEN}" }
            }

            div { class: "field",
                label { r#for: "description", "Description" }
                textarea {
                    id: "description",
                    maxlength: "{MAX_DESCRIPTION_LEN}",
                    value: "{description}",
                    oninput: move |evt| description.set(evt.value()),
                }
                span { class: "char-counter", "{description().chars().count()}/{MAX_DESCRIPTION_LEN}" }
            }

            div { class: "field",
                label { "Options" }
                for (idx, option) in options().into_iter().enumerate() {
                    div { class: "option-entry", key: "{idx}",
                        input {
                            maxlength: "{MAX_OPTION_LEN}",
                            placeholder: "Option {idx + 1}",
                            value: "{option}",
                            oninput: move |evt| options.with_mut(|o| o[idx] = evt.value()),
                        }
                        if options().len() > MIN_OPTIONS {
                            button {
                                class: "option-delete",
                                r#type: "button",
                                "aria-label": "Remove option",
                                onclick: move |_| {
                                    options
                                        .with_mut(|o| {
                                            o.remove(idx);
                                        });
                                },
                                "\u{2715}"
                            }
                        }
                    }
                }
                button {
                    class: "add-option-btn",
                    r#type: "button",
                    onclick: move |_| options.with_mut(|o| o.push(String::new())),
                    "+ Add option"
                }
            }

            details { class: "additional-settings", open: show_additional(),
                summary {
                    onclick: move |evt| {
                        evt.prevent_default();
                        show_additional.set(!show_additional());
                    },
                    "Additional settings"
                }

                div { class: "field toggle-field",
                    label { class: "switch",
                        input {
                            r#type: "checkbox",
                            checked: deadline_enabled(),
                            onchange: move |evt| deadline_enabled.set(evt.checked()),
                        }
                        span { class: "switch-slider" }
                    }
                    div {
                        span { class: "toggle-label", "Deadline" }
                        p { class: "flavor-text",
                            "Close voting and reveal a fixed cutoff for this poll."
                        }
                    }
                }

                if deadline_enabled() {
                    div { class: "field",
                        label { r#for: "deadline", "Deadline" }
                        input {
                            id: "deadline",
                            r#type: "datetime-local",
                            value: "{deadline_input}",
                            oninput: move |evt| deadline_input.set(evt.value()),
                        }
                    }

                    div { class: "field toggle-field",
                        label { class: "switch",
                            input {
                                r#type: "checkbox",
                                checked: hide_results(),
                                onchange: move |evt| hide_results.set(evt.checked()),
                            }
                            span { class: "switch-slider" }
                        }
                        div {
                            span { class: "toggle-label", "Hide results until deadline" }
                            p { class: "flavor-text",
                                "Voters won't see standings until the poll closes."
                            }
                        }
                    }
                }
            }

            if let Some(err) = error() {
                p { class: "form-error", "{err}" }
            }

            button {
                class: "cta-button",
                r#type: "button",
                disabled: submitting(),
                onclick: on_submit,
                if submitting() {
                    "Creating..."
                } else {
                    "Create poll"
                }
            }
        }
    }
}
