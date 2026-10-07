//! The short-lived cookie carrying a sign-in attempt across the provider
//! round trip: `<state>:<code_verifier>:<return_to>`.
//!
//! - `:` is a safe separator: base64url never contains it, nor does a
//!   sanitised return path.
//! - `Path=/login` covers the callback; `SameSite=Lax` still sends it on the
//!   top-level GET back from the provider.
use oauth2::{CsrfToken, PkceCodeVerifier};
use subtle::ConstantTimeEq;

use super::return_to;

pub const NAME: &str = "oauth_state";
const MAX_AGE_SECS: u32 = 600;
/// 32 random bytes as unpadded base64url.
const VERIFIER_LEN: usize = 43;

pub struct OAuthState {
    pub state: String,
    pub code_verifier: String,
    pub return_to: String,
}

pub fn set_header(
    state: &CsrfToken,
    verifier: &PkceCodeVerifier,
    return_to: &str,
    secure: bool,
) -> String {
    let value = format!("{}:{}:{return_to}", state.secret(), verifier.secret());
    header(&value, MAX_AGE_SECS, secure)
}

pub fn clear_header(secure: bool) -> String {
    header("", 0, secure)
}

fn header(value: &str, max_age: u32, secure: bool) -> String {
    let mut header =
        format!("{NAME}={value}; Path=/login; HttpOnly; SameSite=Lax; Max-Age={max_age}");
    if secure {
        header.push_str("; Secure");
    }
    header
}

/// `None` unless all three fields are present and the verifier is well-formed.
pub fn parse(value: &str) -> Option<OAuthState> {
    let mut fields = value.splitn(3, ':');
    let state = fields.next().filter(|state| !state.is_empty())?;
    let code_verifier = fields.next().filter(|verifier| is_verifier(verifier))?;
    let return_to = fields.next()?;
    Some(OAuthState {
        state: state.to_string(),
        code_verifier: code_verifier.to_string(),
        // Cookies can be planted, so this is re-checked rather than trusted.
        return_to: return_to::sanitize(Some(return_to)),
    })
}

fn is_verifier(value: &str) -> bool {
    value.len() == VERIFIER_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// Constant-time over the content; a length mismatch fails without comparing.
pub fn matches(expected: &str, presented: &str) -> bool {
    expected.as_bytes().ct_eq(presented.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERIFIER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ0123-_6";

    #[test]
    fn set_header_has_every_attribute() {
        let state = CsrfToken::new("st".to_string());
        let verifier = PkceCodeVerifier::new(VERIFIER.to_string());
        assert_eq!(
            set_header(&state, &verifier, "/p/abc123", false),
            format!(
                "oauth_state=st:{VERIFIER}:/p/abc123; Path=/login; HttpOnly; SameSite=Lax; Max-Age=600"
            )
        );
    }

    #[test]
    fn clear_header_expires_the_same_path() {
        assert_eq!(
            clear_header(false),
            "oauth_state=; Path=/login; HttpOnly; SameSite=Lax; Max-Age=0"
        );
    }

    #[test]
    fn secure_is_added_only_when_requested() {
        let state = CsrfToken::new("st".to_string());
        let verifier = PkceCodeVerifier::new(VERIFIER.to_string());
        assert!(set_header(&state, &verifier, "/", true).ends_with("; Secure"));
        assert!(!set_header(&state, &verifier, "/", false).contains("Secure"));
        assert!(clear_header(true).ends_with("; Secure"));
    }

    #[test]
    fn random_values_round_trip() {
        let state = CsrfToken::new_random();
        let (_, verifier) = oauth2::PkceCodeChallenge::new_random_sha256();
        let header = set_header(&state, &verifier, "/p/abc123", false);
        let value = header
            .strip_prefix("oauth_state=")
            .and_then(|rest| rest.split_once(';'))
            .map(|(value, _)| value)
            .expect("cookie value");

        let parsed = parse(value).expect("parses");
        assert_eq!(parsed.state, *state.secret());
        assert_eq!(parsed.code_verifier, *verifier.secret());
        assert_eq!(parsed.return_to, "/p/abc123");
    }

    #[test]
    fn malformed_values_are_rejected() {
        for value in [
            "",
            "st",
            &format!("st:{VERIFIER}"),
            ":{VERIFIER}:/",
            "st::/",
            &format!("st:{}:/", &VERIFIER[1..]),
            &format!("st:{VERIFIER}a:/"),
            &format!("st:{}=:/", &VERIFIER[1..]),
            &format!("st:{}+:/", &VERIFIER[1..]),
        ] {
            assert!(parse(value).is_none(), "for {value:?}");
        }
    }

    #[test]
    fn a_planted_return_path_is_resanitised() {
        let parsed = parse(&format!("st:{VERIFIER}://evil.com")).expect("parses");
        assert_eq!(parsed.return_to, "/");
    }

    #[test]
    fn matches_requires_identical_values() {
        assert!(matches("abc", "abc"));
        assert!(!matches("abc", "abd"));
        assert!(!matches("abc", "ab"));
        assert!(!matches("abc", "abcd"));
        assert!(!matches("abc", ""));
    }
}
