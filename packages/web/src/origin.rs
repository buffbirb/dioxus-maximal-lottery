#![cfg(feature = "server")]

//! Derives the app's own public origin - `scheme://host[:port]` - from the
//! incoming request, so `components::share_section` can server-render a
//! finished absolute URL instead of patching one in after hydration.
//!
//! Neither half of that origin can be read off the socket. Render fronts every
//! web service with a TLS-terminating proxy, so this process only ever serves
//! plain HTTP on an internal port and has to be *told* the public scheme via
//! `X-Forwarded-Proto`. Locally there is a proxy of a different kind: `dx serve`
//! fronts the app with the CLI's dev server and forwards to this process on a
//! random internal port *without* rewriting `Host`, so the header names a port
//! nobody is browsing and the CLI has to be asked instead.

use http::{header, request::Parts, uri::Authority};
use std::sync::OnceLock;

/// Pins the origin outright, for deployments where the forwarded headers can't
/// be trusted (a proxy chain that rewrites them, say). Unset on Render.
const BASE_URL_VAR: &str = "PUBLIC_BASE_URL";

/// A 253-byte DNS name plus `:65535`. Purely a sanity bound - anything longer
/// is not a hostname anyone is serving from.
const MAX_AUTHORITY_LEN: usize = 259;

const X_FORWARDED_PROTO: &str = "x-forwarded-proto";
const X_FORWARDED_HOST: &str = "x-forwarded-host";

/// The origin to build absolute URLs from, or `None` when the request carries
/// no authority worth trusting - callers should then fall back to a relative
/// URL rather than guess.
pub fn derive_origin(parts: &Parts) -> Option<String> {
    derive_origin_with(env_override(), dev_server_authority(), parts)
}

/// Both ambient inputs are passed in rather than read here: they are fixed for
/// the life of the process, so this keeps the per-request precedence testable
/// without touching the environment the test runner is itself running in.
/// `base_url_override` is expected to be already sanitized - see
/// [`sanitize_base_url`].
fn derive_origin_with(
    base_url_override: Option<&str>,
    dev_server_authority: Option<&str>,
    parts: &Parts,
) -> Option<String> {
    if let Some(base_url) = base_url_override {
        return Some(base_url.to_string());
    }

    let scheme = forwarded_scheme(parts).unwrap_or("http");
    let authority = request_authority(dev_server_authority, parts)?;
    Some(format!("{scheme}://{authority}"))
}

/// The address `dx serve` is listening on for the browser, which is the one
/// address in local development that `Host` cannot tell us: the CLI proxies to
/// this process on a random internal port and passes the upstream `Host`
/// through unchanged. Set only by the CLI, so it is absent in every real
/// deployment - including the container Render runs, which starts the server
/// binary directly.
fn dev_server_authority() -> Option<&'static str> {
    static AUTHORITY: OnceLock<Option<String>> = OnceLock::new();
    AUTHORITY
        .get_or_init(|| dioxus::cli_config::devserver_raw_addr().map(|addr| addr.to_string()))
        .as_deref()
}

/// Read once: the environment does not change under a running server, and the
/// warning below should not repeat on every request.
fn env_override() -> Option<&'static str> {
    static BASE_URL: OnceLock<Option<String>> = OnceLock::new();
    BASE_URL
        .get_or_init(|| {
            let raw = std::env::var(BASE_URL_VAR).ok()?;
            let raw = raw.trim();
            if raw.is_empty() {
                return None;
            }
            let sanitized = sanitize_base_url(raw);
            if sanitized.is_none() {
                tracing::warn!(
                    "{BASE_URL_VAR}={raw:?} is not a bare `http(s)://host[:port]` origin; \
                     falling back to the request's forwarded and Host headers"
                );
            }
            sanitized
        })
        .as_deref()
}

/// Accepts only a bare origin, trailing slashes trimmed. A path, query or
/// credentials would each produce a broken link once a poll's path is appended,
/// so they are rejected rather than silently mangled.
fn sanitize_base_url(raw: &str) -> Option<String> {
    let (scheme, authority) = raw.split_once("://")?;
    let authority = authority.trim_end_matches('/');
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    if !is_valid_authority(authority) {
        return None;
    }
    Some(format!("{}://{}", scheme.to_ascii_lowercase(), authority))
}

/// The public scheme. Only `X-Forwarded-Proto` knows it: behind Render's proxy
/// the connection this process serves is plain HTTP even though the site is
/// HTTPS. Absent (local `dx serve`) and unrecognised (a spoofed value) both
/// mean `http`, which is what an unproxied dev server really is speaking.
fn forwarded_scheme(parts: &Parts) -> Option<&'static str> {
    let value = first_token(parts, X_FORWARDED_PROTO)?;
    ["https", "http"]
        .into_iter()
        .find(|scheme| value.eq_ignore_ascii_case(scheme))
}

/// The public authority, preferring what the outermost proxy was asked for over
/// what it forwarded us, then the dev server's own address over the `Host` it
/// proxied through, and finally the URI's authority for HTTP/2, where
/// `:authority` stands in for the `Host` header. The first *valid* candidate
/// wins, so one mangled header degrades to the next source instead of taking
/// the whole URL down with it.
fn request_authority<'a>(
    dev_server_authority: Option<&'a str>,
    parts: &'a Parts,
) -> Option<&'a str> {
    [
        first_token(parts, X_FORWARDED_HOST),
        dev_server_authority,
        first_token(parts, header::HOST),
        parts.uri.authority().map(Authority::as_str),
    ]
    .into_iter()
    .flatten()
    .find(|authority| is_valid_authority(authority))
}

/// The first entry of a possibly comma-separated forwarded header. Every proxy
/// in a chain appends its own value, so the first is the hop nearest the
/// client - the public-facing one.
fn first_token(parts: &Parts, name: impl header::AsHeaderName) -> Option<&str> {
    let value = parts.headers.get(name)?.to_str().ok()?;
    Some(value.split(',').next().unwrap_or_default().trim())
}

/// A conservative RFC 3986 host-and-port charset check. `HeaderValue::to_str`
/// has already ruled out control characters and rsx escapes the result, so this
/// is not an injection concern. It is about the share box: a host we cannot
/// vouch for should collapse to a relative URL, which reads as unfinished,
/// rather than an absolute one that reads as finished and is wrong.
fn is_valid_authority(authority: &str) -> bool {
    !authority.is_empty()
        && authority.len() <= MAX_AUTHORITY_LEN
        && authority.bytes().any(|byte| byte.is_ascii_alphanumeric())
        && authority.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b':' | b'[' | b']' | b'_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn origin(headers: &[(&str, &str)]) -> Option<String> {
        derive_origin_with(None, None, &parts("/abc123", headers))
    }

    #[test]
    fn no_authority_anywhere_yields_no_origin() {
        assert_eq!(origin(&[]), None);
    }

    #[test]
    fn unproxied_request_uses_the_host_header_over_http() {
        assert_eq!(
            origin(&[("host", "example.com")]),
            Some("http://example.com".to_string())
        );
    }

    #[test]
    fn port_is_kept() {
        assert_eq!(
            origin(&[("host", "127.0.0.1:8080")]),
            Some("http://127.0.0.1:8080".to_string())
        );
    }

    #[test]
    fn ipv6_literal_is_accepted() {
        assert_eq!(
            origin(&[("host", "[::1]:8080")]),
            Some("http://[::1]:8080".to_string())
        );
    }

    #[test]
    fn forwarded_proto_sets_the_scheme() {
        assert_eq!(
            origin(&[("host", "example.com"), ("x-forwarded-proto", "https")]),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn forwarded_proto_takes_the_first_of_a_proxy_chain() {
        for value in ["https, http", " https ,http", "HTTPS,http"] {
            assert_eq!(
                origin(&[("host", "example.com"), ("x-forwarded-proto", value)]),
                Some("https://example.com".to_string()),
                "for X-Forwarded-Proto: {value:?}"
            );
        }
    }

    #[test]
    fn unrecognised_forwarded_proto_falls_back_to_http() {
        for value in ["ftp", "\"><script>", ""] {
            assert_eq!(
                origin(&[("host", "example.com"), ("x-forwarded-proto", value)]),
                Some("http://example.com".to_string()),
                "for X-Forwarded-Proto: {value:?}"
            );
        }
    }

    #[test]
    fn forwarded_host_beats_the_host_header() {
        assert_eq!(
            origin(&[
                ("host", "internal.svc:8080"),
                ("x-forwarded-host", "polls.example.com"),
            ]),
            Some("http://polls.example.com".to_string())
        );
    }

    #[test]
    fn forwarded_host_takes_the_first_of_a_proxy_chain() {
        assert_eq!(
            origin(&[("x-forwarded-host", "polls.example.com, internal.svc")]),
            Some("http://polls.example.com".to_string())
        );
    }

    #[test]
    fn a_mangled_forwarded_host_degrades_to_the_host_header() {
        assert_eq!(
            origin(&[
                ("host", "example.com"),
                ("x-forwarded-host", "evil.example.com/path"),
            ]),
            Some("http://example.com".to_string())
        );
    }

    #[test]
    fn mangled_authorities_yield_no_origin() {
        for value in ["exa mple.com", "example.com/abc", "user@example.com", "::"] {
            assert_eq!(origin(&[("host", value)]), None, "for Host: {value:?}");
        }
    }

    #[test]
    fn http2_falls_back_to_the_uri_authority() {
        assert_eq!(
            derive_origin_with(None, None, &parts("http://h2.example.com/abc123", &[])),
            Some("http://h2.example.com".to_string())
        );
    }

    /// `dx serve` proxies to a random internal port and leaves `Host` pointing
    /// at it, so the dev server's own address is the more truthful of the two.
    #[test]
    fn the_dev_server_address_beats_the_host_it_proxied_through() {
        assert_eq!(
            derive_origin_with(
                None,
                Some("127.0.0.1:8080"),
                &parts("/abc123", &[("host", "127.0.0.1:49165")]),
            ),
            Some("http://127.0.0.1:8080".to_string())
        );
    }

    /// A real proxy in front still knows better than the dev server does, which
    /// keeps forwarded headers testable through `dx serve`.
    #[test]
    fn a_forwarded_host_beats_the_dev_server_address() {
        assert_eq!(
            derive_origin_with(
                None,
                Some("127.0.0.1:8080"),
                &parts("/abc123", &[("x-forwarded-host", "polls.example.com")]),
            ),
            Some("http://polls.example.com".to_string())
        );
    }

    #[test]
    fn an_override_wins_over_every_header() {
        assert_eq!(
            derive_origin_with(
                Some("https://polls.example.com"),
                Some("127.0.0.1:8080"),
                &parts(
                    "/abc123",
                    &[("host", "example.com"), ("x-forwarded-proto", "http")]
                ),
            ),
            Some("https://polls.example.com".to_string())
        );
    }

    #[test]
    fn sanitize_accepts_a_bare_origin() {
        assert_eq!(
            sanitize_base_url("https://polls.example.com"),
            Some("https://polls.example.com".to_string())
        );
    }

    #[test]
    fn sanitize_trims_trailing_slashes_and_lowercases_the_scheme() {
        assert_eq!(
            sanitize_base_url("HTTPS://polls.example.com//"),
            Some("https://polls.example.com".to_string())
        );
    }

    #[test]
    fn sanitize_rejects_anything_that_is_not_a_bare_origin() {
        for value in [
            "polls.example.com",
            "ftp://polls.example.com",
            "https://polls.example.com/polls",
            "https://polls.example.com?a=b",
            "https://",
            "",
        ] {
            assert_eq!(sanitize_base_url(value), None, "for {value:?}");
        }
    }
}
