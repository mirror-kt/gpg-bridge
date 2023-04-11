use std::pin::Pin;

use std::future::Future;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

use super::Listener;

pub struct NamedPipeServerListener {
    server: NamedPipeServer,
    addr: String,
}

impl NamedPipeServerListener {
    pub fn new(server: NamedPipeServer, addr: String) -> Self {
        Self { server, addr }
    }
}

impl Listener for NamedPipeServerListener {
    type Connection = NamedPipeServer;

    fn accept<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Self::Connection>> + 'a>> {
        Box::pin(async move {
            self.server.connect().await?;
            let server = ServerOptions::new().create(&self.addr)?;
            Ok(std::mem::replace(&mut self.server, server))
        })
    }
}
