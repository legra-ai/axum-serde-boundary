//! The application's declared response representations and the result
//! of negotiating one.

use std::fmt;

use http_content_negotiation::{
    ParsedAccept,
    Representation,
    RepresentationId,
};
use serde_stream_formats::EncodeFormat;

use crate::error::BoundaryError;
use crate::media::MediaType;

/// How the typed result is represented on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseMode {
    /// The payload DTO alone — the low-ceremony default.
    Payload,
    /// The payload wrapped in [`crate::ResponseEnvelope`] with
    /// operation metadata.
    Envelope,
}

/// One declared response representation: the media type `Accept`
/// selects (and the success `Content-Type` stamp), the streaming
/// format that renders it, the payload/envelope mode, and how errors
/// render when this representation is negotiated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseRepresentation {
    media_type: MediaType,
    format: EncodeFormat,
    mode: ResponseMode,
    error_format: EncodeFormat,
    error_media_type: MediaType,
}

/// The default error rendering: a JSON error envelope.
fn default_error_media_type() -> MediaType {
    MediaType::try_new("application/json").expect("the default error media type is valid")
}

impl ResponseRepresentation {
    /// Declare a representation. Errors default to a JSON error
    /// envelope stamped `application/json`; override with
    /// [`ResponseRepresentation::errors_as`].
    #[must_use]
    pub fn new(media_type: MediaType, format: EncodeFormat, mode: ResponseMode) -> Self {
        Self {
            media_type,
            format,
            mode,
            error_format: EncodeFormat::Json,
            error_media_type: default_error_media_type(),
        }
    }

    /// Render errors negotiated through this representation with
    /// `format`, stamped as `media_type` (e.g. a vendor
    /// `…envelope+yaml` type for a YAML representation).
    #[must_use]
    pub fn errors_as(mut self, format: EncodeFormat, media_type: MediaType) -> Self {
        self.error_format = format;
        self.error_media_type = media_type;
        self
    }

    /// The media type `Accept` matches and success responses stamp.
    #[must_use]
    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }
}

/// Why a response table was rejected at construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidResponseTable {
    reason: String,
}

impl fmt::Display for InvalidResponseTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid response table: {}", self.reason)
    }
}

impl std::error::Error for InvalidResponseTable {}

/// The application's response representations in server preference
/// order. The first entry is the contract default: it serves absent
/// and empty `Accept` headers, and wildcard ranges resolve to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseTable {
    // Bounded: an application's representation table is a small,
    // fixed declaration.
    representations: Vec<ResponseRepresentation>,
}

impl ResponseTable {
    /// Validate and build a table.
    ///
    /// # Errors
    ///
    /// [`InvalidResponseTable`] when the table is empty or declares
    /// the same media type twice.
    pub fn try_new(
        representations: Vec<ResponseRepresentation>,
    ) -> Result<Self, InvalidResponseTable> {
        if representations.is_empty() {
            return Err(InvalidResponseTable {
                reason: "a table needs at least one representation".to_owned(),
            });
        }
        for (index, entry) in representations.iter().enumerate() {
            let duplicate = representations[..index]
                .iter()
                .any(|earlier| earlier.media_type == entry.media_type);
            if duplicate {
                return Err(InvalidResponseTable {
                    reason: format!("duplicate media type {}", entry.media_type),
                });
            }
        }
        Ok(Self { representations })
    }

    /// Negotiate a representation from an `Accept` header value.
    ///
    /// An absent header, or one containing no parseable media range,
    /// selects the table's first entry.
    ///
    /// # Errors
    ///
    /// [`BoundaryError::NotAcceptable`] when the header is malformed
    /// or matches no declared representation (HTTP 406).
    pub fn negotiate(&self, accept: Option<&str>) -> Result<Negotiated, BoundaryError> {
        let Some(accept) = accept else {
            return Ok(Negotiated::from(&self.representations[0]));
        };
        let parsed = ParsedAccept::parse(accept).map_err(|_| BoundaryError::NotAcceptable {
            accept: accept.to_owned(),
        })?;
        if parsed.is_empty() {
            return Ok(Negotiated::from(&self.representations[0]));
        }
        // Bounded: one candidate per declared representation.
        let candidates = self
            .representations
            .iter()
            .map(|entry| {
                Representation::new(
                    RepresentationId::new(entry.media_type.as_str()),
                    entry.media_type.as_str(),
                )
            })
            .collect::<Vec<_>>();
        let selected =
            parsed
                .negotiate(&candidates)
                .ok_or_else(|| BoundaryError::NotAcceptable {
                    accept: accept.to_owned(),
                })?;
        let index = candidates
            .iter()
            .position(|candidate| std::ptr::eq(candidate, selected))
            .ok_or_else(|| BoundaryError::NotAcceptable {
                accept: accept.to_owned(),
            })?;
        Ok(Negotiated::from(&self.representations[index]))
    }
}

/// The settled outcome of negotiation: everything response rendering
/// needs, detached from the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Negotiated {
    format: EncodeFormat,
    mode: ResponseMode,
    content_type: MediaType,
    pub(crate) error_format: EncodeFormat,
    pub(crate) error_media_type: MediaType,
}

impl From<&ResponseRepresentation> for Negotiated {
    fn from(entry: &ResponseRepresentation) -> Self {
        Self {
            format: entry.format,
            mode: entry.mode,
            content_type: entry.media_type.clone(),
            error_format: entry.error_format,
            error_media_type: entry.error_media_type.clone(),
        }
    }
}

impl Negotiated {
    /// The negotiated streaming format.
    #[must_use]
    pub fn format(&self) -> EncodeFormat {
        self.format
    }

    /// The negotiated payload/envelope mode.
    #[must_use]
    pub fn mode(&self) -> ResponseMode {
        self.mode
    }

    /// The `Content-Type` of a success response in this negotiation.
    #[must_use]
    pub fn content_type(&self) -> &MediaType {
        &self.content_type
    }
}
