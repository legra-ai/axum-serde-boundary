//! Negotiation, rendering, extraction, and validation behaviour.

use axum::Router;
use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::{
    Request,
    StatusCode,
};
use axum::routing::post;
use http_body_util::BodyExt;
use serde::{
    Deserialize,
    Serialize,
};
use serde_stream_formats::EncodeFormat;
use tower::ServiceExt;

use crate::{
    BoundaryError,
    ErrorBody,
    HttpErrorContract,
    MediaType,
    ResponseEnvelope,
    ResponseMode,
    ResponseRepresentation,
    ResponseTable,
    SerdeBody,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Point {
    x: u32,
    y: u32,
}

fn media(value: &'static str) -> MediaType {
    MediaType::try_new(value).expect("valid media type")
}

fn table() -> ResponseTable {
    ResponseTable::try_new(vec![
        ResponseRepresentation::new(
            media("application/json"),
            EncodeFormat::Json,
            ResponseMode::Payload,
        ),
        ResponseRepresentation::new(
            media("application/yaml"),
            EncodeFormat::Yaml,
            ResponseMode::Payload,
        )
        .errors_as(
            EncodeFormat::Yaml,
            media("application/vnd.example.envelope+yaml"),
        ),
        ResponseRepresentation::new(
            media("application/vnd.example.envelope+json"),
            EncodeFormat::Json,
            ResponseMode::Envelope,
        ),
    ])
    .expect("valid table")
}

struct TeapotError;

impl HttpErrorContract for TeapotError {
    fn status(&self) -> StatusCode {
        StatusCode::IM_A_TEAPOT
    }

    fn wire_code(&self) -> &'static str {
        "TEAPOT"
    }

    fn wire_message(&self) -> String {
        "short and stout".to_owned()
    }

    fn retry_after(&self) -> Option<u32> {
        Some(30)
    }
}

async fn body_bytes(response: axum::response::Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes()
        .to_vec()
}

#[test]
fn absent_and_empty_accept_select_the_first_entry() {
    for accept in [None, Some(""), Some(" , ,")] {
        let negotiated = table().negotiate(accept).expect("default");
        assert_eq!(negotiated.content_type().as_str(), "application/json");
    }
}

#[test]
fn q_values_steer_the_selection() {
    let negotiated = table()
        .negotiate(Some("application/json;q=0.1, application/yaml"))
        .expect("negotiate");
    assert_eq!(negotiated.content_type().as_str(), "application/yaml");
}

#[test]
fn wildcards_resolve_to_the_first_entry() {
    let negotiated = table().negotiate(Some("*/*")).expect("negotiate");
    assert_eq!(negotiated.content_type().as_str(), "application/json");
}

#[test]
fn unmatched_and_malformed_accepts_are_406() {
    for accept in ["text/plain", "application/json;q=9"] {
        let err = table().negotiate(Some(accept)).expect_err("must fail");
        assert!(
            matches!(err, BoundaryError::NotAcceptable { .. }),
            "{accept}"
        );
    }
}

#[test]
fn tables_reject_duplicates_and_emptiness() {
    assert!(ResponseTable::try_new(Vec::new()).is_err());
    let duplicate = ResponseTable::try_new(vec![
        ResponseRepresentation::new(
            media("application/json"),
            EncodeFormat::Json,
            ResponseMode::Payload,
        ),
        ResponseRepresentation::new(
            media("application/json"),
            EncodeFormat::Json,
            ResponseMode::Envelope,
        ),
    ]);
    assert!(duplicate.is_err());
}

#[tokio::test]
async fn payload_mode_streams_the_bare_dto() {
    let negotiated = table().negotiate(Some("application/json")).expect("json");
    let response = negotiated.respond::<_, BoundaryError>(Ok(Point { x: 3, y: 4 }));
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/json");
    let point: Point = serde_json::from_slice(&body_bytes(response).await).expect("payload");
    assert_eq!(point, Point { x: 3, y: 4 });
}

#[tokio::test]
async fn envelope_mode_wraps_the_payload() {
    let negotiated = table()
        .negotiate(Some("application/vnd.example.envelope+json"))
        .expect("envelope");
    let response = negotiated.respond::<_, BoundaryError>(Ok(Point { x: 1, y: 2 }));
    assert_eq!(
        response.headers()["content-type"],
        "application/vnd.example.envelope+json"
    );
    let envelope: ResponseEnvelope<Point> =
        serde_json::from_slice(&body_bytes(response).await).expect("envelope");
    assert_eq!(envelope.payload, Some(Point { x: 1, y: 2 }));
    assert!(envelope.error.is_none());
}

#[tokio::test]
async fn errors_render_the_envelope_with_the_entry_policy() {
    let negotiated = table().negotiate(Some("application/yaml")).expect("yaml");
    let response = negotiated.respond::<Point, _>(Err(TeapotError));
    assert_eq!(response.status(), StatusCode::IM_A_TEAPOT);
    assert_eq!(
        response.headers()["content-type"],
        "application/vnd.example.envelope+yaml"
    );
    assert_eq!(response.headers()["retry-after"], "30");
}

#[tokio::test]
async fn no_content_success_is_bodyless() {
    let negotiated = table().negotiate(Some("application/json")).expect("json");
    let response =
        negotiated.respond_with_status::<_, BoundaryError>(StatusCode::NO_CONTENT, Ok(()));
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(response.headers().get("content-type").is_none());
    assert_eq!(body_bytes(response).await, Vec::<u8>::new());
}

#[tokio::test]
async fn encoding_failures_surface_as_typed_stream_errors() {
    struct Unencodable;
    impl Serialize for Unencodable {
        fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("deliberately unencodable"))
        }
    }
    let negotiated = table().negotiate(Some("application/json")).expect("json");
    let response = negotiated.respond::<_, BoundaryError>(Ok(Unencodable));
    let error = response
        .into_body()
        .into_data_stream()
        .next()
        .await
        .expect("failure frame")
        .expect_err("body must fail");
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(&error);
    let mut typed = false;
    while let Some(current) = source {
        if matches!(
            current.downcast_ref::<BoundaryError>(),
            Some(BoundaryError::ResponseEncoding { .. })
        ) {
            typed = true;
            break;
        }
        source = current.source();
    }
    assert!(typed, "stream error must stay a typed BoundaryError");
}

use tokio_stream::StreamExt as _;

fn echo_app() -> Router {
    Router::new()
        .route(
            "/echo",
            post(|SerdeBody(point): SerdeBody<Point>| async move { axum::Json(point) }),
        )
        .layer(DefaultBodyLimit::max(256))
}

async fn send(app: Router, content_type: Option<&str>, body: &[u8]) -> axum::response::Response {
    let mut request = Request::builder().method("POST").uri("/echo");
    if let Some(content_type) = content_type {
        request = request.header("content-type", content_type);
    }
    app.oneshot(request.body(Body::from(body.to_vec())).expect("request"))
        .await
        .expect("response")
}

#[tokio::test]
async fn missing_content_type_decodes_as_json() {
    let response = send(echo_app(), None, br#"{"x":7,"y":8}"#).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn msgpack_bodies_decode_incrementally() {
    let bytes = rmp_serde::to_vec_named(&Point { x: 5, y: 6 }).expect("encode");
    let response = send(echo_app(), Some("application/vnd.msgpack"), &bytes).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn unsupported_content_type_is_a_typed_415() {
    let response = send(echo_app(), Some("text/plain"), b"x=1").await;
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let envelope: ResponseEnvelope<()> =
        serde_json::from_slice(&body_bytes(response).await).expect("envelope");
    assert_eq!(
        envelope.error.expect("error").code,
        ErrorBody::CODE_UNSUPPORTED_MEDIA_TYPE
    );
}

#[tokio::test]
async fn oversized_bodies_are_a_typed_413() {
    let huge = format!(r#"{{"x":1,"y":2,"pad":"{}"}}"#, "a".repeat(512));
    let response = send(echo_app(), Some("application/json"), huge.as_bytes()).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let envelope: ResponseEnvelope<()> =
        serde_json::from_slice(&body_bytes(response).await).expect("envelope");
    assert_eq!(
        envelope.error.expect("error").code,
        ErrorBody::CODE_PAYLOAD_TOO_LARGE
    );
}

#[tokio::test]
async fn malformed_bodies_are_a_typed_400_with_detail() {
    let response = send(echo_app(), Some("application/json"), b"{\"x\": }").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let envelope: ResponseEnvelope<()> =
        serde_json::from_slice(&body_bytes(response).await).expect("envelope");
    let error = envelope.error.expect("error");
    assert_eq!(error.code, ErrorBody::CODE_MALFORMED_BODY);
    assert!(error.message.contains("JSON"), "{}", error.message);
}

#[test]
fn media_types_enforce_token_syntax() {
    assert!(MediaType::try_new("application/vnd.example+json").is_ok());
    for value in [
        "",
        "noslash",
        "a/b/c",
        "/sub",
        "main/",
        "sp ace/json",
        "app/js{on}",
    ] {
        assert!(MediaType::try_new(value).is_err(), "{value:?}");
    }
}

#[test]
fn envelope_warnings_stay_bounded() {
    let mut envelope = ResponseEnvelope::ok(());
    for index in 0..100 {
        envelope.push_warning(format!("warning {index}"));
    }
    assert_eq!(envelope.warnings.len(), 16);
}
