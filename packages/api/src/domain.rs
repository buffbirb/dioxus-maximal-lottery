//! Shared domain limits and newtypes. Compiled for both server and client.
//!
//! These newtypes are the source of truth for validity: they parse, sanitize,
//! and validate at construction time (including during serde deserialization),
//! so invalid payloads fail before they reach business logic.
use nutype::nutype;

/// Maximum length of a poll title, in Unicode characters.
pub const MAX_TITLE_LEN: usize = 200;

/// Maximum length of a poll description, in Unicode characters.
pub const MAX_DESCRIPTION_LEN: usize = 2000;

/// Minimum number of options required to create a poll.
pub const MIN_OPTIONS: usize = 2;

/// Maximum number of options allowed in a poll.
pub const MAX_OPTIONS: usize = 20;

/// Maximum length of an option label, in Unicode characters.
pub const MAX_OPTION_LABEL_LEN: usize = 200;

/// Length of the nanoid-based share/slug identifier for polls.
pub const SHARE_ID_LEN: usize = 10;

/// Poll title: non-empty after trimming, at most `MAX_TITLE_LEN` characters.
#[nutype(
    sanitize(trim),
    validate(not_empty, len_char_max = MAX_TITLE_LEN),
    derive(
        AsRef,
        Clone,
        Debug,
        Deserialize,
        Deref,
        Display,
        PartialEq,
        Serialize,
    ),
)]
pub struct Title(String);

/// Poll description: optional field, but if provided must be at most
/// `MAX_DESCRIPTION_LEN` characters after trimming. Empty strings are allowed
/// because the field is optional; callers should normalize empty to `None`.
#[nutype(
    sanitize(trim),
    validate(len_char_max = MAX_DESCRIPTION_LEN),
    derive(
        AsRef,
        Clone,
        Debug,
        Deserialize,
        Deref,
        Display,
        PartialEq,
        Serialize,
    ),
)]
pub struct Description(String);

/// A single option label: non-empty after trimming, at most
/// `MAX_OPTION_LABEL_LEN` characters.
#[nutype(
    sanitize(trim),
    validate(not_empty, len_char_max = MAX_OPTION_LABEL_LEN),
    derive(
        AsRef,
        Clone,
        Debug,
        Deserialize,
        Deref,
        Display,
        PartialEq,
        Serialize,
    ),
)]
pub struct OptionLabel(String);

/// A complete list of options for a new poll. Must contain between
/// `MIN_OPTIONS` and `MAX_OPTIONS` labels.
#[nutype(
    validate(predicate = |options| options.len() >= MIN_OPTIONS && options.len() <= MAX_OPTIONS),
    derive(
        AsRef,
        Clone,
        Debug,
        Deserialize,
        Deref,
        PartialEq,
        Serialize,
    ),
)]
pub struct Options(Vec<OptionLabel>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_rejects_empty() {
        assert!(Title::try_new("   ").is_err());
    }

    #[test]
    fn title_rejects_too_long() {
        assert!(Title::try_new("x".repeat(MAX_TITLE_LEN + 1)).is_err());
    }

    #[test]
    fn title_accepts_valid() {
        let title = Title::try_new("  A valid title  ").unwrap();
        assert_eq!(title.as_ref(), "A valid title");
    }

    #[test]
    fn description_allows_empty() {
        assert!(Description::try_new("").is_ok());
    }

    #[test]
    fn description_rejects_too_long() {
        assert!(Description::try_new("x".repeat(MAX_DESCRIPTION_LEN + 1)).is_err());
    }

    #[test]
    fn option_label_rejects_empty() {
        assert!(OptionLabel::try_new("   ").is_err());
    }

    #[test]
    fn option_label_rejects_too_long() {
        assert!(OptionLabel::try_new("x".repeat(MAX_OPTION_LABEL_LEN + 1)).is_err());
    }

    #[test]
    fn options_rejects_too_few() {
        let labels = vec![OptionLabel::try_new("A").unwrap()];
        assert!(Options::try_new(labels).is_err());
    }

    #[test]
    fn options_rejects_too_many() {
        let labels = (0..MAX_OPTIONS + 1)
            .map(|i| OptionLabel::try_new(format!("option {i}")).unwrap())
            .collect();
        assert!(Options::try_new(labels).is_err());
    }

    #[test]
    fn options_accepts_valid_count() {
        let labels = (0..MIN_OPTIONS)
            .map(|i| OptionLabel::try_new(format!("option {i}")).unwrap())
            .collect();
        assert!(Options::try_new(labels).is_ok());
    }
}
