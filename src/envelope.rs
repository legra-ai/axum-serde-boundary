//! The envelope-mode response contract and the machine-readable error
//! contract.
//!
//! Envelope metadata is bounded and deliberately spare: put nothing in
//! it that must not reach a client.

use serde::{
    Deserialize,
    Serialize,
};

/// The maximum number of warnings an envelope carries; later warnings
/// are dropped, never accumulated unboundedly.
const MAX_ENVELOPE_WARNINGS: usize = 16;

/// The envelope-mode response body: the payload (on success) or the
/// error contract (on failure), plus bounded operation metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseEnvelope<T> {
    /// The operation's payload; present exactly when the operation
    /// succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<T>,

    /// The error contract; present exactly when the operation failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,

    /// Non-fatal, human-readable warnings.
    // Capped at MAX_ENVELOPE_WARNINGS by push_warning.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl<T> ResponseEnvelope<T> {
    /// A success envelope around `payload`.
    #[must_use]
    pub fn ok(payload: T) -> Self {
        Self {
            payload: Some(payload),
            error: None,
            warnings: Vec::new(),
        }
    }

    /// A failure envelope around `error`.
    #[must_use]
    pub fn err(error: ErrorBody) -> Self {
        Self {
            payload: None,
            error: Some(error),
            warnings: Vec::new(),
        }
    }

    /// Append a warning, dropping it silently once the bounded
    /// capacity is reached.
    pub fn push_warning(&mut self, warning: impl Into<String>) {
        if self.warnings.len() < MAX_ENVELOPE_WARNINGS {
            self.warnings.push(warning.into());
        }
    }
}

/// The machine-readable error contract carried by every error
/// response (and by failure envelopes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// The machine-readable error code. Always sourced from a typed
    /// `CODE_*` associated constant — the application defines its own
    /// constants for domain errors; never a bare string literal at a
    /// call site.
    pub code: String,

    /// The human-readable explanation.
    pub message: String,

    /// Field-level validation failures, when the error is a request
    /// validation failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationDetails>,
}

impl ErrorBody {
    /// No representation satisfies the request's `Accept` header
    /// (HTTP 406).
    pub const CODE_NOT_ACCEPTABLE: &'static str = "NOT_ACCEPTABLE";
    /// The request `Content-Type` is not supported (HTTP 415).
    pub const CODE_UNSUPPORTED_MEDIA_TYPE: &'static str = "UNSUPPORTED_MEDIA_TYPE";
    /// The request body exceeds the permitted size (HTTP 413).
    pub const CODE_PAYLOAD_TOO_LARGE: &'static str = "PAYLOAD_TOO_LARGE";
    /// The request body is unreadable or not a valid document in the
    /// negotiated format (HTTP 400).
    pub const CODE_MALFORMED_BODY: &'static str = "MALFORMED_BODY";
    /// The response failed to encode in the negotiated format
    /// (HTTP 500).
    pub const CODE_RESPONSE_ENCODING: &'static str = "RESPONSE_ENCODING";

    /// An error body from a typed code constant and a message.
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
            validation: None,
        }
    }

    /// Attach field-level validation details.
    #[must_use]
    pub fn with_validation(mut self, validation: ValidationDetails) -> Self {
        self.validation = Some(validation);
        self
    }
}

/// Field-level detail for a request validation failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationDetails {
    /// The failing fields — one entry per invalid field of one
    /// request body.
    pub fields: Vec<FieldError>,
}

/// One invalid field in a request body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldError {
    /// A JSON-Pointer-style location of the failing field.
    pub pointer: String,
    /// Why the field was rejected.
    pub message: String,
}
