//! Session-cookie guard against repeat votes: a per-poll `voted_<share_id>`
//! cookie is set on the response when a vote is recorded, and its presence on
//! the request blocks a second one.
//!
//! The cookie is a session cookie - no `Expires`/`Max-Age` - so it lives only
//! as long as the browser session, per the feature's scope. Clearing cookies
//! bypasses the guard, which is the inherent limit of this lightweight
//! mechanism rather than a surprise.
//!
//! The cookie is deliberately not `HttpOnly`: the client renders "Already
//! voted" straight off `document.cookie` with no extra request. That makes the
//! flag readable (and writable) by same-origin JS, which is acceptable because
//! the cookie is not the enforcement - `submit_vote` checks it server-side and
//! a repeat vote still 400s whatever the client believes. Same-origin XSS can
//! already act as the user (same-origin fetches ride the session's cookies),
//! so a JS-visible flag does not widen that blast radius. `SameSite=Strict`
//! keeps the cookie off cross-site requests, and `Secure` is set whenever the
//! request arrived over HTTPS so local plain-HTTP development keeps working.

#[cfg(feature = "server")]
use http::{header, request::Parts};

/// The cookie value used to flag a vote, alongside the fixed attributes.
/// `share_id` is a cookie-safe 10-char nanoid, so no escaping is needed.
const PATH: &str = "Path=/";
const SAME_SITE_STRICT: &str = "SameSite=Strict";
const SECURE: &str = "Secure";

/// The value the voted cookie must hold to count as set. Anything else in the
/// cookie (or a missing cookie) reads as "not voted".
const VOTED_VALUE: &str = "1";

/// The name of the cookie flagging a poll as voted on.
pub fn cookie_name(share_id: &str) -> String {
    format!("voted_{share_id}")
}

/// Whether a single `Cookie` header value - or the whole `document.cookie`
/// string - flags `share_id` as already voted on. Pairs are `;`-separated and
/// trimmed, so unrelated cookies riding along never interfere.
pub fn parse_voted(cookie_header: &str, share_id: &str) -> bool {
    let target = cookie_name(share_id);
    cookie_header
        .split(';')
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            Some((name.trim(), value.trim()))
        })
        .any(|(name, value)| name == target && value == VOTED_VALUE)
}

/// Whether the request's `Cookie` headers flag `share_id` as already voted on.
/// Every `Cookie` header is scanned - the header can repeat.
#[cfg(feature = "server")]
pub fn has_voted(parts: &Parts, share_id: &str) -> bool {
    parts
        .headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| parse_voted(value, share_id))
}

/// The `Set-Cookie` value to flag `share_id` as voted for the rest of the
/// browser session. `secure` should reflect whether the request arrived over
/// HTTPS (see [`is_https_request`]); the attribute is omitted on plain HTTP so
/// local development keeps working.
pub fn set_voted_header(share_id: &str, secure: bool) -> String {
    let mut header = format!(
        "{}={VOTED_VALUE}; {PATH}; {SAME_SITE_STRICT}",
        cookie_name(share_id)
    );
    if secure {
        header.push_str("; ");
        header.push_str(SECURE);
    }
    header
}

/// Whether the request was made over HTTPS. The first `X-Forwarded-Proto`
/// token is authoritative (Render terminates TLS in front of this process);
/// absent or unrecognised, the URI's own scheme is the fallback.
#[cfg(feature = "server")]
pub fn is_https_request(parts: &Parts) -> bool {
    let forwarded = parts
        .headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim);
    match forwarded {
        Some("https") => true,
        Some("http") => false,
        _ => parts.uri.scheme_str() == Some("https"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "server")]
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
    fn no_cookie_headers_means_no_vote() {
        assert!(!parse_voted("", "abc123"));
    }

    #[test]
    fn the_matching_cookie_marks_a_vote() {
        assert!(parse_voted(
            &format!("voted_abc123={VOTED_VALUE}"),
            "abc123"
        ));
    }

    #[test]
    fn the_vote_cookie_among_other_cookies_is_found() {
        assert!(parse_voted(
            &format!("session=xyz; voted_abc123={VOTED_VALUE}; theme=dark"),
            "abc123"
        ));
    }

    #[test]
    fn the_vote_cookie_is_per_poll() {
        assert!(!parse_voted(
            &format!("voted_abc123={VOTED_VALUE}"),
            "def456"
        ));
    }

    #[test]
    fn only_the_value_one_counts() {
        for value in ["0", "10", "", "1x"] {
            let header = format!("voted_abc123={value}");
            assert!(!parse_voted(&header, "abc123"), "for Cookie: {header:?}");
        }
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert!(parse_voted(
            &format!(" voted_abc123={VOTED_VALUE} ; other=x"),
            "abc123"
        ));
    }

    #[test]
    fn a_longer_name_prefix_does_not_match() {
        assert!(!parse_voted(
            &format!("voted_abc1234={VOTED_VALUE}"),
            "abc123"
        ));
    }

    #[test]
    fn malformed_cookie_headers_are_skipped() {
        for value in ["garbage", "=1", ";", "voted_abc123", "=2"] {
            assert!(!parse_voted(value, "abc123"), "for Cookie: {value:?}");
        }
    }

    #[test]
    fn set_voted_header_builds_a_session_cookie() {
        assert_eq!(
            set_voted_header("abc123", false),
            "voted_abc123=1; Path=/; SameSite=Strict"
        );
    }

    #[test]
    fn set_voted_header_adds_secure_when_requested() {
        assert_eq!(
            set_voted_header("abc123", true),
            "voted_abc123=1; Path=/; SameSite=Strict; Secure"
        );
    }

    #[cfg(feature = "server")]
    #[test]
    fn every_cookie_header_is_scanned() {
        assert!(has_voted(
            &parts(
                "/",
                &[
                    ("cookie", "a=1"),
                    ("cookie", &format!("voted_abc123={VOTED_VALUE}"))
                ],
            ),
            "abc123"
        ));
    }

    #[cfg(feature = "server")]
    #[test]
    fn forwarded_proto_https_is_secure() {
        assert!(is_https_request(&parts(
            "/",
            &[("x-forwarded-proto", "https")],
        )));
    }

    #[cfg(feature = "server")]
    #[test]
    fn forwarded_proto_takes_the_first_of_a_proxy_chain() {
        assert!(is_https_request(&parts(
            "/",
            &[("x-forwarded-proto", "https, http")],
        )));
    }

    #[cfg(feature = "server")]
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
