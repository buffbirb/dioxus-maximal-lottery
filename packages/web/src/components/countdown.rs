//! Ticking countdown to a deadline, with a live/closed indicator. Fires
//! `on_elapsed` exactly once, the moment the deadline passes. Tapping the
//! countdown briefly reveals the exact deadline in the viewer's local time
//! zone.
use chrono::{DateTime, Months, Utc};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

const COUNTDOWN_TICK_MS: u32 = 1000;
const EXACT_REVEAL_MS: u32 = 3000;

/// Renders the top 3 consecutive non-zero units of the gap between `now` and
/// `deadline`, starting at the largest non-zero unit (years down to
/// seconds). Naturally degrades to `"{m}m {s}s"` under an hour and `"{s}s"`
/// under a minute, since there simply aren't 3 units left to show.
fn format_remaining(now: DateTime<Utc>, deadline: DateTime<Utc>) -> String {
    let mut months = 0u32;
    while let Some(next) = now.checked_add_months(Months::new(months + 1))
        && next <= deadline
    {
        months += 1;
    }
    let years = months / 12;
    let months = months % 12;

    let carried = now
        .checked_add_months(Months::new(years * 12 + months))
        .unwrap_or(now);
    let leftover = deadline - carried;
    let total_secs = leftover.num_seconds().max(0);
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    let units: [(i64, &str); 6] = [
        (years as i64, "y"),
        (months as i64, "mo"),
        (days, "d"),
        (hours, "h"),
        (minutes, "m"),
        (seconds, "s"),
    ];

    let start = units.iter().position(|(v, _)| *v > 0).unwrap_or(5);
    units[start..(start + 3).min(units.len())]
        .iter()
        .map(|(v, suffix)| format!("{v}{suffix}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[component]
pub fn Countdown(deadline: DateTime<Utc>, on_elapsed: EventHandler<()>) -> Element {
    let mut now = use_signal(Utc::now);
    let mut fired = use_signal(|| false);
    let mut revealed = use_signal(|| false);
    let mut reveal_seq = use_signal(|| 0u32);

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

    let on_reveal = move |_| {
        revealed.set(true);
        let seq = reveal_seq() + 1;
        reveal_seq.set(seq);
        spawn(async move {
            TimeoutFuture::new(EXACT_REVEAL_MS).await;
            if reveal_seq() == seq {
                revealed.set(false);
            }
        });
    };

    rsx! {
        span { class: "countdown-wrap",
            span {
                class: if elapsed { "countdown countdown-closed" } else { "countdown countdown-live" },
                onclick: on_reveal,
                if elapsed {
                    "closed"
                } else {
                    "{format_remaining(now(), deadline)}"
                }
            }
            if revealed() {
                {
                    let exact = deadline
                        .with_timezone(&chrono::Local)
                        .format("%a %-d %b %Y, %-I:%M %p")
                        .to_string();
                    rsx! {
                        span { class: "countdown-exact", "{exact}" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn dt(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    #[test]
    fn shows_years_months_days_when_over_a_year_out() {
        let now = dt(2026, 1, 1, 0, 0, 0);
        let deadline = dt(2027, 3, 6, 0, 0, 0);
        assert_eq!(format_remaining(now, deadline), "1y 2mo 5d");
    }

    #[test]
    fn shows_months_days_hours_when_months_out() {
        let now = dt(2026, 1, 1, 0, 0, 0);
        let deadline = dt(2026, 3, 6, 3, 0, 0);
        assert_eq!(format_remaining(now, deadline), "2mo 5d 3h");
    }

    #[test]
    fn shows_days_hours_minutes_when_days_out() {
        let now = dt(2026, 1, 1, 0, 0, 0);
        let deadline = dt(2026, 1, 6, 3, 20, 0);
        assert_eq!(format_remaining(now, deadline), "5d 3h 20m");
    }

    #[test]
    fn shows_hours_minutes_seconds_when_under_a_day() {
        let now = dt(2026, 1, 1, 0, 0, 0);
        let deadline = dt(2026, 1, 1, 5, 12, 30);
        assert_eq!(format_remaining(now, deadline), "5h 12m 30s");
    }

    #[test]
    fn shows_minutes_seconds_when_under_an_hour() {
        let now = dt(2026, 1, 1, 0, 0, 0);
        let deadline = dt(2026, 1, 1, 0, 20, 15);
        assert_eq!(format_remaining(now, deadline), "20m 15s");
    }

    #[test]
    fn shows_seconds_only_when_under_a_minute() {
        let now = dt(2026, 1, 1, 0, 0, 0);
        let deadline = dt(2026, 1, 1, 0, 0, 42);
        assert_eq!(format_remaining(now, deadline), "42s");
    }
}
