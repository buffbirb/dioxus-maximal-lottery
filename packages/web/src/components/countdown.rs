use chrono::{DateTime, Utc};
use dioxus::prelude::*;

#[component]
pub fn Countdown(
    deadline: DateTime<Utc>,
    on_deadline: EventHandler<()>,
    live: bool,
    children: Element,
) -> Element {
    let mut now = use_signal(Utc::now);
    let mut started = use_signal(|| false);
    let mut fired = use_signal(|| false);

    use_effect(move || {
        if !started() {
            started.set(true);
            gloo_timers::callback::Interval::new(1000, move || {
                now.set(Utc::now());
            })
            .forget();
        }
    });

    let remaining = deadline - now();
    let expired = remaining.num_seconds() <= 0;
    let deadline_text = format!("{}", deadline.format("%Y-%m-%d %H:%M UTC"));
    let duration_text = format_duration(remaining);

    use_effect(move || {
        if expired && !fired() {
            fired.set(true);
            on_deadline.call(());
        }
    });

    rsx! {
        div { class: "status-bar",
            if live && !expired {
                span { class: "live-indicator", "Live" }
                span { class: "countdown", " - {duration_text} remaining" }
            } else if !expired {
                span { class: "countdown", "{duration_text} until deadline" }
            } else {
                span { class: "deadline-closed", "Closed at {deadline_text}" }
            }
            {children}
        }
    }
}

fn format_duration(dur: chrono::Duration) -> String {
    let total_secs = dur.num_seconds().max(0) as u64;
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if days > 0 {
        format!("{days}d {hours:02}h {minutes:02}m {seconds:02}s")
    } else {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    }
}
