//! A poll's public identifier, safe to embed in URLs and response headers by
//! construction.
//!
//! A [`ShareId`] can only be produced by [`ShareId::mint`] or by the
//! validating [`ShareId::try_new`], so every value of the type is already
//! vetted and no caller has to re-check it before interpolating it into a
//! header or a URL. The database enforces the same predicate with a `CHECK`
//! constraint, so a value read back from storage is vetted too.
use nutype::nutype;
use sqlx::error::BoxDynError;
use sqlx::postgres::{PgTypeInfo, PgValueRef};
use sqlx::{Decode, Postgres, Type};

/// Mint-time only: nothing ever checks an existing id back against these,
/// so changing either can't strand ids already in the wild.
const SHARE_ID_LEN: usize = 10;

/// Nanoid's "nolookalikes safe" set: no vowels (ids never spell words)
/// and no characters easily confused by ear or handwriting.
const SHARE_ID_ALPHABET: &[char] = &[
    '6', '7', '8', '9', 'B', 'C', 'D', 'F', 'G', 'H', 'J', 'K', 'L', 'M', 'N', 'P', 'Q', 'R', 'T',
    'W', 'b', 'c', 'd', 'f', 'g', 'h', 'j', 'k', 'm', 'n', 'p', 'q', 'r', 't', 'w', 'z',
];

/// Whether `c` is an ASCII letter or digit.
const fn is_ascii_alphanumeric(c: char) -> bool {
    matches!(c, '0'..='9' | 'A'..='Z' | 'a'..='z')
}

/// The predicate a [`ShareId`] must satisfy. Alphanumeric is the smallest
/// closed set that is safe everywhere a share id appears.
fn is_safe(value: &str) -> bool {
    !value.is_empty() && value.chars().all(is_ascii_alphanumeric)
}

/// A poll's public identifier.
#[nutype(
    validate(predicate = is_safe),
    derive(AsRef, Clone, Debug, Display, Eq, PartialEq),
)]
pub struct ShareId(String);

impl ShareId {
    /// Mint a fresh share id.
    pub fn mint() -> Self {
        ShareId::try_new(nanoid::nanoid!(SHARE_ID_LEN, SHARE_ID_ALPHABET)).expect(
            "mint produced an id the ShareId predicate rejects: SHARE_ID_ALPHABET and is_safe disagree",
        )
    }
}

/// Every character [`ShareId::mint`] can produce must be accepted by
/// [`is_safe`], or a freshly minted id would not parse back into its own
/// type. A compile-time check rather than a test: it fails the build, not a
/// run.
const _: () = {
    let mut idx = 0;
    while idx < SHARE_ID_ALPHABET.len() {
        assert!(is_ascii_alphanumeric(SHARE_ID_ALPHABET[idx]));
        idx += 1;
    }
};

impl Type<Postgres> for ShareId {
    fn type_info() -> PgTypeInfo {
        <String as Type<Postgres>>::type_info()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        <String as Type<Postgres>>::compatible(ty)
    }
}

impl<'r> Decode<'r, Postgres> for ShareId {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let text = <&str as Decode<Postgres>>::decode(value)?;
        ShareId::try_new(text).map_err(|err| {
            format!("stored share_id does not satisfy the ShareId invariant: {err}").into()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minted_ids_satisfy_the_type() {
        for _ in 0..64 {
            let id = ShareId::mint();
            assert_eq!(id.as_ref().len(), SHARE_ID_LEN);
            assert!(is_safe(id.as_ref()));
            assert!(ShareId::try_new(id.as_ref()).is_ok());
        }
    }

    #[test]
    fn alphanumeric_values_are_accepted() {
        for value in ["a", "abc1234567", "ABCXYZ", "0123456789"] {
            assert!(ShareId::try_new(value).is_ok(), "for {value:?}");
        }
    }

    #[test]
    fn unsafe_values_are_rejected() {
        for value in [
            "",
            "a;b",
            "a,b",
            "a b",
            "a/b",
            "a=b",
            "a\tb",
            "a\r\nb",
            "a%b",
            "\"quoted\"",
            "🚀",
            "café",
        ] {
            assert!(ShareId::try_new(value).is_err(), "for {value:?}");
        }
    }
}
