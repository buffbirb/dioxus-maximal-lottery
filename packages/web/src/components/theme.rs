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
            match theme() {
                Theme::System => rsx! {
                    svg {
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        rect {
                            x: "2",
                            y: "4",
                            width: "20",
                            height: "14",
                            rx: "2",
                        }
                        line {
                            x1: "8",
                            y1: "21",
                            x2: "16",
                            y2: "21",
                        }
                        line {
                            x1: "12",
                            y1: "18",
                            x2: "12",
                            y2: "21",
                        }
                    }
                },
                Theme::Light => rsx! {
                    svg {
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        circle { cx: "12", cy: "12", r: "4" }
                        line {
                            x1: "12",
                            y1: "2",
                            x2: "12",
                            y2: "5",
                        }
                        line {
                            x1: "12",
                            y1: "19",
                            x2: "12",
                            y2: "22",
                        }
                        line {
                            x1: "4.2",
                            y1: "4.2",
                            x2: "6.3",
                            y2: "6.3",
                        }
                        line {
                            x1: "17.7",
                            y1: "17.7",
                            x2: "19.8",
                            y2: "19.8",
                        }
                        line {
                            x1: "2",
                            y1: "12",
                            x2: "5",
                            y2: "12",
                        }
                        line {
                            x1: "19",
                            y1: "12",
                            x2: "22",
                            y2: "12",
                        }
                        line {
                            x1: "4.2",
                            y1: "19.8",
                            x2: "6.3",
                            y2: "17.7",
                        }
                        line {
                            x1: "17.7",
                            y1: "6.3",
                            x2: "19.8",
                            y2: "4.2",
                        }
                    }
                },
                Theme::Dark => rsx! {
                    svg {
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8z" }
                    }
                },
            }
        }
    }
}
