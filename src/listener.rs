use std::future::Future;
use std::pin::Pin;

pub mod named_pipe;
pub mod tcp;

pub trait Listener {
    type Connection;

    fn accept<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Self::Connection>> + 'a>>;
}
