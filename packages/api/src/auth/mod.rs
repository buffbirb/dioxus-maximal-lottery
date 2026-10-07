//! Sign-in through external identity providers.
//!
//! - The OAuth flow is plain axum routes (see `routes`), because it needs
//!   redirects and several cookies per response.
//! - Everything else - the current user, vote attribution - reads the
//!   session cookie from inside server functions.
use dioxus::prelude::*;

use crate::model::UserView;

pub mod return_to;

#[cfg(feature = "server")]
pub mod github;
#[cfg(feature = "server")]
pub mod provider;
#[cfg(feature = "server")]
pub mod routes;
#[cfg(feature = "server")]
pub mod session;
#[cfg(feature = "server")]
pub mod state;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderInfo {
    /// The `/login/{id}` path segment and the stored identity's provider.
    pub id: &'static str,
    pub label: &'static str,
}

/// Every sign-in provider. The login page renders one button per entry and
/// the server routes exactly these ids.
pub const PROVIDERS: &[ProviderInfo] = &[ProviderInfo {
    id: "github",
    label: "GitHub",
}];

/// Loads provider credentials. Panics naming the variable when one is unset
/// or empty, so a running server always has every provider configured.
#[cfg(feature = "server")]
pub fn init() {
    github::init();
}

#[get("/api/me")]
#[cfg_attr(feature = "server", tracing::instrument)]
pub async fn current_user() -> Result<Option<UserView>, ServerFnError> {
    let Some(hash) = session::hash_from_context() else {
        return Ok(None);
    };
    let user = crate::db::fetch_session_user(&hash)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(user.map(|user| UserView {
        display_name: user.display_name,
        avatar_url: user.avatar_url,
    }))
}
