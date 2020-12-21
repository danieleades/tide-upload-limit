#![deny(clippy::all, unsafe_code)]
#![warn(clippy::pedantic)]

use futures_io::AsyncRead;
use pin_project::pin_project;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

pub trait Callback: Fn(Result<(), Error>) + Send + Sync + 'static {}

impl<F> Callback for F where F: Fn(Result<(), Error>) + Send + Sync + 'static {}

#[pin_project]
pub struct AsyncReadLimit<Reader>
where
    Reader: AsyncRead,
{
    /// The wrapped [`AsyncRead`]er
    #[pin]
    inner: Reader,

    /// Current length accumulator
    current_length: usize,

    /// The configured maximum length
    max_length: usize,

    /// Optional callback for when the stream has finished being read
    callback: Option<Arc<dyn Callback>>,
}

impl<Reader> AsyncReadLimit<Reader>
where
    Reader: AsyncRead,
{
    pub fn new(max_length: usize, inner: Reader) -> Self {
        let current_length = 0;

        Self {
            inner,
            current_length,
            max_length,
            callback: None,
        }
    }

    /// Optionally set a callback which fires when the stream is fully read.
    ///
    /// The callback must be a function which accepts [`Result<(), Error>`].
    /// `()` is returned if the stream is read successfully, and [`Error`] is
    /// returned if the maximum length is exceeded.
    ///
    /// # Example
    ///
    /// ```
    /// use async_read_limit::AsyncReadLimit;
    /// use std::sync::{
    ///     atomic::{AtomicBool, Ordering},
    ///     Arc,
    /// };
    ///
    /// let async_reader = "some string".as_bytes();
    /// let max_length = 1024 * 1024 * 4;
    ///
    /// let payload_too_large = Arc::new(AtomicBool::new(false));
    /// let payload_too_large_clone = Arc::clone(&payload_too_large);
    ///
    /// let upload_limiter =
    ///     AsyncReadLimit::new(max_length, async_reader).with_callback(move |result: Result<_, _>| {
    ///         if result.is_err() {
    ///             payload_too_large_clone.store(true, Ordering::SeqCst)
    ///         }
    ///     });
    /// ```
    pub fn with_callback<F: Callback>(mut self, cb: F) -> Self {
        self.callback = Some(Arc::new(cb));
        self
    }
}

impl<Reader> AsyncRead for AsyncReadLimit<Reader>
where
    Reader: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<futures_io::Result<usize>> {
        let this = self.project();

        let result = this.inner.poll_read(cx, buf);

        match result {
            Poll::Ready(Ok(0)) => handle_eof(this.callback),
            Poll::Ready(Ok(bytes)) => {
                handle_ok(this.current_length, *this.max_length, bytes, this.callback)
            }
            x => x,
        }
    }
}

fn handle_eof(callback: &Option<Arc<dyn Callback>>) -> Poll<Result<usize, futures_io::Error>> {
    if let Some(cb) = callback {
        (cb)(Ok(()))
    }
    Poll::Ready(Ok(0))
}

fn handle_ok(
    current_length: &mut usize,
    max_length: usize,
    bytes: usize,
    callback: &Option<Arc<dyn Callback>>,
) -> Poll<Result<usize, futures_io::Error>> {
    *current_length += bytes;

    Poll::Ready(match check_under_maximum(*current_length, max_length) {
        Ok(()) => Ok(bytes),
        Err(e) => {
            if let Some(cb) = callback {
                (cb)(Err(e));
            }
            Err(e.into())
        }
    })
}

fn check_under_maximum(current_length: usize, max_length: usize) -> Result<(), Error> {
    if current_length > max_length {
        Err(Error::new(max_length))
    } else {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error, Clone, Copy)]
#[error("payload is larger than configured maximum (>{maximum_length} bytes)")]
pub struct Error {
    maximum_length: usize,
}

impl Error {
    fn new(maximum_length: usize) -> Self {
        Self { maximum_length }
    }
}

impl From<Error> for futures_io::Error {
    fn from(e: Error) -> Self {
        futures_io::Error::new(futures_io::ErrorKind::InvalidData, e.to_string())
    }
}

#[cfg(test)]
mod tests {

    use super::AsyncReadLimit;
    use futures_util::io::AsyncReadExt;
    use test_case::test_case;

    #[test_case("test string", 2 => panics "payload is larger than configured maximum (>2 bytes)" ; "when payload is larger than maximum")]
    #[test_case("test string", 128 ; "when payload is less than the maximum")]
    #[async_std::test]
    async fn max_value(payload: &str, max_length: usize) {
        let mut bytes_sniffer = AsyncReadLimit::new(max_length, payload.as_bytes());

        let mut output = Vec::new();

        bytes_sniffer.read_to_end(&mut output).await.unwrap();
    }
}
