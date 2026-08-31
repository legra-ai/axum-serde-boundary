//! A validated media type.

use std::fmt;

/// Why a media type was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidMediaType {
    reason: &'static str,
    value: String,
}

impl fmt::Display for InvalidMediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid media type: {} ({:?})", self.reason, self.value)
    }
}

impl std::error::Error for InvalidMediaType {}

/// A validated `type/subtype` media type, as stamped into
/// `Content-Type` headers and matched against `Accept`.
///
/// Limits: 1–255 bytes, exactly one `/` separating two non-empty
/// segments of HTTP token characters (RFC 9110 §5.6.2). Parameters are
/// not part of a `MediaType`; they belong to header rendering.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MediaType(&'static str);

impl MediaType {
    /// Validate and wrap a static media type.
    ///
    /// Representation tables are built once from compile-time
    /// constants, so the value is `'static`; validation still runs so
    /// a malformed constant fails fast at table construction.
    ///
    /// # Errors
    ///
    /// [`InvalidMediaType`] when the value is empty, longer than 255
    /// bytes, has no single `/`, or contains a non-token character.
    pub fn try_new(value: &'static str) -> Result<Self, InvalidMediaType> {
        let invalid = |reason| InvalidMediaType {
            reason,
            value: value.to_owned(),
        };
        if value.is_empty() {
            return Err(invalid("must not be empty"));
        }
        if value.len() > 255 {
            return Err(invalid("longer than 255 bytes"));
        }
        let Some((main, sub)) = value.split_once('/') else {
            return Err(invalid("must be type/subtype"));
        };
        if main.is_empty() || sub.is_empty() || sub.contains('/') {
            return Err(invalid("must be exactly two non-empty segments"));
        }
        let is_token_char = |b: u8| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b);
        if !main.bytes().all(is_token_char) || !sub.bytes().all(is_token_char) {
            return Err(invalid("segments must be HTTP token characters"));
        }
        Ok(Self(value))
    }

    /// The validated media type.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl AsRef<str> for MediaType {
    fn as_ref(&self) -> &str {
        self.0
    }
}
