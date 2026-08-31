# axum-serde-boundary

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![CI][ci-badge]][ci-url]
[![License][license-badge]][license-url]
[![Downloads][downloads-badge]][downloads-url]

A typed Serde HTTP boundary for [axum]: streaming request-body
extraction, `Accept`-negotiated streaming responses, and a
machine-readable error envelope.

Handlers return `Result<Dto, DomainError>`; this crate settles which
representation goes on the wire and renders it — without the handler
parsing headers, choosing a serializer, or ever materializing a body:

- **`SerdeBody<T>`** — decodes the request body in the format named by
  `Content-Type` (JSON by default; `MessagePack` and Postcard supported),
  incrementally on a blocking worker: the HTTP task never owns the
  complete body. Failures are typed — 415 for an unsupported media type,
  413 when the body limit trips, 400 with decoder detail — and always
  carry the error envelope, never plain text.
- **`ResponseTable`** — the application's declared representations in
  preference order, each a validated [`MediaType`] plus a streaming
  format ([`serde-stream-formats`]) and a payload-or-envelope mode, with
  a per-entry error-rendering policy. `negotiate(accept)` settles one
  `Negotiated` before the domain operation runs, so an unsatisfiable
  request 406es up front.
- **`Negotiated::respond(result)`** — streams the success payload in the
  negotiated format (bounded chunks, backpressure; a serialization
  failure surfaces as a typed body error, never a silent truncation),
  or renders the failure as a `ResponseEnvelope` with the error's
  `HttpErrorContract` status, typed `CODE_*` wire code, and optional
  `retry-after`. `204 No Content` stays bodyless.
- **`ResponseEnvelope<T>` / `ErrorBody`** — the envelope shape: payload
  or error plus at most 16 warnings; error codes are always typed
  constants, and applications extend the set with their own.

```rust
use axum_serde_boundary::{
    MediaType, ResponseMode, ResponseRepresentation, ResponseTable, SerdeBody,
};
use serde_stream_formats::EncodeFormat;

# #[derive(serde::Serialize, serde::Deserialize)]
# struct Point { x: u32, y: u32 }
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let table = ResponseTable::try_new(vec![
    ResponseRepresentation::new(
        MediaType::try_new("application/json")?,
        EncodeFormat::Json,
        ResponseMode::Payload,
    ),
    ResponseRepresentation::new(
        MediaType::try_new("application/vnd.example.envelope+json")?,
        EncodeFormat::Json,
        ResponseMode::Envelope,
    ),
])?;

// In a handler:
async fn create(body: SerdeBody<Point>) -> axum::response::Response {
    # let table = ResponseTable::try_new(vec![ResponseRepresentation::new(
    #     MediaType::try_new("application/json").unwrap(),
    #     EncodeFormat::Json,
    #     ResponseMode::Payload,
    # )]).unwrap();
    let negotiated = table.negotiate(Some("application/json")).unwrap();
    negotiated.respond::<_, axum_serde_boundary::BoundaryError>(Ok(body.0))
}
# let _ = table;
# let _ = create;
# Ok(())
# }
```

Everything streams: responses are encoded through a bounded channel
(64 KiB chunks) and requests are decoded from the live stream. There is
no `Vec<u8>` of a whole body anywhere on the success path — the one
deliberate exception is the error envelope, which is small, bounded, and
encoded before headers are sent so the status is always truthful.

[axum]: https://docs.rs/axum
[`serde-stream-formats`]: https://crates.io/crates/serde-stream-formats
[`MediaType`]: https://docs.rs/axum-serde-boundary/latest/axum_serde_boundary/struct.MediaType.html

## License

Licensed under either of:

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE));
- MIT License ([`LICENSE-MIT`](LICENSE-MIT)).

## Links

[crates-badge]: https://img.shields.io/crates/v/axum-serde-boundary.svg
[crates-url]: https://crates.io/crates/axum-serde-boundary
[docs-badge]: https://docs.rs/axum-serde-boundary/badge.svg
[docs-url]: https://docs.rs/axum-serde-boundary
[ci-badge]: https://github.com/legra-ai/axum-serde-boundary/actions/workflows/ci.yml/badge.svg
[ci-url]: https://github.com/legra-ai/axum-serde-boundary/actions/workflows/ci.yml
[license-badge]: https://img.shields.io/crates/l/axum-serde-boundary.svg
[license-url]: https://github.com/legra-ai/axum-serde-boundary/blob/main/LICENSE-APACHE
[downloads-badge]: https://img.shields.io/crates/d/axum-serde-boundary.svg
[downloads-url]: https://crates.io/crates/axum-serde-boundary
