#![deny(clippy::all, unsafe_code)]
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
use futures_util::io::AsyncBufRead;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tide::{Middleware, Request, StatusCode};

use upload_limit::ByteSniffer;

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

/// Request body payload is larger than the configured maximum
#[derive(Debug, thiserror::Error)]
#[error("payload size exceeds configured maximum ({size} > {limit})")]
pub struct Error {
    /// The size of the payload
    size: usize,

    /// The maximum payload size
    limit: usize,
}

impl Error {
    pub(crate) fn new(size: usize, limit: usize) -> Self {
        Self { size, limit }
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

        let upload_clamped = wrap_request(self.max_content_length, &mut request);

        let mut response = next.run(request).await;

        if upload_clamped.load(Ordering::Relaxed) {
            response.set_status(StatusCode::PayloadTooLarge)
        };

        Ok(response)
    }
}

/// if the length is set, and is larger than the configured maximum, then we
/// have an 'escape hatch' without requiring any further processing.
fn check_header(max_length: usize, length: Option<usize>) -> Result<(), tide::Error> {
    length.map_or(Ok(()), |len| {
        if len > max_length {
            Err(tide::Error::new(
                StatusCode::PayloadTooLarge,
                Error::new(len, max_length),
            ))
        } else {
            Ok(())
        }
    })
}

/// Wrap the request body in a byte sniffer and then reassemble the request
fn wrap_request<State>(max_length: usize, request: &mut Request<State>) -> Arc<AtomicBool> {
    let length = request.len();
    let body = request.take_body();

    let (sniffer, upload_clamped) = get_sniffer(max_length, body);

    let sniffed_reader = tide::Body::from_reader(sniffer, length);

    request.set_body(sniffed_reader);

    upload_clamped
}

/// Create a new byte 'sniffer' to count bytes as they go past
fn get_sniffer(max_length: usize, body: tide::Body) -> (impl AsyncBufRead, Arc<AtomicBool>) {
    let upload_clamped = Arc::new(AtomicBool::new(false));
    let upload_clamped_clone = Arc::clone(&upload_clamped);

    let sniffer =
        futures_util::io::BufReader::new(ByteSniffer::new(max_length, body).with_callback(
            move |result: Result<(), upload_limit::Error>| {
                if result.is_err() {
                    upload_clamped_clone.store(true, Ordering::SeqCst)
                }
            },
        ));

    (sniffer, upload_clamped)
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
