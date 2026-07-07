use dioxus::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    fn next(&self) -> Self {
        match self {
            Theme::System => Theme::Light,
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::System,
        }
    }
}

fn read_stored_theme() -> Theme {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(storage) = web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten()
        {
            if let Ok(Some(val)) = storage.get_item("theme") {
                return match val.as_str() {
                    "light" => Theme::Light,
                    "dark" => Theme::Dark,
                    _ => Theme::System,
                };
            }
        }
    }
    Theme::System
}

fn apply_theme_to_dom(theme: &Theme) {
    #[cfg(target_arch = "wasm32")]
    apply_theme_to_dom_impl(theme);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = theme;
}

#[cfg(target_arch = "wasm32")]
fn apply_theme_to_dom_impl(theme: &Theme) {
    let val = match theme {
        Theme::System => "",
        Theme::Light => "light",
        Theme::Dark => "dark",
    };
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.set_item("theme", val);
        }
        if let Some(document) = window.document() {
            if let Some(el) = document.document_element() {
                if val.is_empty() {
                    let _ = el.remove_attribute("data-theme");
                } else {
                    let _ = el.set_attribute("data-theme", val);
                }
            }
        }
    }
}

pub fn use_theme() -> Signal<Theme> {
    let initial = read_stored_theme();
    let theme = use_signal(|| initial.clone());

    apply_theme_to_dom(&initial);

    theme
}

pub fn cycle_theme(theme: &mut Theme) {
    *theme = theme.next();
    apply_theme_to_dom(theme);
}

pub fn theme_icon(theme: Theme) -> Element {
    match theme {
        Theme::System => rsx! {
            svg {
                view_box: "0 0 24 24",
                width: "18",
                height: "18",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                rect { x: "2", y: "3", width: "20", height: "14", rx: "2" }
                path { d: "M8 21h8" }
                path { d: "M12 17v4" }
            }
        },
        Theme::Light => rsx! {
            svg {
                view_box: "0 0 24 24",
                width: "18",
                height: "18",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                circle { cx: "12", cy: "12", r: "5" }
                path { d: "M12 1v2" }
                path { d: "M12 21v2" }
                path { d: "M4.22 4.22l1.42 1.42" }
                path { d: "M18.36 18.36l1.42 1.42" }
                path { d: "M1 12h2" }
                path { d: "M21 12h2" }
                path { d: "M4.22 19.78l1.42-1.42" }
                path { d: "M18.36 5.64l1.42-1.42" }
            }
        },
        Theme::Dark => rsx! {
            svg {
                view_box: "0 0 24 24",
                width: "18",
                height: "18",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
            }
        },
    }
}
