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
            Poll::Ready(Ok(0)) => {
                if this.length < this.max_length {
                    Poll::Ready(Err(futures_util::io::Error::new(
                        futures_util::io::ErrorKind::InvalidData,
                        format!(
                            "payload is smaller than expected ({} < {})",
                            this.length, this.max_length
                        ),
                    )))
                } else {
                    Poll::Ready(Ok(0))
                }
            }
            Poll::Ready(Ok(bytes)) => {
                *this.length += bytes;
                if this.length > this.max_length {
                    Poll::Ready(Err(futures_util::io::Error::new(
                        futures_util::io::ErrorKind::InvalidData,
                        format!("payload is too large (>{} bytes)", this.max_length),
                    )))
                } else {
                    Poll::Ready(Ok(bytes))
                }
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context,
        bufs: &mut [futures_util::io::IoSliceMut<'_>],
    ) -> Poll<futures_util::io::Result<usize>> {
        //todo reimplement this to delegate to `self::inner`s implementation

        for b in bufs {
            if !b.is_empty() {
                return self.poll_read(cx, b);
            }
        }

        self.poll_read(cx, &mut [])
    }
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
