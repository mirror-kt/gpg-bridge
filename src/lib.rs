pub mod bridge;
pub mod listener;
pub mod stream;
pub mod util;

use std::io;

use tokio::net::windows::named_pipe::ServerOptions;
use tokio::net::TcpListener;
use tokio::process::Command;

use crate::bridge::extra::bridge_to_stream;
use crate::bridge::ssh::bridge_to_message;
use crate::listener::named_pipe::NamedPipeServerListener;
use crate::listener::Listener;
use crate::stream::SplitStream;

#[derive(Clone, Copy)]
pub enum SocketType {
    Ssh,
    Extra,
}

impl SocketType {
    fn name(&self) -> &'static str {
        match self {
            SocketType::Ssh => "agent-ssh-socket",
            SocketType::Extra => "agent-extra-socket",
        }
    }

    pub async fn try_get_path(&self) -> io::Result<String> {
        let output = Command::new("gpgconf")
            .arg("--list-dir")
            .arg(self.name())
            .output()
            .await?;
        if !output.status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "failed to load extra socket: {:?}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            ));
        }
        Ok(String::from_utf8(output.stdout).unwrap().trim().to_owned())
    }
}
pub async fn ping_gpg_agent() -> io::Result<()> {
    let output = Command::new("gpg-connect-agent")
        .arg("/bye")
        .output()
        .await?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "failed to start gpg-agent: {:?}",
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }
    Ok(())
}

/// A bridge that forwards all requests from certain stream to gpg-agent on Windows.
///
/// `to_path` should point to the path of gnupg UDS. `from_addr` can be either TCP address
/// or Named Pipe.
// TODO: use trait to unify access.
pub async fn bridge(ty: SocketType, from_addr: String, to_path: Option<String>) -> io::Result<()> {
    // Attempt to setup gpg-agent if it's not up yet.
    let _ = ping_gpg_agent().await;
    // We can also try to guess ':'. But then we can distinguish between named pipe localhost and
    // invalid tcp address localhost. Force check '\pipe\' can allow those address fail with clear
    // error.
    if from_addr.starts_with("\\\\.\\pipe\\") {
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&from_addr)?;
        let listener = NamedPipeServerListener::new(server, from_addr);
        bridge_listener(ty, listener, to_path).await?;
    } else {
        let listener = TcpListener::bind(&from_addr).await?;
        bridge_listener(ty, listener, to_path).await?;
    }
    Ok(())
}

async fn bridge_listener<L>(ty: SocketType, listener: L, to_path: Option<String>) -> io::Result<()>
where
    L: Listener,
    L::Connection: SplitStream + Send + 'static,
{
    match ty {
        SocketType::Extra => bridge_to_stream(listener, to_path).await?,
        SocketType::Ssh => bridge_to_message(listener).await?,
    }
    Ok(())
}
