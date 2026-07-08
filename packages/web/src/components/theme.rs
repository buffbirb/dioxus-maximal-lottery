//! Theme toggle cycling system -> light -> dark, persisted to
//! `localStorage` and applied via `data-theme` on `<html>` so plain CSS
//! (`:root[data-theme=...]` + a `prefers-color-scheme` fallback for
//! "system") can pick it up with no JS re-render.
use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    fn as_str(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::System,
        }
    }

    fn next(self) -> Self {
        match self {
            Theme::System => Theme::Light,
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::System,
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Theme::System => "\u{1F5A5}",
            Theme::Light => "\u{2600}",
            Theme::Dark => "\u{1F319}",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Theme::System => "System theme",
            Theme::Light => "Light theme",
            Theme::Dark => "Dark theme",
        }
    }
}

fn apply_theme(theme: Theme) {
    let script = format!(
        r#"
        localStorage.setItem("theme", "{theme}");
        if ("{theme}" === "system") {{
            document.documentElement.removeAttribute("data-theme");
        }} else {{
            document.documentElement.setAttribute("data-theme", "{theme}");
        }}
        "#,
        theme = theme.as_str()
    );
    document::eval(&script);
}

#[component]
pub fn ThemeToggle() -> Element {
    let mut theme = use_signal(|| Theme::System);

    use_effect(move || {
        spawn(async move {
            let stored: Result<Option<String>, _> =
                document::eval(r#"return localStorage.getItem("theme");"#)
                    .join()
                    .await;
            if let Ok(Some(value)) = stored {
                let stored = Theme::from_str(&value);
                theme.set(stored);
                apply_theme(stored);
            }
        });
    });

    rsx! {
        button {
            class: "theme-toggle",
            r#type: "button",
            "aria-label": "{theme().label()}",
            title: "{theme().label()}",
            onclick: move |_| {
                let next = theme().next();
                theme.set(next);
                apply_theme(next);
            },
            "{theme().icon()}"
        }
    }
}
