//! Rendering typed results and boundary failures through the
//! negotiated representation.

use axum::http::{
    StatusCode,
    header,
};
use axum::response::{
    IntoResponse,
    Response,
};
use serde::Serialize;
use serde_stream_formats::EncodeFormat;
use tokio_stream::StreamExt;

use crate::contract::HttpErrorContract;
use crate::envelope::{
    ErrorBody,
    ResponseEnvelope,
};
use crate::error::BoundaryError;
use crate::media::MediaType;
use crate::table::{
    Negotiated,
    ResponseMode,
};

impl Negotiated {
    /// Render a typed result through this negotiation: the payload in
    /// the negotiated format and mode on success, the error envelope
    /// on failure (errors always carry the envelope body).
    pub fn respond<T, E>(&self, result: Result<T, E>) -> Response
    where
        T: Serialize + Send + 'static,
        E: HttpErrorContract,
    {
        self.respond_with_status(StatusCode::OK, result)
    }

    /// [`Negotiated::respond`] with an explicit success status
    /// (`201`, `202`, …).
    pub fn respond_with_status<T, E>(
        &self,
        success_status: StatusCode,
        result: Result<T, E>,
    ) -> Response
    where
        T: Serialize + Send + 'static,
        E: HttpErrorContract,
    {
        match result {
            Ok(payload) => self.render_success(success_status, payload),
            Err(error) => {
                let mut response = render_error(
                    self.error_format,
                    &self.error_media_type,
                    error.status(),
                    error.error_body(),
                );
                if let Some(seconds) = error.retry_after()
                    && let Ok(value) = seconds.to_string().parse()
                {
                    response.headers_mut().insert("retry-after", value);
                }
                response
            }
        }
    }

    /// Stream the success representation; an encoding failure
    /// surfaces through the body as a typed error, never masked.
    ///
    /// A `204 No Content` success is bodyless by HTTP semantics — no
    /// payload is serialized and no `Content-Type` is stamped, in
    /// either mode.
    fn render_success<T>(&self, status: StatusCode, payload: T) -> Response
    where
        T: Serialize + Send + 'static,
    {
        if status == StatusCode::NO_CONTENT {
            return status.into_response();
        }
        let stream = match self.mode() {
            ResponseMode::Payload => self.format().encode_stream(payload),
            ResponseMode::Envelope => self.format().encode_stream(ResponseEnvelope::ok(payload)),
        };
        let body =
            axum::body::Body::from_stream(stream.map(|item| item.map_err(BoundaryError::from)));
        (
            status,
            [(header::CONTENT_TYPE, self.content_type().as_str())],
            body,
        )
            .into_response()
    }
}

/// Render an error body as a failure envelope in `format`, stamped as
/// `media_type`.
///
/// The envelope is encoded before headers are sent so the status is
/// always truthful. If the envelope itself fails to encode, a static
/// JSON envelope is emitted — the failure is never an empty 2xx or a
/// bodyless response.
#[must_use]
pub fn render_error(
    format: EncodeFormat,
    media_type: &MediaType,
    status: StatusCode,
    body: ErrorBody,
) -> Response {
    let envelope = ResponseEnvelope::<()>::err(body);
    match format.encode_vec(&envelope) {
        Ok(bytes) => (status, [(header::CONTENT_TYPE, media_type.as_str())], bytes).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "application/json")],
            format!(
                "{{\"error\":{{\"code\":\"{}\",\"message\":\"error envelope failed to encode\"}}}}",
                ErrorBody::CODE_RESPONSE_ENCODING
            ),
        )
            .into_response(),
    }
}

/// A boundary failure raised by an extractor, rendered as the typed
/// JSON error envelope.
#[derive(Debug)]
pub struct BoundaryRejection {
    error: BoundaryError,
}

impl BoundaryRejection {
    /// Wrap a boundary error for use as an extractor rejection.
    #[must_use]
    pub fn new(error: BoundaryError) -> Self {
        Self { error }
    }

    /// The underlying boundary error.
    #[must_use]
    pub fn error(&self) -> &BoundaryError {
        &self.error
    }
}

impl IntoResponse for BoundaryRejection {
    fn into_response(self) -> Response {
        let media_type =
            MediaType::try_new("application/json").expect("the JSON media type is valid");
        render_error(
            EncodeFormat::Json,
            &media_type,
            self.error.status(),
            self.error.error_body(),
        )
    }
}
