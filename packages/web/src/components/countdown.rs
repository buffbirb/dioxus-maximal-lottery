//! Ticking countdown to a deadline, with a live/closed indicator. Fires
//! `on_elapsed` exactly once, the moment the deadline passes.
use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

const COUNTDOWN_TICK_MS: u32 = 1000;

fn format_remaining(remaining: chrono::Duration) -> String {
    let total_secs = remaining.num_seconds().max(0);
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

#[component]
pub fn Countdown(deadline: DateTime<Utc>, on_elapsed: EventHandler<()>) -> Element {
    let mut now = use_signal(Utc::now);
    let mut fired = use_signal(|| false);

    use_effect(move || {
        spawn(async move {
            loop {
                TimeoutFuture::new(COUNTDOWN_TICK_MS).await;
                now.set(Utc::now());
            }
        });
    });

    use_effect(move || {
        if deadline <= now() && !fired() {
            fired.set(true);
            on_elapsed.call(());
        }
    });

    let remaining = deadline - now();
    let elapsed = remaining.num_seconds() <= 0;

    rsx! {
        span { class: if elapsed { "countdown countdown-closed" } else { "countdown countdown-live" },
            if elapsed {
                "closed"
            } else {
                "{format_remaining(remaining)}"
            }
        }
    }
}
