use tokio::net::TcpStream;

use super::{PinAsyncRead, PinAsyncWrite, SplitStream};

impl SplitStream for TcpStream {
    #[inline]
    fn split_rw(&mut self) -> (PinAsyncRead, PinAsyncWrite) {
        let (read_half, write_half) = TcpStream::split(self);
        (Box::pin(read_half), Box::pin(write_half))
    }
}
