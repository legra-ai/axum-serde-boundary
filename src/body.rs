//! Typed request-body extraction in the format named by
//! `Content-Type`, decoded incrementally.

use axum::RequestExt;
use axum::extract::{
    FromRequest,
    Request,
};
use axum::http::header;
use http_body_util::LengthLimitError;
use serde::de::DeserializeOwned;
use serde_stream_formats::{
    DecodeFormat,
    PayloadLimitExceeded,
};
use tokio_stream::StreamExt;
use tokio_util::io::{
    StreamReader,
    SyncIoBridge,
};

use crate::error::BoundaryError;
use crate::respond::BoundaryRejection;

/// A typed request body decoded in the structured format named by the
/// request's `Content-Type`.
///
/// - No `Content-Type` → decoded as JSON, the documented contract default.
/// - An unsupported `Content-Type` → typed 415.
/// - A body over the configured axum body limit → typed 413.
/// - A body that fails to decode → typed 400 with decoder detail.
///
/// The decoder runs on a blocking worker while the reader pulls from
/// the request stream, so the HTTP task never owns the complete body.
/// All rejections carry the standard error envelope, never plain text.
#[derive(Debug, Clone)]
pub struct SerdeBody<T>(pub T);

/// Resolve the request's decode format from its `Content-Type`.
fn request_format(request: &Request) -> Result<DecodeFormat, BoundaryError> {
    let Some(content_type) = request.headers().get(header::CONTENT_TYPE) else {
        return Ok(DecodeFormat::Json);
    };
    let Ok(content_type) = content_type.to_str() else {
        return Err(BoundaryError::UnsupportedMediaType {
            content_type: String::from("<non-ASCII Content-Type>"),
        });
    };
    DecodeFormat::from_content_type(content_type).ok_or_else(|| {
        BoundaryError::UnsupportedMediaType {
            content_type: content_type.to_owned(),
        }
    })
}

impl<S, T> FromRequest<S> for SerdeBody<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send + 'static,
{
    type Rejection = BoundaryRejection;

    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let format = request_format(&request).map_err(BoundaryRejection::new)?;
        let handle = tokio::runtime::Handle::current();
        let body = request
            .into_limited_body()
            .into_data_stream()
            .map(|result| {
                result.map_err(|error| {
                    let inner = error.into_inner();
                    if inner.downcast_ref::<LengthLimitError>().is_some() {
                        std::io::Error::other(PayloadLimitExceeded)
                    } else {
                        std::io::Error::other(inner)
                    }
                })
            });
        let reader = StreamReader::new(body);
        let value = tokio::task::spawn_blocking(move || {
            format
                .decode_reader(SyncIoBridge::new_with_handle(reader, handle))
                .map_err(BoundaryError::from)
        })
        .await
        .map_err(|error| {
            BoundaryRejection::new(BoundaryError::BodyRead {
                detail: format!("body decoder task failed: {error}"),
            })
        })?
        .map_err(BoundaryRejection::new)?;
        Ok(Self(value))
    }
}
