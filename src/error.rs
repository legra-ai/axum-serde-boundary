//! The boundary's typed failure set.

use std::fmt;

use serde_stream_formats::FormatError;

/// A failure at the Serde HTTP boundary.
#[derive(Debug)]
pub enum BoundaryError {
    /// No declared representation satisfies the request's `Accept`
    /// header (HTTP 406).
    NotAcceptable {
        /// The offending `Accept` value.
        accept: String,
    },
    /// The request `Content-Type` is not a supported structured
    /// format (HTTP 415).
    UnsupportedMediaType {
        /// The offending `Content-Type` value.
        content_type: String,
    },
    /// The request body exceeded the configured size limit (HTTP 413).
    PayloadTooLarge,
    /// The request body could not be read (HTTP 400).
    BodyRead {
        /// Human-readable read detail.
        detail: String,
    },
    /// The request body is not a valid document in the negotiated
    /// format (HTTP 400).
    MalformedBody {
        /// The format's display name.
        format: &'static str,
        /// Decoder detail (position information where available).
        detail: String,
    },
    /// The response failed to encode in the negotiated format
    /// (HTTP 500).
    ResponseEncoding {
        /// The format's display name.
        format: &'static str,
        /// Encoder detail.
        detail: String,
    },
}

impl fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAcceptable { accept } => {
                write!(f, "no representation satisfies Accept: {accept}")
            }
            Self::UnsupportedMediaType { content_type } => {
                write!(f, "unsupported Content-Type: {content_type}")
            }
            Self::PayloadTooLarge => f.write_str("request body exceeded the configured limit"),
            Self::BodyRead { detail } => write!(f, "request body read failed: {detail}"),
            Self::MalformedBody { format, detail } => {
                write!(f, "malformed {format} request body: {detail}")
            }
            Self::ResponseEncoding { format, detail } => {
                write!(f, "{format} response encoding failed: {detail}")
            }
        }
    }
}

impl std::error::Error for BoundaryError {}

impl From<FormatError> for BoundaryError {
    fn from(error: FormatError) -> Self {
        match error {
            FormatError::MalformedDocument { format, detail } => {
                Self::MalformedBody { format, detail }
            }
            FormatError::PayloadTooLarge => Self::PayloadTooLarge,
            FormatError::Read { detail } => Self::BodyRead { detail },
            FormatError::Encoding { format, detail } => Self::ResponseEncoding { format, detail },
        }
    }
}
