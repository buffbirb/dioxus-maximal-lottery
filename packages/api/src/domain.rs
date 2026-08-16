//! Shared domain limits and newtypes. Compiled for both server and client.
//!
//! These newtypes are the source of truth for validity: they parse, sanitize,
//! and validate at construction time (including during serde deserialization),
//! so invalid payloads fail before they reach business logic.
use chrono::{DateTime, Utc};
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

/// Length of the nanoid-based share/slug identifier for polls. Minting
/// policy: changing it only affects ids minted from then on, so any change
/// must also be recorded in `MINTED_SHARE_ID_LENS`.
pub const SHARE_ID_LEN: usize = 10;

/// Nanoid's "nolookalikes safe" set: no vowels (ids never spell words)
/// and no characters easily confused by ear or handwriting. Minting
/// policy, like `SHARE_ID_LEN` - see `is_share_id_shaped` for why the
/// recognizer deliberately does not read this.
pub const SHARE_ID_ALPHABET: &[char] = &[
    '6', '7', '8', '9', 'B', 'C', 'D', 'F', 'G', 'H', 'J', 'K', 'L', 'M', 'N', 'P', 'Q', 'R', 'T',
    'W', 'b', 'c', 'd', 'f', 'g', 'h', 'j', 'k', 'm', 'n', 'p', 'q', 'r', 't', 'w', 'z',
];

/// Every length we have ever minted, so a link handed out under an older
/// `SHARE_ID_LEN` is still recognized. Append on change; never remove -
/// URLs already in the wild keep their length forever.
///
/// Spelled out rather than derived from `SHARE_ID_LEN`: deriving it would
/// silently follow the minting policy, which is exactly the coupling this
/// exists to break.
pub const MINTED_SHARE_ID_LENS: &[usize] = &[10];

/// Could this path segment plausibly be a share link we handed out?
///
/// Frozen on purpose, and looser than what we mint. Tying it to
/// `SHARE_ID_ALPHABET` would mean any future change to minting silently
/// reclassifies live links: drop a character from the alphabet and every
/// existing id containing it starts reading as "no such page." So the
/// recognizer only asks for a minted length and ASCII alphanumerics - a
/// superset no plausible minting policy escapes, enforced by
/// `minted_ids_stay_recognizable` below.
///
/// Being loose costs nothing, because this never gates a lookup: the
/// server always queries, and only reaches for this to choose the wording
/// on a 404. Erring loose also handles the common case better - a share
/// link mistyped into a lookalike (`0` for `Q`, `1` for `L`) is still a
/// dead poll, not a stray URL.
pub fn is_share_id_shaped(candidate: &str) -> bool {
    MINTED_SHARE_ID_LENS.contains(&candidate.chars().count())
        && candidate.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Default poll lifetime used to prefill the deadline field when creating a
/// poll. Every poll must have a deadline, so this is the fallback, not an
/// optional feature.
pub const DEFAULT_DEADLINE_DAYS: i64 = 7;

/// Minimum allowed value for a poll's vote cap.
pub const MIN_VOTE_CAP: i32 = 1;

/// Maximum allowed value for a poll's vote cap.
pub const MAX_VOTE_CAP: i32 = 1_000_000;

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

/// A poll's optional vote cap: closes the poll once this many votes have been
/// cast, independent of the deadline.
#[nutype(
    validate(greater_or_equal = MIN_VOTE_CAP, less_or_equal = MAX_VOTE_CAP),
    derive(
        AsRef,
        Clone,
        Copy,
        Debug,
        Deserialize,
        Deref,
        Display,
        PartialEq,
        Serialize,
    ),
)]
pub struct VoteCap(i32);

/// Whether a poll is closed: true once its deadline has passed (if it has
/// one) or its vote cap has been reached (if it has one). A poll with
/// neither never closes on its own.
pub fn poll_closed(
    deadline: Option<DateTime<Utc>>,
    vote_cap: Option<i32>,
    vote_count: i64,
    now: DateTime<Utc>,
) -> bool {
    let deadline_passed = deadline.is_some_and(|d| d <= now);
    let cap_reached = vote_cap.is_some_and(|cap| vote_count >= cap as i64);
    deadline_passed || cap_reached
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one invariant that keeps old links working: whatever we mint
    /// today has to still look like a share id tomorrow. Widening the
    /// minting alphabet past ASCII alphanumerics, or changing
    /// `SHARE_ID_LEN` without appending to `MINTED_SHARE_ID_LENS`, breaks
    /// every link already in the wild - so it breaks this test first.
    #[test]
    fn minted_ids_stay_recognizable() {
        assert!(
            MINTED_SHARE_ID_LENS.contains(&SHARE_ID_LEN),
            "SHARE_ID_LEN changed to {SHARE_ID_LEN}; append it to MINTED_SHARE_ID_LENS \
             instead of replacing the old length, or links already handed out \
             stop being recognized"
        );
        assert!(
            SHARE_ID_ALPHABET.iter().all(char::is_ascii_alphanumeric),
            "SHARE_ID_ALPHABET gained a non-alphanumeric character, which \
             is_share_id_shaped does not recognize"
        );

        let id: String = SHARE_ID_ALPHABET
            .iter()
            .cycle()
            .take(SHARE_ID_LEN)
            .collect();
        assert!(is_share_id_shaped(&id));
    }

    #[test]
    fn share_id_shape_rejects_the_wrong_length() {
        assert!(!is_share_id_shaped(""));
        assert!(!is_share_id_shaped("BCDFGHJKM"));
        assert!(!is_share_id_shaped("BCDFGHJKMNP"));
    }

    #[test]
    fn share_id_shape_rejects_ordinary_paths() {
        assert!(!is_share_id_shaped("about"));
        // Right length, but `.` is not alphanumeric.
        assert!(!is_share_id_shaped("robots.txt"));
    }

    /// Deliberate: these are not ids we would mint, but a mistyped share
    /// link looks exactly like this, and "no such poll" is the better
    /// answer for it than "no such page."
    #[test]
    fn share_id_shape_accepts_lookalikes_we_do_not_mint() {
        assert!(is_share_id_shaped("AEIOU01234"));
    }

    #[test]
    fn share_id_shape_counts_characters_not_bytes() {
        // Emoji are multi-byte; the alphanumeric check, not byte length, rejects them.
        assert!(!is_share_id_shaped(&"🎲".repeat(SHARE_ID_LEN)));
    }

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
    fn vote_cap_rejects_below_min() {
        assert!(VoteCap::try_new(MIN_VOTE_CAP - 1).is_err());
    }

    #[test]
    fn vote_cap_rejects_above_max() {
        assert!(VoteCap::try_new(MAX_VOTE_CAP + 1).is_err());
    }

    #[test]
    fn vote_cap_accepts_in_range() {
        assert!(VoteCap::try_new(MIN_VOTE_CAP).is_ok());
        assert!(VoteCap::try_new(MAX_VOTE_CAP).is_ok());
    }

    fn ymd(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        use chrono::TimeZone;
        Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap()
    }

    #[test]
    fn poll_closed_open_with_neither_condition() {
        let now = ymd(2026, 1, 1);
        assert!(!poll_closed(None, None, 1_000, now));
    }

    #[test]
    fn poll_closed_by_deadline_only() {
        let now = ymd(2026, 1, 2);
        assert!(poll_closed(Some(ymd(2026, 1, 1)), None, 0, now));
        assert!(!poll_closed(Some(ymd(2026, 1, 3)), None, 0, now));
    }

    #[test]
    fn poll_closed_by_vote_cap_only() {
        let now = ymd(2026, 1, 1);
        assert!(poll_closed(None, Some(10), 10, now));
        assert!(poll_closed(None, Some(10), 11, now));
        assert!(!poll_closed(None, Some(10), 9, now));
    }

    #[test]
    fn poll_closed_when_either_condition_is_met() {
        let now = ymd(2026, 1, 2);
        // Deadline passed, cap not reached.
        assert!(poll_closed(Some(ymd(2026, 1, 1)), Some(10), 5, now));
        // Cap reached, deadline not passed.
        assert!(poll_closed(Some(ymd(2026, 1, 3)), Some(10), 10, now));
        // Neither met.
        assert!(!poll_closed(Some(ymd(2026, 1, 3)), Some(10), 5, now));
    }

    #[test]
    fn options_accepts_valid_count() {
        let labels = (0..MIN_OPTIONS)
            .map(|i| OptionLabel::try_new(format!("option {i}")).unwrap())
            .collect();
        assert!(Options::try_new(labels).is_ok());
    }
}
