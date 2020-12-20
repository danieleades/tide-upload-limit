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
    #[pin]
    inner: Reader,
    length: usize,
    max_length: usize,
}

impl<Reader> ByteSniffer<Reader>
where
    Reader: AsyncRead,
{
    pub fn new(max_length: usize, inner: Reader) -> Self {
        let length = 0;

        Self {
            inner,
            length,
            max_length,
        }
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
            Poll::Ready(Ok(0)) => handle_eof(*this.length, *this.max_length),
            Poll::Ready(Ok(bytes)) => handle_ok(bytes, this.length, this.max_length),
            x => x,
        }
    }
}

fn handle_eof(length: usize, max_length: usize) -> Poll<Result<usize, futures_util::io::Error>> {
    let result = if length < max_length {
        Err(futures_util::io::Error::new(
            futures_util::io::ErrorKind::InvalidData,
            format!(
                "payload is smaller than expected ({} < {})",
                length, max_length
            ),
        ))
    } else {
        Ok(0)
    };

    Poll::Ready(result)
}

fn handle_ok(
    bytes: usize,
    length: &mut usize,
    max_length: &mut usize,
) -> Poll<Result<usize, futures_util::io::Error>> {
    *length += bytes;

    let result = if length > max_length {
        Err(futures_util::io::Error::new(
            futures_util::io::ErrorKind::InvalidData,
            format!("payload is too large (>{} bytes)", max_length),
        ))
    } else {
        Ok(bytes)
    };

    Poll::Ready(result)
}

#[cfg(test)]
mod tests {

    use super::ByteSniffer;
    use futures_util::io::AsyncReadExt;
    use test_case::test_case;

    #[test_case("test string", 2 => panics "payload is too large (>2 bytes)" ; "when payload is larger than maximum")]
    #[test_case("test string", 11 ; "when payload matches maximum")]
    #[test_case("test string", 128 => panics "payload is smaller than expected (11 < 128)" ; "when payload is smaller than maximum")]
    #[async_std::test]
    async fn sniff_test(payload: &str, max_length: usize) {
        let mut bytes_sniffer = ByteSniffer::new(max_length, payload.as_bytes());

        let mut output = Vec::new();

        bytes_sniffer.read_to_end(&mut output).await.unwrap();
    }
}
