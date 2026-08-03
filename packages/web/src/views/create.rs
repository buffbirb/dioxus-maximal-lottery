use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use time::Date;

use api::domain::{
    DEFAULT_DEADLINE_DAYS, Description, MAX_DESCRIPTION_LEN, MAX_OPTION_LABEL_LEN, MAX_OPTIONS,
    MAX_TITLE_LEN, MAX_VOTE_CAP, MIN_OPTIONS, MIN_VOTE_CAP, OptionLabel, Options, Title, VoteCap,
};
use api::model::CreatePollRequest;

use crate::Route;
use crate::components::DatePicker;
use crate::nav_cache::PENDING_POLL;
use crate::unsaved_guard::{mark_clean, use_unsaved_changes_guard};

async fn parse_local_datetime(value: &str) -> Option<DateTime<Utc>> {
    let script = format!(
        r#"const d = new Date("{}"); return isNaN(d.getTime()) ? null : d.getTime();"#,
        value.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let millis: Option<i64> = document::eval(&script).join().await.ok()?;
    millis.and_then(DateTime::from_timestamp_millis)
}

/// The browser-local datetime string (`YYYY-MM-DDTHH:mm`, as expected by an
/// `<input type="datetime-local">`) for "now plus `days` days".
async fn default_deadline_input_value(days: i64) -> Option<String> {
    let script = format!(
        "const d = new Date(Date.now() + {days} * 24 * 60 * 60 * 1000);\
         const pad = (n) => String(n).padStart(2, '0');\
         return `${{d.getFullYear()}}-${{pad(d.getMonth() + 1)}}-${{pad(d.getDate())}}T${{pad(d.getHours())}}:${{pad(d.getMinutes())}}`;"
    );
    document::eval(&script).join().await.ok()
}

/// Parses a plain `YYYY-MM-DD` string (as produced by
/// `default_deadline_input_value`) into a `time::Date`.
fn parse_ymd(s: &str) -> Option<Date> {
    let mut parts = s.splitn(3, '-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    Date::from_calendar_date(year, time::Month::try_from(month).ok()?, day).ok()
}

/// Focuses the option input at `index`, if one exists.
async fn focus_option_input(index: usize) {
    let script = format!(
        "const inputs = document.querySelectorAll('.option-input');
         if (inputs[{index}]) {{ inputs[{index}].focus(); }}"
    );
    let _ = document::eval(&script).join::<()>().await;
}

#[component]
pub fn Create() -> Element {
    let mut title = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut options = use_signal(|| vec![String::new(); MIN_OPTIONS]);
    let mut show_additional = use_signal(|| false);
    let mut deadline_date = use_signal(|| None::<Date>);
    let mut deadline_time_input = use_signal(String::new);
    // Flipped once (and never back) by the effect below. Distinct from
    // `deadline_date().is_some()`, which goes false again whenever the user
    // clears the picker - that must not bring the placeholder shimmer back.
    let mut deadline_ready = use_signal(|| false);
    let mut vote_cap_enabled = use_signal(|| false);
    let mut vote_cap_input = use_signal(String::new);
    let mut hide_results = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut submitting = use_signal(|| false);
    let mut focus_new_option = use_signal(|| false);

    let navigator = use_navigator();

    use_effect(move || {
        spawn(async move {
            if deadline_date().is_none()
                && let Some(default) = default_deadline_input_value(DEFAULT_DEADLINE_DAYS).await
                && let Some((date_part, time_part)) = default.split_once('T')
                && let Some(date) = parse_ymd(date_part)
            {
                deadline_date.set(Some(date));
                deadline_time_input.set(time_part.to_string());
            }
            // Reveal the controls whether or not the default resolved - if the
            // eval failed there is nothing left to wait for, and an empty
            // picker beats shimmering forever.
            deadline_ready.set(true);
        });
    });

    use_effect(move || {
        if focus_new_option() {
            focus_new_option.set(false);
            spawn(async move {
                let last = options().len().saturating_sub(1);
                focus_option_input(last).await;
            });
        }
    });

    use_unsaved_changes_guard(move || {
        !title().trim().is_empty()
            || !description().trim().is_empty()
            || options().iter().any(|o| !o.trim().is_empty())
    });

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
        if option_values.len() > MAX_OPTIONS {
            error.set(Some(format!(
                "a poll can have at most {MAX_OPTIONS} options"
            )));
            return;
        }
        if deadline_date().is_none() || deadline_time_input().is_empty() {
            error.set(Some("a deadline is required".to_string()));
            return;
        }

        let vote_cap_value = if vote_cap_enabled() {
            match vote_cap_input().trim().parse::<i32>() {
                Ok(n) if (MIN_VOTE_CAP..=MAX_VOTE_CAP).contains(&n) => Some(n),
                _ => {
                    error.set(Some(format!(
                        "vote cap must be between {MIN_VOTE_CAP} and {MAX_VOTE_CAP}"
                    )));
                    return;
                }
            }
        } else {
            None
        };

        error.set(None);
        submitting.set(true);

        spawn(async move {
            // Checked non-None/non-empty above; combine into the
            // `YYYY-MM-DDTHH:mm` shape parse_local_datetime expects.
            let date = deadline_date().expect("deadline date checked above");
            let deadline_str = format!(
                "{:04}-{:02}-{:02}T{}",
                date.year(),
                u8::from(date.month()),
                date.day(),
                deadline_time_input()
            );

            let deadline = match parse_local_datetime(&deadline_str).await {
                Some(dt) => dt,
                None => {
                    error.set(Some("please enter a valid deadline".to_string()));
                    submitting.set(false);
                    return;
                }
            };

            let title = match Title::try_new(title_value) {
                Ok(title) => title,
                Err(err) => {
                    error.set(Some(err.to_string()));
                    submitting.set(false);
                    return;
                }
            };

            let description = if description_value.is_empty() {
                None
            } else {
                match Description::try_new(description_value) {
                    Ok(description) => Some(description),
                    Err(err) => {
                        error.set(Some(err.to_string()));
                        submitting.set(false);
                        return;
                    }
                }
            };

            let option_labels = match option_values
                .into_iter()
                .map(OptionLabel::try_new)
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(labels) => labels,
                Err(err) => {
                    error.set(Some(err.to_string()));
                    submitting.set(false);
                    return;
                }
            };

            let options = match Options::try_new(option_labels) {
                Ok(options) => options,
                Err(err) => {
                    error.set(Some(err.to_string()));
                    submitting.set(false);
                    return;
                }
            };

            let vote_cap = match vote_cap_value.map(VoteCap::try_new).transpose() {
                Ok(vote_cap) => vote_cap,
                Err(err) => {
                    error.set(Some(err.to_string()));
                    submitting.set(false);
                    return;
                }
            };

            let request = CreatePollRequest {
                title,
                description,
                options,
                deadline,
                vote_cap,
                hide_results: hide_results(),
            };

            match api::polls::create_poll(request).await {
                Ok(poll_view) => {
                    let share_id = poll_view.share_id.clone();
                    *PENDING_POLL.write() = Some(poll_view);
                    mark_clean();
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
                            class: "option-input",
                            maxlength: "{MAX_OPTION_LABEL_LEN}",
                            placeholder: "Option {idx + 1}",
                            value: "{option}",
                            // Without an explicit hint, mobile keyboards (Chrome/Android
                            // in particular) guess "next" on their own and handle the
                            // return key by jumping focus themselves - silently, before
                            // it ever reaches `onkeydown` - which is what made Enter act
                            // like Tab instead of running the logic below. Setting it
                            // ourselves makes the return key dispatch a normal,
                            // interceptable Enter keydown instead.
                            "enterkeyhint": if idx + 1 < options().len() || options().len() < MAX_OPTIONS { "next" } else { "done" },
                            oninput: move |evt| options.with_mut(|o| o[idx] = evt.value()),
                            onkeydown: move |evt| {
                                if evt.key() == Key::Enter {
                                    evt.prevent_default();
                                    if idx + 1 < options().len() {
                                        spawn(focus_option_input(idx + 1));
                                    } else if options().len() < MAX_OPTIONS {
                                        options.with_mut(|o| o.push(String::new()));
                                        focus_new_option.set(true);
                                    }
                                }
                            },
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
                if options().len() < MAX_OPTIONS {
                    button {
                        class: "add-option-btn",
                        r#type: "button",
                        onclick: move |_| options.with_mut(|o| o.push(String::new())),
                        "+ Add option"
                    }
                }
            }

            div { class: "field",
                label { "Poll closes" }
                // The default deadline is the browser's local "now + N days",
                // which only the client knows - the effect above fills it in
                // after hydration. Until then both controls are empty and would
                // show their `YYYY-MM-DD` / `--:-- --` placeholders, so shimmer
                // over them instead. The wrappers are sized by the real
                // controls they contain, so nothing shifts when the value lands.
                div {
                    class: "deadline-fields",
                    "data-pending": if deadline_ready() { "false" } else { "true" },
                    div { class: "deadline-field",
                        DatePicker {
                            selected_date: deadline_date,
                            on_value_change: move |d| deadline_date.set(d),
                        }
                    }
                    div { class: "deadline-field",
                        input {
                            id: "deadline-time",
                            r#type: "time",
                            value: "{deadline_time_input}",
                            oninput: move |evt| deadline_time_input.set(evt.value()),
                        }
                    }
                }
                p { class: "flavor-text", "Voting stops automatically at this time." }
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
                            checked: vote_cap_enabled(),
                            onchange: move |evt| vote_cap_enabled.set(evt.checked()),
                        }
                        span { class: "switch-slider" }
                    }
                    div {
                        span { class: "toggle-label", "Close after N votes" }
                        p { class: "flavor-text",
                            "Also close voting once this many votes have been cast, whichever comes first."
                        }
                    }
                }

                if vote_cap_enabled() {
                    div { class: "field",
                        label { r#for: "vote-cap", "Vote cap" }
                        input {
                            id: "vote-cap",
                            r#type: "number",
                            min: "{MIN_VOTE_CAP}",
                            max: "{MAX_VOTE_CAP}",
                            value: "{vote_cap_input}",
                            oninput: move |evt| vote_cap_input.set(evt.value()),
                        }
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
                        span { class: "toggle-label", "Hide results until close" }
                        p { class: "flavor-text", "Voters won't see standings until the poll closes." }
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
