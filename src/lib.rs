#![deny(clippy::all, missing_debug_implementations, missing_docs, unsafe_code)]
#![warn(clippy::pedantic)]

//! An upload limiting filter [`Middleware`] for ['tide']
use async_trait::async_trait;
use tide::{Middleware, StatusCode};

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
        check_header(self.max_content_length, &request)?;

        let body = request.take_body();
        // TODO: wrap the Body in a 'sniffer' and stick it back in the Request
        request.set_body(body);

        Ok(next.run(request).await)
    }
}

/// if the length is set, and is larger than the configured maximum, then we
/// have an 'escape hatch' without requiring any further processing.
fn check_header<State>(
    max_length: usize,
    request: &tide::Request<State>,
) -> Result<(), tide::Error> {
    request.len().map_or(Ok(()), |length| {
        if length > max_length {
            Err(tide::Error::new(
                StatusCode::PayloadTooLarge,
                Error::payload_too_large(length, max_length),
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

    fn build_request(input: impl Into<String>) -> tide::Request<()> {
        let mut request: tide::Request<()> = tide::http::Request::new(
            tide::http::Method::Post,
            tide::http::Url::parse("http://_").unwrap(),
        )
        .into();

        request.set_body(tide::http::Body::from_string(input.into()));

        request
    }

    #[test_case("test string", 32 ; "when content is shorter than maximum")]
    #[test_case("test string", 8 => panics "payload size exceeds configured maximum (11 > 8)" ; "when content is longer than maximum")]
    fn check_header_test(input: &str, max_length: usize) {
        let request = build_request(input);

        check_header(max_length, &request).unwrap()
    }
}
