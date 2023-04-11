pub mod named_pipe;
pub mod tcp;

use std::pin::Pin;
use std::{io, ptr};

use log::trace;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

pub type PinAsyncRead<'a> = Pin<Box<dyn AsyncRead + Send + 'a>>;
pub type PinAsyncWrite<'a> = Pin<Box<dyn AsyncWrite + Send + 'a>>;

pub trait SplitStream {
    /// Splits a TcpStream into a read half and a write half, which can be used to read and write the stream concurrently.
    fn split_rw(&mut self) -> (PinAsyncRead, PinAsyncWrite);
}

pub async fn copy<'a>(
    tag: &str,
    from: &mut Pin<Box<dyn AsyncRead + Send + 'a>>,
    to: &mut Pin<Box<dyn AsyncWrite + Send + 'a>>,
) -> io::Result<u64> {
    let mut buf = vec![0; 4096];
    let mut total = 0;
    loop {
        let cnt = from.read(&mut buf).await?;
        if cnt == 0 {
            to.shutdown().await?;
            unsafe {
                ptr::write_bytes(buf.as_mut_ptr(), 0, 4096);
            }
            return Ok(total);
        }
        total += cnt as u64;
        trace!("{} {:?}", tag, String::from_utf8_lossy(&buf[..cnt]));
        to.write_all(&buf[..cnt]).await?;
    }
}
