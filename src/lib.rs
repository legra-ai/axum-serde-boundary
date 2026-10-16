#![doc = include_str!("../README.md")]

mod body;
mod contract;
mod envelope;
mod error;
mod respond;
mod table;

#[cfg(test)]
mod tests;

pub use body::SerdeBody;
pub use contract::HttpErrorContract;
pub use envelope::{
    ErrorBody,
    FieldError,
    ResponseEnvelope,
    ValidationDetails,
};
pub use error::BoundaryError;
pub use http_content_negotiation::{
    InvalidMediaType,
    MediaType,
};
pub use respond::{
    BoundaryRejection,
    render_error,
};
pub use table::{
    InvalidResponseTable,
    Negotiated,
    ResponseMode,
    ResponseRepresentation,
    ResponseTable,
};
