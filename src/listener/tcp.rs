use std::future::Future;
use std::pin::Pin;

use tokio::net::{TcpListener, TcpStream};

use super::Listener;

impl Listener for TcpListener {
    type Connection = TcpStream;

    fn accept<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Self::Connection>> + 'a>> {
        Box::pin(async move {
            let (conn, _) = TcpListener::accept(self).await?;
            Ok(conn)
        })
    }
}
