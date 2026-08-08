//! Router-side guard that confirms before leaving a page with unsaved changes.
//! Covers `Link` clicks and programmatic navigation; `unsaved_guard` handles
//! tab close/reload and browser back/forward via JS listeners.
use dioxus::prelude::*;
use dioxus::router::{GenericRouterContext, RouterConfig};

use crate::Route;
use crate::unsaved_guard::UNSAVED_CHANGES;
#[cfg(feature = "web")]
use crate::unsaved_guard::confirm_message;

// Used to revert a declined navigation.
static LAST_ROUTE: GlobalSignal<Option<Route>> = Signal::global(|| None);

pub fn use_track_route() {
    let route = use_route::<Route>();
    *LAST_ROUTE.write() = Some(route);
}

fn leaves_unsaved_page(from: &Route) -> bool {
    matches!(from, Route::Create {} | Route::Vote { .. })
}

fn confirm_leave() -> bool {
    #[cfg(feature = "web")]
    {
        web_sys::window()
            .and_then(|w| w.confirm_with_message(confirm_message!()).ok())
            .unwrap_or(true)
    }
    // SSR has no interactive window to prompt.
    #[cfg(not(feature = "web"))]
    {
        true
    }
}

pub fn config(_: ()) -> RouterConfig<Route> {
    RouterConfig::default().on_update(|ctx: GenericRouterContext<Route>| {
        let new_route = ctx.current();
        let prev = LAST_ROUTE.read();
        let from = prev.as_ref()?;
        if *from == new_route || !leaves_unsaved_page(from) || !UNSAVED_CHANGES() || confirm_leave()
        {
            return None;
        }
        Some(from.clone().into())
    })
}
