//! Public-API integration test: a negotiated response table renders a
//! payload in the requested representation, from the shipped crate.

use axum::body::to_bytes;
use axum_serde_boundary::{
    BoundaryError,
    MediaType,
    ResponseMode,
    ResponseRepresentation,
    ResponseTable,
};
use serde_stream_formats::EncodeFormat;

#[derive(serde::Serialize)]
struct Point {
    x: u32,
    y: u32,
}

#[tokio::test]
async fn negotiated_json_payload_renders_the_value() {
    let table = ResponseTable::try_new(vec![ResponseRepresentation::new(
        MediaType::try_new("application/json").expect("media type"),
        EncodeFormat::Json,
        ResponseMode::Payload,
    )])
    .expect("one representation");
    let negotiated = table
        .negotiate(Some("application/json"))
        .expect("json is offered");
    let response = negotiated.respond::<_, BoundaryError>(Ok(Point { x: 3, y: 4 }));
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()["content-type"].to_str().unwrap(),
        "application/json"
    );
    let body = to_bytes(response.into_body(), 1024).await.expect("body");
    let value: serde_json::Value = serde_json::from_slice(&body).expect("json body");
    assert_eq!(value["x"], 3);
    assert_eq!(value["y"], 4);
}
