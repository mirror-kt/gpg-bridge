use std::pin::Pin;
use std::task::{ready, Context, Poll};

use log::trace;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::windows::named_pipe::NamedPipeServer;

use super::{PinAsyncRead, PinAsyncWrite, SplitStream};

struct PipeServerRead<'a> {
    server: &'a NamedPipeServer,
}

impl<'a> AsyncRead for PipeServerRead<'a> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        trace!("polling pipe reader");
        if let Err(e) = ready!(self.server.poll_read_ready(cx)) {
            return Poll::Ready(Err(e));
        }

        loop {
            let arr = buf.initialize_unfilled();
            match self.server.try_read(arr) {
                Ok(n) => {
                    buf.advance(n);
                    return Poll::Ready(Ok(()));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if let Err(e) = ready!(self.server.poll_read_ready(cx)) {
                        return Poll::Ready(Err(e));
                    }
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }
    }
}

struct PipeServerWrite<'a> {
    server: &'a NamedPipeServer,
}

impl<'a> AsyncWrite for PipeServerWrite<'a> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        trace!("polling pipe writer");
        if let Err(e) = ready!(self.server.poll_write_ready(cx)) {
            return Poll::Ready(Err(e));
        }
        loop {
            match self.server.try_write(buf) {
                Ok(n) => return Poll::Ready(Ok(n)),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if let Err(e) = ready!(self.server.poll_read_ready(cx)) {
                        return Poll::Ready(Err(e));
                    }
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        trace!("polling pipe flush");
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        trace!("polling pipe shutdown");
        self.poll_flush(cx)
    }
}

impl SplitStream for NamedPipeServer {
    fn split_rw(&mut self) -> (PinAsyncRead, PinAsyncWrite) {
        (
            Box::pin(PipeServerRead { server: self }),
            Box::pin(PipeServerWrite { server: self }),
        )
    }
}
