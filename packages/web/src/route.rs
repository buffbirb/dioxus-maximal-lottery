// Rust 1.100's unreachable_code lint fires in the FromStr impl Routable derives
// for String segments, which an attribute on the enum can't reach. Remove this
// once a dioxus release includes DioxusLabs/dioxus#6077.
#![allow(unreachable_code)]

use dioxus::prelude::*;

use crate::components::Navbar;
use crate::views::{Create, Home, Login, NotFound, Results, Vote};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub(crate) enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    #[route("/create")]
    Create {},
    #[route("/p/:share_id/results")]
    Results { share_id: String },
    #[route("/p/:share_id")]
    Vote { share_id: String },
    // Only the page: the OAuth steps under /login/ are server routes.
    #[route("/login?:return_to")]
    Login { return_to: Option<String> },
    // Last on purpose: the catch-all only gets what the routes above declined.
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}
