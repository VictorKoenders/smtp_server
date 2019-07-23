use futures::io::AsyncRead;
use futures::stream::Stream;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct LineReader<R: AsyncRead + Unpin> {
    pub reader: R,
    buffer: VecDeque<u8>,
}

impl<R: AsyncRead + Unpin> LineReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Default::default(),
        }
    }
}

impl<R: AsyncRead + Unpin> Stream for LineReader<R> {
    type Item = std::result::Result<String, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this: &mut Self = Pin::into_inner(self);
        let mut did_read = false;
        let mut buffer = [0u8; 1024];

        loop {
            if let Some(idx) = this.buffer.iter().position(|b| b == &b'\n') {
                let mut line = this.buffer.drain(..=idx).take(idx).collect::<Vec<u8>>();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                let line = String::from_utf8_lossy(&line).into_owned();
                return Poll::Ready(Some(Ok(line)));
            }

            let reader_pin: Pin<&mut R> = Pin::new(&mut this.reader);
            match reader_pin.poll_read(cx, &mut buffer) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(l)) if l == 0 => {
                    if did_read {
                        return Poll::Pending;
                    } else {
                        return Poll::Ready(Some(Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "Received 0 bytes",
                        ))));
                    }
                }
                Poll::Ready(Ok(l)) => {
                    this.buffer.extend(&buffer[..l]);
                    did_read = true;
                }
                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Some(Err(e)));
                }
            }
        }
    }
}
