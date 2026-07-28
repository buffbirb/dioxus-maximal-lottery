//! Client-side cache used to seed the vote page with the `PollView` already
//! returned by `create_poll`, so navigating there right after creating a
//! poll doesn't re-fetch what the server just gave us.
use dioxus::prelude::*;

use api::model::PollView;

pub static PENDING_POLL: GlobalSignal<Option<PollView>> = Signal::global(|| None);
