use std::path::Path;
use std::sync::Arc;
use std::{io, ptr};

use log::{debug, error, info, trace};
use tokio::fs::File;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::listener::Listener;
use crate::stream::{copy, SplitStream};
use crate::util::report_data_err;
use crate::{ping_gpg_agent, SocketType};

struct AgentMeta {
    path: Option<String>,
    args: Option<(u16, [u8; 16])>,
}

pub async fn bridge_to_stream<L>(mut listener: L, to_path: Option<String>) -> io::Result<()>
where
    L: Listener,
    L::Connection: SplitStream + Send + 'static,
{
    info!("bridge to stream");
    let meta = Arc::new(Mutex::new(AgentMeta {
        path: to_path,
        args: None,
    }));
    loop {
        let conn = listener.accept().await?;
        info!("connection accepted");

        let meta = meta.clone();
        let (port, nounce) = {
            let mut m = meta.lock().await;
            if m.args.is_none() {
                if m.path.is_none() {
                    m.path = Some(SocketType::Extra.try_get_path().await?);
                }
                m.args = Some(load_port_nounce(m.path.as_ref().unwrap()).await?);
            }
            m.args.unwrap()
        };

        tokio::spawn(async move {
            if let Err(e) = delegate(conn, port, nounce).await {
                error!("failed to delegate stream: {:?}", e);
                meta.lock().await.args.take();
            }
        });
    }
}

async fn delegate(mut from: impl SplitStream, to_port: u16, nounce: [u8; 16]) -> io::Result<()> {
    let mut delegate = match TcpStream::connect(("127.0.0.1", to_port)).await {
        Ok(s) => s,
        Err(e) => {
            // It's possible that gpg-client was killed and leave stale meta untouched.
            // Reping agent to make it startup.
            let _ = ping_gpg_agent().await;
            return Err(e);
        }
    };
    trace!("--> {:?}", String::from_utf8_lossy(&nounce));
    delegate.write_all(&nounce).await?;
    delegate.flush().await?;

    let (mut source_read, mut source_write) = from.split_rw();
    let (mut target_read, mut target_write) = delegate.split_rw();
    let s2t = copy("-->", &mut source_read, &mut target_write);
    let t2s = copy("<--", &mut target_read, &mut source_write);
    let (received, replied) = tokio::join!(s2t, t2s);
    debug!(
        "connection finished, received {}, replied {}",
        received?, replied?
    );
    Ok(())
}

fn load_cygwin_port_nounce(buffer: &[u8]) -> io::Result<(u16, [u8; 16])> {
    // "%u %c %08x-%08x-%08x-%08x\x00"
    let find = |buffer: &[u8], start_pos: usize, delimeter| {
        if buffer.len() <= start_pos {
            return Err(report_data_err("buffer to short"));
        }
        match buffer[start_pos..].iter().position(|c| *c == delimeter) {
            Some(pos) => Ok(pos),
            None => Err(report_data_err("wrong data format")),
        }
    };
    let parse = |buffer: &[u8], radix: u32| match std::str::from_utf8(buffer) {
        Ok(s) => match u32::from_str_radix(s, radix) {
            Ok(v) => Ok(v),
            Err(e) => Err(report_data_err(e)),
        },
        Err(e) => Err(report_data_err(e)),
    };

    let end_pos = find(buffer, 0, b' ')?;
    let port = parse(&buffer[..end_pos], 10)?;

    if (1..=65535).contains(&port)
        || !buffer[end_pos..].starts_with(b" s ")
        || buffer.len() < end_pos + 3 + 35 + 1
    {
        return Err(report_data_err("wrong data format"));
    }

    let mut start_pos = end_pos + 3;
    let mut nounce = [0u32; 4];
    for (pos, n) in nounce.iter_mut().enumerate() {
        *n = parse(&buffer[start_pos..start_pos + 4], 16)?;
        if pos < 3 {
            if buffer[start_pos + 4] != b'-' {
                return Err(report_data_err("wrong data format"));
            }
        } else if buffer[start_pos + 4] != b'x' {
            return Err(report_data_err("wrong data format"));
        }
        start_pos += 5;
    }
    // It's on purpose to ignore endianess.
    Ok((port as u16, unsafe { std::mem::transmute(nounce) }))
}

async fn load_port_nounce(path: &str) -> io::Result<(u16, [u8; 16])> {
    if !Path::new(&path).exists() {
        ping_gpg_agent().await?;
    }
    let mut f = File::open(&path.replace('\\', "/")).await?;
    let mut buffer = Vec::with_capacity(50);
    f.read_to_end(&mut buffer).await?;
    if buffer.starts_with(b"!<socket >") {
        return load_cygwin_port_nounce(&buffer[10..]);
    }
    let (left, right) = buffer.split_at(buffer.len() - 16);
    let to_port: u16 = std::str::from_utf8(left).unwrap().trim().parse().unwrap();
    let mut nounce = [0; 16];
    unsafe {
        ptr::copy_nonoverlapping(right.as_ptr(), nounce.as_mut_ptr(), 16);
    }
    Ok((to_port, nounce))
}
