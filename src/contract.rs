//! How a typed error crosses the HTTP boundary.

use axum::http::StatusCode;

use crate::envelope::ErrorBody;
use crate::error::BoundaryError;

/// How a domain error crosses the HTTP boundary: its status code, its
/// typed `CODE_*` wire code, and its client-consumable error body.
pub trait HttpErrorContract {
    /// The HTTP status this error maps to.
    fn status(&self) -> StatusCode;

    /// The machine-readable wire code — always a typed `CODE_*`
    /// associated constant, never a bare literal.
    fn wire_code(&self) -> &'static str;

    /// The human-readable explanation. Must not disclose secrets or
    /// internal topology.
    fn wire_message(&self) -> String;

    /// The error's client-consumable body.
    fn error_body(&self) -> ErrorBody {
        ErrorBody::new(self.wire_code(), self.wire_message())
    }

    /// Seconds after which the caller may retry, stamped as a
    /// `retry-after` header on the error response. `None` (the
    /// default) stamps nothing; implement for transient 503-class
    /// errors.
    fn retry_after(&self) -> Option<u32> {
        None
    }
}

impl HttpErrorContract for BoundaryError {
    fn status(&self) -> StatusCode {
        match self {
            Self::NotAcceptable { .. } => StatusCode::NOT_ACCEPTABLE,
            Self::UnsupportedMediaType { .. } => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::BodyRead { .. } | Self::MalformedBody { .. } => StatusCode::BAD_REQUEST,
            Self::ResponseEncoding { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn wire_code(&self) -> &'static str {
        match self {
            Self::NotAcceptable { .. } => ErrorBody::CODE_NOT_ACCEPTABLE,
            Self::UnsupportedMediaType { .. } => ErrorBody::CODE_UNSUPPORTED_MEDIA_TYPE,
            Self::PayloadTooLarge => ErrorBody::CODE_PAYLOAD_TOO_LARGE,
            Self::BodyRead { .. } | Self::MalformedBody { .. } => ErrorBody::CODE_MALFORMED_BODY,
            Self::ResponseEncoding { .. } => ErrorBody::CODE_RESPONSE_ENCODING,
        }
    }

    fn wire_message(&self) -> String {
        self.to_string()
    }
}
