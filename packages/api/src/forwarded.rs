//! The `X-Forwarded-*` headers a TLS-terminating proxy adds: this process only
//! speaks plain HTTP, so the client's scheme and host can only come from here.
//! Proxies append, so the first token is the hop nearest the client.
use http::{header, request::Parts};

const X_FORWARDED_PROTO: &str = "x-forwarded-proto";

pub fn first_token(parts: &Parts, name: impl header::AsHeaderName) -> Option<&str> {
    let value = parts.headers.get(name)?.to_str().ok()?;
    Some(value.split(',').next().unwrap_or_default().trim())
}

pub fn scheme(parts: &Parts) -> Option<&'static str> {
    let value = first_token(parts, X_FORWARDED_PROTO)?;
    ["https", "http"]
        .into_iter()
        .find(|candidate| value.eq_ignore_ascii_case(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    const URI: &str = "http://example.com/p/abc1234567/api/poll";

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
    fn missing_header_yields_no_scheme() {
        assert_eq!(scheme(&parts(URI, &[])), None);
    }

    #[test]
    fn recognised_schemes_match_case_insensitively() {
        for value in ["https", "HTTPS", "Https"] {
            assert_eq!(
                scheme(&parts(URI, &[("x-forwarded-proto", value)])),
                Some("https"),
                "for {value:?}"
            );
        }
        for value in ["http", "HTTP", "hTtP"] {
            assert_eq!(
                scheme(&parts(URI, &[("x-forwarded-proto", value)])),
                Some("http"),
                "for {value:?}"
            );
        }
    }

    #[test]
    fn the_first_token_of_a_proxy_chain_wins() {
        for value in ["https, http", " https ,http", "HTTPS,http"] {
            assert_eq!(
                scheme(&parts(URI, &[("x-forwarded-proto", value)])),
                Some("https"),
                "for {value:?}"
            );
        }
    }

    #[test]
    fn unrecognised_values_yield_no_scheme() {
        for value in ["ftp", "", "https://example.com", "gopher"] {
            assert_eq!(
                scheme(&parts(URI, &[("x-forwarded-proto", value)])),
                None,
                "for {value:?}"
            );
        }
    }

    #[test]
    fn first_token_reads_any_named_header() {
        assert_eq!(
            first_token(&parts(URI, &[("host", "internal:8080")]), header::HOST),
            Some("internal:8080")
        );
        assert_eq!(
            first_token(
                &parts(URI, &[("x-forwarded-host", "polls.example.com, internal")]),
                "x-forwarded-host"
            ),
            Some("polls.example.com")
        );
        assert_eq!(first_token(&parts(URI, &[]), header::HOST), None);
    }
}
