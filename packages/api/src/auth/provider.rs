//! Provider dispatch. An enum rather than a trait object: `async fn` in traits
//! is not dyn-compatible, the set is closed, and a new variant makes the
//! compiler flag every match that needs an arm.
use std::collections::HashMap;
use std::fmt;

use oauth2::{CsrfToken, PkceCodeChallenge, RedirectUrl, url::Url};

use super::github;

/// A provider account normalised for storage. Profile fields are optional
/// because providers differ in what they return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub provider: &'static str,
    /// The provider's stable account id, never a renameable handle.
    pub subject: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

/// A verified callback. Carries every other query parameter too, since some
/// providers deliver profile data on the callback itself.
pub struct Callback {
    pub code: String,
    pub code_verifier: String,
    pub params: HashMap<String, String>,
}

#[derive(Debug)]
pub enum AuthError {
    /// The provider refused or failed the token or profile request.
    Upstream(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::Upstream(message) => write!(f, "provider request failed: {message}"),
        }
    }
}

impl std::error::Error for AuthError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    GitHub,
}

impl Provider {
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "github" => Some(Provider::GitHub),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Provider::GitHub => "github",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Provider::GitHub => "GitHub",
        }
    }

    pub fn authorize_url(
        self,
        redirect_uri: &RedirectUrl,
        state: CsrfToken,
        code_challenge: PkceCodeChallenge,
    ) -> Url {
        match self {
            Provider::GitHub => github::authorize_url(redirect_uri, state, code_challenge),
        }
    }

    /// `redirect_uri` must equal the one sent with the authorize request.
    pub async fn exchange(
        self,
        callback: &Callback,
        redirect_uri: &RedirectUrl,
    ) -> Result<Identity, AuthError> {
        match self {
            Provider::GitHub => github::exchange(callback, redirect_uri).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::PROVIDERS;

    #[test]
    fn registry_and_dispatch_agree() {
        for info in PROVIDERS {
            let provider = Provider::from_id(info.id).expect("registered id resolves");
            assert_eq!(provider.id(), info.id);
            assert_eq!(provider.label(), info.label);
        }
    }

    #[test]
    fn unknown_ids_are_rejected() {
        for id in ["google", "apple", "GitHub", "", "github/"] {
            assert_eq!(Provider::from_id(id), None, "for {id:?}");
        }
    }
}
