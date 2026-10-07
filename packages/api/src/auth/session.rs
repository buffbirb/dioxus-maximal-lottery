//! The signed-in session: an opaque random token in an HttpOnly cookie, with
//! only its SHA-256 stored server-side. Expiry is enforced by the lookup;
//! the sweeper only bounds table growth.
use std::time::Duration;

use http::request::Parts;

use crate::{cookies, db};

pub const NAME: &str = "session";
pub const MAX_AGE_SECS: i64 = 30 * 24 * 60 * 60;

const SWEEP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

pub fn token_from_request(parts: &Parts) -> Option<String> {
    cookies::value(parts, NAME).filter(|token| !token.is_empty())
}

pub fn set_header(token: &str, secure: bool) -> String {
    header(token, MAX_AGE_SECS, secure)
}

pub fn clear_header(secure: bool) -> String {
    header("", 0, secure)
}

fn header(value: &str, max_age: i64, secure: bool) -> String {
    let mut header = format!("{NAME}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}");
    if secure {
        header.push_str("; Secure");
    }
    header
}

/// The session cookie's hash for the current server-function request.
/// Sync so the request lock is released before any await.
pub fn hash_from_context() -> Option<Vec<u8>> {
    let ctx = dioxus::fullstack::FullstackContext::current()?;
    let token = token_from_request(&ctx.parts_mut())?;
    Some(cookies::hash_token(&token))
}

/// The signed-in user, if any. Requests without the cookie never query.
pub async fn current_user_id() -> Result<Option<i64>, sqlx::Error> {
    let Some(hash) = hash_from_context() else {
        return Ok(None);
    };
    Ok(db::fetch_session_user(&hash).await?.map(|user| user.id))
}

/// Deletes expired sessions now and then daily. Every instance runs one;
/// the delete is idempotent, and a failed sweep just waits for the next.
pub fn spawn_sweeper() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            interval.tick().await;
            match db::delete_expired_sessions().await {
                Ok(deleted) => tracing::debug!(deleted, "swept expired sessions"),
                Err(e) => tracing::warn!("session sweep failed: {e}"),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn parts(cookie: Option<&str>) -> Parts {
        let mut request = http::Request::builder().uri("http://example.com/");
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        request.body(()).expect("test request").into_parts().0
    }

    #[test]
    fn set_header_has_every_attribute() {
        assert_eq!(
            set_header(TOKEN, false),
            format!("session={TOKEN}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000")
        );
    }

    #[test]
    fn clear_header_expires_the_same_path() {
        assert_eq!(
            clear_header(false),
            "session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
        );
    }

    #[test]
    fn secure_is_added_only_when_requested() {
        assert!(set_header(TOKEN, true).ends_with("; Secure"));
        assert!(clear_header(true).ends_with("; Secure"));
        assert!(!set_header(TOKEN, false).contains("Secure"));
        assert!(!clear_header(false).contains("Secure"));
    }

    #[test]
    fn the_token_is_read_among_other_cookies() {
        let cookie = format!("vote_token=abc; {NAME}={TOKEN}; theme=dark");
        assert_eq!(
            token_from_request(&parts(Some(&cookie))),
            Some(TOKEN.to_string())
        );
    }

    #[test]
    fn missing_or_empty_cookie_is_no_session() {
        assert_eq!(token_from_request(&parts(None)), None);
        assert_eq!(token_from_request(&parts(Some("vote_token=abc"))), None);
        assert_eq!(token_from_request(&parts(Some("session="))), None);
    }
}
