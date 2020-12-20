use futures_util::io::AsyncRead;
use pin_project::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

#[pin_project]
#[derive(Debug)]
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

    /// The reported length of the payload (if provided)
    expected_length: Option<usize>,
}

impl<Reader> ByteSniffer<Reader>
where
    Reader: AsyncRead,
{
    pub fn new(max_length: usize, inner: Reader, expected_length: Option<usize>) -> Self {
        let current_length = 0;

        Self {
            inner,
            current_length,
            max_length,
            expected_length,
        }
    }
}

/// Helper functions for [`AsyncRead`] implementation
impl<Reader> ByteSniffer<Reader> where Reader: AsyncRead {}

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
            Poll::Ready(Ok(0)) => handle_eof(*this.current_length, *this.expected_length),
            Poll::Ready(Ok(bytes)) => handle_ok(
                this.current_length,
                *this.max_length,
                *this.expected_length,
                bytes,
            ),
            x => x,
        }
    }
}

fn handle_eof(
    current_length: usize,
    expected_length: Option<usize>,
) -> Poll<Result<usize, futures_util::io::Error>> {
    Poll::Ready(if let Some(expected_length) = expected_length {
        if current_length < expected_length {
            Err(futures_util::io::Error::new(
                futures_util::io::ErrorKind::InvalidData,
                format!(
                    "payload is smaller than expected ({} < {})",
                    current_length, expected_length
                ),
            ))
        } else {
            Ok(0)
        }
    } else {
        Ok(0)
    })
}

fn handle_ok(
    current_length: &mut usize,
    max_length: usize,
    expected_length: Option<usize>,
    bytes: usize,
) -> Poll<Result<usize, futures_util::io::Error>> {
    *current_length += bytes;

    check_under_maximum(*current_length, max_length)
        .and(check_under_expected(*current_length, expected_length))?;

    Poll::Ready(Ok(bytes))
}

fn check_under_maximum(
    current_length: usize,
    max_length: usize,
) -> Result<(), futures_util::io::Error> {
    if current_length > max_length {
        Err(futures_util::io::Error::new(
            futures_util::io::ErrorKind::InvalidData,
            format!(
                "payload is larger than configured maximum (>{} bytes)",
                max_length
            ),
        ))
    } else {
        Ok(())
    }
}

fn check_under_expected(
    current_length: usize,
    expected_length: Option<usize>,
) -> Result<(), futures_util::io::Error> {
    if let Some(expected_length) = expected_length {
        if current_length > expected_length {
            return Err(futures_util::io::Error::new(
                futures_util::io::ErrorKind::InvalidData,
                format!(
                    "payload is larger than expected (>{} bytes)",
                    expected_length
                ),
            ));
        }
    }

    Ok(())
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
        let length = Some(payload.len());
        let mut bytes_sniffer = ByteSniffer::new(max_length, payload.as_bytes(), length);

        let mut output = Vec::new();

        bytes_sniffer.read_to_end(&mut output).await.unwrap();
    }

    #[test_case("test string", 2 => panics "payload is larger than expected (>2 bytes)" ; "when payload is larger than expected")]
    #[test_case("test string", 11 ; "when payload is expected size")]
    #[test_case("test string", 128 => panics "payload is smaller than expected (11 < 128)" ; "when payload is smaller than expected")]
    #[async_std::test]
    async fn expected_value(payload: &str, expected_length: usize) {
        let mut bytes_sniffer = ByteSniffer::new(1024, payload.as_bytes(), Some(expected_length));

        let mut output = Vec::new();

        bytes_sniffer.read_to_end(&mut output).await.unwrap();
    }
}
