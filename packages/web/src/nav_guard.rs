//! Confirms before router-initiated navigation (a `Link` click, programmatic
//! nav) leaves a page with unsaved input - the gap `unsaved_guard`'s
//! `beforeunload` can't cover, since the router never unloads the document
//! for these. The browser's back/forward buttons are a third path neither of
//! those sees - a same-document `popstate` for which the router never calls
//! `on_update` - and are guarded by `unsaved_guard`'s popstate listener.
//!
//! Wired in as the router's `on_update` callback, which - per
//! `dioxus_router` - runs *after* the history entry has already changed but
//! *before* components/hooks re-render for it. That ordering is what makes
//! blocking possible at all: returning a route from here makes the router
//! silently replace the history entry with it (a second, un-observed change)
//! before anything re-renders, so a declined navigation never becomes
//! visible - not even as a flash of the page being left.
use dioxus::prelude::*;
use dioxus::router::{GenericRouterContext, RouterConfig};

use crate::Route;
#[cfg(feature = "web")]
use crate::unsaved_guard::CONFIRM_MESSAGE;
use crate::unsaved_guard::UNSAVED_CHANGES;

/// The most recently rendered route - i.e. the one to revert to if a
/// navigation away from it is declined. Kept in sync by `use_track_route`
/// rather than from inside `on_update` itself: `on_update` fires only on
/// navigation, so it never sees the very first route a page loaded on
/// directly, which would otherwise leave this `None` right up until the
/// first *away* navigation - exactly the one that most needs to be caught.
static LAST_ROUTE: GlobalSignal<Option<Route>> = Signal::global(|| None);

/// Keeps `LAST_ROUTE` current, including the very first route the app
/// loaded on. Call once from a component every route renders through, e.g.
/// the layout - `use_route`'s subscription then re-runs this on every
/// navigation the same way it re-renders that component.
pub fn use_track_route() {
    let route = use_route::<Route>();
    *LAST_ROUTE.write() = Some(route);
}

fn leaves_unsaved_page(from: &Route) -> bool {
    matches!(from, Route::Create {} | Route::Vote { .. })
}

#[cfg(feature = "web")]
fn confirm_leave() -> bool {
    web_sys::window()
        .and_then(|w| w.confirm_with_message(CONFIRM_MESSAGE).ok())
        .unwrap_or(true)
}

// Non-web targets (SSR) never navigate interactively, so nothing is ever
// actually left mid-edit - only the `web` build's window has a user to ask.
#[cfg(not(feature = "web"))]
fn confirm_leave() -> bool {
    true
}

pub fn config(_: ()) -> RouterConfig<Route> {
    RouterConfig::default().on_update(|ctx: GenericRouterContext<Route>| {
        let new_route = ctx.current();
        let prev_route = LAST_ROUTE.read().clone();

        let should_confirm = prev_route.as_ref().is_some_and(|prev| {
            *prev != new_route && leaves_unsaved_page(prev) && UNSAVED_CHANGES()
        });

        if should_confirm && !confirm_leave() {
            return prev_route.map(Into::into);
        }

        None
    })
}
