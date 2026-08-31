#![doc = include_str!("../README.md")]

mod body;
mod contract;
mod envelope;
mod error;
mod media;
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
pub use media::{
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
