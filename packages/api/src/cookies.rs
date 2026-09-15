//! Per-poll voter token: a random value kept in an HttpOnly cookie scoped
//! to one poll's page path, so a browser that voted cannot vote again on
//! that poll for the life of the cookie.
//!
//! - `Path=/p/{share_id}` means the browser only attaches it to that
//!   poll's requests; RFC 6265 path-match reaches slash-delimited
//!   descendants only, so other polls never see it.
//! - Session-scoped, host-only, `SameSite=Lax`, `Secure` over HTTPS.
//! - The database stores only `SHA-256(token)`, never the token, so a
//!   database copy cannot reconstruct it or link voters across polls.
//! - Clearing cookies isolates the guard to the browser session that cast
//!   the vote. That is the accepted limit of a no-account mechanism.
use http::{header, request::Parts};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::forwarded;
use crate::share_id::ShareId;

/// Cookie name. A constant is safe because the path scopes it to one poll:
/// cookies with the same name at different paths never collide.
pub const NAME: &str = "vote_token";

pub fn new_token() -> String {
    Uuid::new_v4().simple().to_string()
}

/// SHA-256 of the token, the only form the database ever sees.
pub fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// The poll's token from the request, if one was sent. Every `Cookie` header
/// is scanned since the header can repeat.
pub fn token_from_request(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            Some((name.trim(), value.trim()))
        })
        .find(|(name, _)| *name == NAME)
        .map(|(_, value)| value.to_string())
}

/// The `Set-Cookie` value carrying a token for one poll. `Secure` is
/// omitted on plain HTTP so local development keeps working.
pub fn set_token_header(share_id: &ShareId, token: &str, secure: bool) -> String {
    let mut header = format!("{NAME}={token}; Path=/p/{share_id}; HttpOnly; SameSite=Lax");
    if secure {
        header.push_str("; Secure");
    }
    header
}

pub fn is_https_request(parts: &Parts) -> bool {
    match forwarded::scheme(parts) {
        Some(scheme) => scheme == "https",
        None => parts.uri.scheme_str() == Some("https"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";
    const URI: &str = "http://example.com/p/abc1234567/api/poll";

    fn share_id(value: &str) -> ShareId {
        ShareId::try_new(value).expect("test share id")
    }

    fn parts(uri: &str, headers: &[(&str, &str)]) -> Parts {
        let mut request = http::Request::builder().uri(uri);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        request
            .body(())
            .expect("failed to build request")
            .into_parts()
            .0
    }

    #[test]
    fn new_tokens_are_32_lowercase_hex_chars() {
        for _ in 0..16 {
            let token = new_token();
            assert_eq!(Uuid::try_parse(&token).unwrap().simple().to_string(), token);
        }
    }

    #[test]
    fn hashing_is_deterministic_and_token_specific() {
        assert_eq!(hash_token(TOKEN).len(), 32);
        assert_eq!(hash_token(TOKEN), hash_token(TOKEN));
        assert_ne!(
            hash_token(TOKEN),
            hash_token("00000000000000000000000000000000")
        );
    }

    #[test]
    fn no_cookie_headers_means_no_token() {
        assert_eq!(token_from_request(&parts(URI, &[])), None);
    }

    #[test]
    fn the_matching_cookie_is_found_among_others() {
        let header = format!("session=xyz; {NAME}={TOKEN}; theme=dark");
        assert_eq!(
            token_from_request(&parts(URI, &[("cookie", &header)])),
            Some(TOKEN.to_string())
        );
    }

    #[test]
    fn every_cookie_header_is_scanned() {
        assert_eq!(
            token_from_request(&parts(
                URI,
                &[("cookie", "a=1"), ("cookie", &format!("{NAME}={TOKEN}")),]
            )),
            Some(TOKEN.to_string())
        );
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let header = format!(" {NAME}={TOKEN} ; other=x");
        assert_eq!(
            token_from_request(&parts(URI, &[("cookie", &header)])),
            Some(TOKEN.to_string())
        );
    }

    #[test]
    fn malformed_cookie_headers_are_skipped() {
        for value in ["garbage", "=1", ";", NAME] {
            assert_eq!(
                token_from_request(&parts(URI, &[("cookie", value)])),
                None,
                "for Cookie: {value:?}"
            );
        }
    }

    #[test]
    fn the_raw_cookie_value_is_the_token() {
        for value in ["", "short", "0123456789ABCDEF0123456789abcdef"] {
            let header = format!("{NAME}={value}");
            assert_eq!(
                token_from_request(&parts(URI, &[("cookie", &header)])),
                Some(value.to_string()),
                "for {value:?}"
            );
        }
    }

    #[test]
    fn a_longer_name_prefix_does_not_match() {
        let header = format!("{NAME}_extra={TOKEN}");
        assert_eq!(
            token_from_request(&parts(URI, &[("cookie", &header)])),
            None
        );
    }

    #[test]
    fn the_header_scopes_the_cookie_to_one_poll() {
        assert_eq!(
            set_token_header(&share_id("abc1234567"), TOKEN, false),
            format!("{NAME}={TOKEN}; Path=/p/abc1234567; HttpOnly; SameSite=Lax")
        );
    }

    #[test]
    fn secure_is_added_only_when_requested() {
        assert!(set_token_header(&share_id("abc1234567"), TOKEN, true).ends_with("; Secure"));
        assert!(!set_token_header(&share_id("abc1234567"), TOKEN, false).contains("Secure"));
    }

    #[test]
    fn forwarded_proto_https_is_secure() {
        assert!(is_https_request(&parts(
            URI,
            &[("x-forwarded-proto", "https")]
        )));
    }

    #[test]
    fn forwarded_proto_takes_the_first_of_a_proxy_chain() {
        assert!(is_https_request(&parts(
            URI,
            &[("x-forwarded-proto", "https, http")]
        )));
    }

    #[test]
    fn forwarded_proto_is_matched_case_insensitively() {
        assert!(is_https_request(&parts(
            URI,
            &[("x-forwarded-proto", "HTTPS")]
        )));
        assert!(!is_https_request(&parts(
            "https://example.com/",
            &[("x-forwarded-proto", "HTTP")],
        )));
    }

    #[test]
    fn unrecognised_forwarded_proto_falls_back_to_the_uri_scheme() {
        assert!(!is_https_request(&parts("http://example.com/", &[])));
        assert!(!is_https_request(&parts(
            "http://example.com/",
            &[("x-forwarded-proto", "ftp")],
        )));
        assert!(is_https_request(&parts("https://example.com/", &[])));
    }
}
