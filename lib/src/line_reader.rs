use futures::io::{AsyncRead, AsyncWrite};
use futures::ready;
use futures::Stream;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct LineReader<R: AsyncRead + AsyncWrite + Unpin> {
    inner: R,
    max_size: usize,
    read_buffer: VecDeque<u8>,
    write_buffer: VecDeque<u8>,
}

impl<R: AsyncRead + AsyncWrite + Unpin> LineReader<R> {
    pub fn new(inner: R, max_size: usize) -> Self {
        Self {
            inner,
            max_size,
            read_buffer: Default::default(),
            write_buffer: Default::default(),
        }
    }
}

impl<R: AsyncRead + AsyncWrite + Unpin> LineReader<R> {
    pin_utils::unsafe_pinned!(inner: R);
    pin_utils::unsafe_unpinned!(write_buffer: VecDeque<u8>);

    fn project<'a>(self: Pin<&'a mut Self>) -> (Pin<&'a mut R>, &'a mut VecDeque<u8>) {
        unsafe {
            let this = self.get_unchecked_mut();
            (Pin::new_unchecked(&mut this.inner), &mut this.write_buffer)
        }
    }

    fn poll_flush_buffer(
        self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Result<(), std::io::Error>> {
        let (mut inner, buffer) = self.project();
        while !buffer.is_empty() {
            let slices = buffer.as_slices();
            let mut written = ready!(inner.as_mut().poll_write(cx, slices.0))?;
            written += ready!(inner.as_mut().poll_write(cx, slices.1))?;
            buffer.drain(..written);
        }
        Poll::Ready(Ok(()))
    }
}

impl<R: AsyncRead + AsyncWrite + Unpin> Stream for LineReader<R> {
    type Item = std::result::Result<String, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this: &mut Self = Pin::into_inner(self);
        let mut did_read = false;
        let mut buffer = [0u8; 1024];

        loop {
            if let Some(idx) = this.read_buffer.iter().position(|b| b == &b'\n') {
                let mut line = this
                    .read_buffer
                    .drain(..=idx)
                    .take(idx)
                    .collect::<Vec<u8>>();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                let line = String::from_utf8_lossy(&line).into_owned();
                return Poll::Ready(Some(Ok(line)));
            }

            let reader_pin: Pin<&mut R> = Pin::new(&mut this.inner);
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
                    this.read_buffer.extend(&buffer[..l]);
                    if this.read_buffer.len() > this.max_size {
                        return Poll::Ready(Some(Err(std::io::Error::new(
                            std::io::ErrorKind::Interrupted,
                            "Too much data received",
                        ))));
                    }
                    did_read = true;
                }
                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Some(Err(e)));
                }
            }
        }
    }
}

impl<'a, R: AsyncRead + AsyncWrite + Unpin> futures::Sink<Vec<u8>> for LineReader<R> {
    type Error = std::io::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_flush_buffer(cx))?;
        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, item: Vec<u8>) -> Result<(), Self::Error> {
        let buffer = self.as_mut().write_buffer();
        buffer.reserve(item.len());
        for i in item {
            buffer.push_back(i);
        }
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_flush_buffer(cx))?;
        ready!(self.as_mut().inner().poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_flush_buffer(cx))?;
        ready!(self.as_mut().inner().poll_close(cx))?;
        Poll::Ready(Ok(()))
    }
}
