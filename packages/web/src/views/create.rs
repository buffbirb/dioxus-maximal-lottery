use chrono::{DateTime, NaiveDateTime, Utc};
use dioxus::prelude::*;

use crate::Route;
use api::*;

#[component]
pub fn Create() -> Element {
    let mut title = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut options = use_signal(|| vec![String::new(), String::new()]);
    let mut deadline_enabled = use_signal(|| false);
    let mut deadline_str = use_signal(String::new);
    let mut hide_results = use_signal(|| false);
    let mut show_settings = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut submitting = use_signal(|| false);
    let nav = use_navigator();

    let add_option = move |_| {
        options.write().push(String::new());
    };

    let submit = move |_| {
        let title_val = title.read().trim().to_string();
        let desc_val = description.read().trim().to_string();
        let desc_val = if desc_val.is_empty() {
            None
        } else {
            Some(desc_val)
        };
        let opts: Vec<String> = options
            .read()
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if title_val.is_empty() {
            error.set(Some("Title is required".into()));
            return;
        }
        if title_val.len() > 200 {
            error.set(Some("Title must be 200 characters or less".into()));
            return;
        }
        if let Some(ref d) = desc_val
            && d.len() > 2000
        {
            error.set(Some("Description must be 2000 characters or less".into()));
            return;
        }
        if opts.len() < 2 {
            error.set(Some("At least 2 options are required".into()));
            return;
        }
        for o in &opts {
            if o.len() > 200 {
                error.set(Some("Each option must be 200 characters or less".into()));
                return;
            }
        }

        let deadline: Option<DateTime<Utc>> = if *deadline_enabled.read() {
            let ds = deadline_str.read().trim().to_string();
            if ds.is_empty() {
                error.set(Some("Deadline is required when enabled".into()));
                return;
            }
            match NaiveDateTime::parse_from_str(&ds, "%Y-%m-%dT%H:%M") {
                Ok(naive) => {
                    let dt = DateTime::from_naive_utc_and_offset(naive, Utc);
                    if dt <= Utc::now() {
                        error.set(Some("Deadline must be in the future".into()));
                        return;
                    }
                    Some(dt)
                }
                Err(_) => {
                    error.set(Some("Invalid deadline format. Use YYYY-MM-DDTHH:MM".into()));
                    return;
                }
            }
        } else {
            None
        };

        error.set(None);
        submitting.set(true);

        let req = CreatePollRequest {
            title: title_val,
            description: desc_val,
            options: opts,
            deadline,
            hide_results: *hide_results.read(),
        };

        spawn(async move {
            match create_poll(req).await {
                Ok(share_id) => {
                    nav.push(Route::Vote { share_id });
                }
                Err(e) => {
                    error.set(Some(format!("Failed to create: {e}")));
                    submitting.set(false);
                }
            }
        });
    };

    let settings_arrow = if show_settings() { "v" } else { ">" };

    rsx! {
        div { class: "create-page",
            h1 { "Create a Poll" }
            p { class: "create-subtitle", "Set up your poll below." }

            if let Some(ref err) = *error.read() {
                div { class: "error-banner", "{err}" }
            }

            div { class: "form-group",
                label { class: "form-label", "Title" }
                input {
                    class: "form-input",
                    r#type: "text",
                    value: "{title}",
                    maxlength: 200,
                    oninput: move |e| title.set(e.value()),
                    placeholder: "What are we deciding?",
                }
                span { class: "char-counter", "{title.read().len()}/200" }
            }

            div { class: "form-group",
                label { class: "form-label", "Description (optional)" }
                textarea {
                    class: "form-textarea",
                    value: "{description}",
                    maxlength: 2000,
                    oninput: move |e| description.set(e.value()),
                    placeholder: "Add context about this poll...",
                    rows: 3,
                }
                span { class: "char-counter", "{description.read().len()}/2000" }
            }

            div { class: "form-group",
                label { class: "form-label", "Options" }
                for (idx, opt) in options.read().iter().enumerate() {
                    div { class: "option-entry",
                        input {
                            class: "form-input",
                            r#type: "text",
                            value: "{opt}",
                            maxlength: 200,
                            oninput: move |e| {
                                options.write()[idx] = e.value();
                            },
                            placeholder: "Option {idx + 1}",
                        }
                        if options.read().len() > 2 {
                            button {
                                class: "option-remove-btn",
                                onclick: move |_| {
                                    if options.read().len() > 2 {
                                        options.write().remove(idx);
                                    }
                                },
                                aria_label: "Remove option",
                                "x"
                            }
                        }
                    }
                }
                button { class: "add-option-btn", onclick: add_option, "+ Add option" }
            }

            div { class: "settings-section",
                button {
                    class: "settings-toggle",
                    onclick: move |_| show_settings.set(!show_settings()),
                    "Additional settings {settings_arrow}"
                }

                if show_settings() {
                    div { class: "settings-content",
                        div { class: "setting-group",
                            label { class: "switch-label",
                                input {
                                    class: "switch-input",
                                    r#type: "checkbox",
                                    checked: *deadline_enabled.read(),
                                    onchange: move |e| deadline_enabled.set(e.checked()),
                                }
                                span { class: "switch-slider" }
                                " Deadline"
                            }
                            p { class: "setting-hint",
                                "Set a cutoff time after which votes are no longer accepted."
                            }
                        }

                        if deadline_enabled() {
                            div { class: "setting-group",
                                input {
                                    class: "form-input",
                                    r#type: "datetime-local",
                                    value: "{deadline_str}",
                                    oninput: move |e| deadline_str.set(e.value()),
                                }
                            }

                            div { class: "setting-group",
                                label { class: "switch-label",
                                    input {
                                        class: "switch-input",
                                        r#type: "checkbox",
                                        checked: *hide_results.read(),
                                        onchange: move |e| hide_results.set(e.checked()),
                                    }
                                    span { class: "switch-slider" }
                                    " Hide results until deadline"
                                }
                            }
                        }
                    }
                }
            }

            button {
                class: "submit-btn",
                onclick: submit,
                disabled: *submitting.read(),
                if submitting() {
                    "Creating..."
                } else {
                    "Create Poll"
                }
            }
        }
    }
}
