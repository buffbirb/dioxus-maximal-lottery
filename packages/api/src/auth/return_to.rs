//! Where to send the browser after sign-in.
//!
//! The result is embedded in an href, a cookie, and a Location header with no
//! encoding, so it is reduced to a strict path charset that can neither leave
//! the origin nor break out of any of those.
const FALLBACK: &str = "/";
const MAX_LEN: usize = 256;

/// The same-origin app path to return to, or `/` when the input is unusable.
pub fn sanitize(raw: Option<&str>) -> String {
    raw.and_then(safe_path).unwrap_or(FALLBACK).to_string()
}

fn safe_path(raw: &str) -> Option<&str> {
    let path = raw.split(['?', '#']).next()?;
    let ok = path.starts_with('/')
        && !path.starts_with("//")
        && path.len() <= MAX_LEN
        && path.bytes().all(is_path_byte)
        // Browsers resolve dot segments before requesting, so the prefix
        // check below would not see where /p/../login/x actually lands.
        && !path.split('/').any(|segment| segment == "." || segment == "..")
        // Returning to the login flow would loop.
        && path != "/login"
        && !path.starts_with("/login/");
    ok.then_some(path)
}

/// Excludes `\`, which some browsers treat as `/` in `/\host`.
fn is_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'~' | b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_empty_falls_back_to_root() {
        assert_eq!(sanitize(None), "/");
        assert_eq!(sanitize(Some("")), "/");
    }

    #[test]
    fn app_paths_are_kept() {
        for path in [
            "/",
            "/create",
            "/p/abc123",
            "/p/abc123/results",
            "/a.b_c~d-e",
        ] {
            assert_eq!(sanitize(Some(path)), path, "for {path:?}");
        }
    }

    #[test]
    fn query_and_fragment_are_dropped() {
        assert_eq!(sanitize(Some("/p/abc123?x=1")), "/p/abc123");
        assert_eq!(sanitize(Some("/p/abc123#top")), "/p/abc123");
        assert_eq!(sanitize(Some("/p/abc123?x=1#top")), "/p/abc123");
        assert_eq!(sanitize(Some("?x=1")), "/");
    }

    #[test]
    fn off_origin_and_unsafe_values_fall_back() {
        for value in [
            "//evil.com",
            "/\\evil.com",
            "https://evil.com",
            "evil.com",
            "/p/abc;x",
            "/p/abc%2F",
            "/p/abc 123",
            "/p/abc\t",
            "/p/abc\n",
            "/p/é",
            "/p/abc:x",
            "/p/\"x",
        ] {
            assert_eq!(sanitize(Some(value)), "/", "for {value:?}");
        }
    }

    #[test]
    fn login_paths_never_loop() {
        for value in [
            "/login",
            "/login/",
            "/login/github",
            "/login/github/callback",
        ] {
            assert_eq!(sanitize(Some(value)), "/", "for {value:?}");
        }
        assert_eq!(sanitize(Some("/loginx")), "/loginx");
    }

    #[test]
    fn dot_segments_fall_back() {
        for value in [
            "/p/../login/github",
            "/./login",
            "/p/abc123/..",
            "/p/./abc123",
            "/..",
        ] {
            assert_eq!(sanitize(Some(value)), "/", "for {value:?}");
        }
        assert_eq!(sanitize(Some("/p/a.b")), "/p/a.b");
        assert_eq!(sanitize(Some("/p/...")), "/p/...");
    }

    #[test]
    fn overlong_paths_fall_back() {
        let at_limit = format!("/{}", "a".repeat(MAX_LEN - 1));
        assert_eq!(sanitize(Some(&at_limit)), at_limit);
        let over = format!("/{}", "a".repeat(MAX_LEN));
        assert_eq!(sanitize(Some(&over)), "/");
    }
}
