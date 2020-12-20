#![deny(clippy::all, missing_debug_implementations, unsafe_code)]
#![warn(clippy::pedantic)]

//! An upload limiting filter [`Middleware`](tide::Middleware) for ['tide']
//!
//! ```rust
//! use tide_upload_limit::UploadLimit;
//!
//! // set a global upload limit of 4kb
//! let upload_limiter = UploadLimit::new(1024 * 4);
//!
//! let mut app = tide::new();
//! app.with(upload_limiter);
//! ```
use async_trait::async_trait;
use tide::{Middleware, StatusCode};

mod byte_sniffer;
use byte_sniffer::ByteSniffer;

/// An upload limiting filter middleware for tide
#[derive(Debug)]
pub struct UploadLimit {
    max_content_length: usize,
}

impl UploadLimit {
    /// Create a new upload-limiting filter
    #[must_use]
    pub fn new(max_content_length: usize) -> Self {
        Self { max_content_length }
    }
}

/// Errors that can occur when filtering payload size
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Request body payload is larger than the configured maximum
    #[error("payload size exceeds configured maximum ({size} > {limit})")]
    PayloadTooLarge {
        /// The size of the payload
        size: usize,

        /// The maximum payload size
        limit: usize,
    },
}

impl Error {
    pub(crate) fn payload_too_large(size: usize, limit: usize) -> Self {
        Self::PayloadTooLarge { size, limit }
    }
}

#[async_trait]
impl<State> Middleware<State> for UploadLimit
where
    State: Send + Sync + Clone + 'static,
{
    async fn handle(
        &self,
        mut request: tide::Request<State>,
        next: tide::Next<'_, State>,
    ) -> tide::Result {
        let length = request.len();
        check_header(self.max_content_length, length)?;

        let body = request.take_body();

        let sniffer =
            futures_util::io::BufReader::new(ByteSniffer::new(self.max_content_length, body));

        let sniffed_reader = tide::Body::from_reader(sniffer, length);

        request.set_body(sniffed_reader);

        Ok(next.run(request).await)
    }
}

/// if the length is set, and is larger than the configured maximum, then we
/// have an 'escape hatch' without requiring any further processing.
fn check_header(max_length: usize, length: Option<usize>) -> Result<(), tide::Error> {
    length.map_or(Ok(()), |len| {
        if len > max_length {
            Err(tide::Error::new(
                StatusCode::PayloadTooLarge,
                Error::payload_too_large(len, max_length),
            ))
        } else {
            Ok(())
        }
    })
}

#[cfg(test)]
mod tests {

    use super::check_header;
    use test_case::test_case;

    #[test_case("test string", 32 ; "when content is shorter than maximum")]
    #[test_case("test string", 8 => panics "payload size exceeds configured maximum (11 > 8)" ; "when content is longer than maximum")]
    fn check_header_test(input: &str, max_length: usize) {
        let length = Some(input.len());

        check_header(max_length, length).unwrap()
    }
}
