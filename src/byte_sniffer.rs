use futures_util::io::AsyncRead;
use pin_project::pin_project;
use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

#[pin_project]
pub(crate) struct ByteSniffer<Reader>
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

    upload_clamped: Option<Arc<AtomicBool>>,
}

impl<Reader> ByteSniffer<Reader>
where
    Reader: AsyncRead,
{
    pub fn new(max_length: usize, inner: Reader) -> Self {
        let current_length = 0;

        Self {
            inner,
            current_length,
            max_length,
            upload_clamped: None,
        }
    }

    pub fn with_callback(mut self, flag: Arc<AtomicBool>) -> Self {
        self.upload_clamped = Some(flag);
        self
    }
}

impl<Reader> AsyncRead for ByteSniffer<Reader>
where
    Reader: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<futures_util::io::Result<usize>> {
        let this = self.project();

        let result = this.inner.poll_read(cx, buf);

        match result {
            Poll::Ready(Ok(bytes)) => handle_ok(
                this.current_length,
                *this.max_length,
                bytes,
                this.upload_clamped,
            ),
            x => x,
        }
    }
}

fn handle_ok(
    current_length: &mut usize,
    max_length: usize,
    bytes: usize,
    upload_clamped: &Option<Arc<AtomicBool>>,
) -> Poll<Result<usize, futures_util::io::Error>> {
    *current_length += bytes;

    Poll::Ready(match check_under_maximum(*current_length, max_length) {
        Ok(()) => Ok(bytes),
        Err(e) => {
            if let Some(b) = upload_clamped {
                b.store(true, Ordering::Relaxed)
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

#[derive(Debug, thiserror::Error)]
#[error("payload is larger than configured maximum (>{maximum_length} bytes)")]
pub(crate) struct Error {
    maximum_length: usize,
}

impl Error {
    fn new(maximum_length: usize) -> Self {
        Self { maximum_length }
    }
}

impl From<Error> for futures_util::io::Error {
    fn from(e: Error) -> Self {
        futures_util::io::Error::new(futures_util::io::ErrorKind::InvalidData, e.to_string())
    }
}

#[cfg(test)]
mod tests {

    use super::ByteSniffer;
    use futures_util::io::AsyncReadExt;
    use test_case::test_case;

    #[test_case("test string", 2 => panics "payload is larger than configured maximum (>2 bytes)" ; "when payload is larger than maximum")]
    #[test_case("test string", 128 ; "when payload is less than the maximum")]
    #[async_std::test]
    async fn max_value(payload: &str, max_length: usize) {
        let mut bytes_sniffer = ByteSniffer::new(max_length, payload.as_bytes());

        let mut output = Vec::new();

        bytes_sniffer.read_to_end(&mut output).await.unwrap();
    }
}
