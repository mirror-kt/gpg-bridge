mod cli;

use std::{os::windows::process::CommandExt, process::Command};

use clap::Parser as _;
use gpg_bridge::{util::other_error, SocketType};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();
    let args = cli::Args::parse();

    let ssh_bridge = args.ssh.map(|addr| (addr, None));
    let extra_bridge = args.extra.map(|addr| (addr, args.extra_socket));

    if args.detach {
        gpg_bridge::ping_gpg_agent().await?;

        let mut args = std::env::args();
        let mut cmd = Command::new(args.next().unwrap());
        for arg in args {
            if arg != "-d" && arg != "--detach" {
                cmd.arg(arg);
            }
        }

        return cmd
            .creation_flags(0x0000_0200 | 0x0000_0008 | 0x0400_0000)
            .spawn()
            .map(|_| ());
    }

    let ssh_task = async move {
        if let Some((addr, socket)) = ssh_bridge {
            log::info!("ssh bridge start");
            return gpg_bridge::bridge(SocketType::Ssh, addr, socket).await;
        }
        log::info!("ssh bridge disabled");
        Ok(())
    };
    let extra_task = async move {
        if let Some((addr, socket)) = extra_bridge {
            log::info!("extra bridge start");
            return gpg_bridge::bridge(SocketType::Extra, addr, socket).await;
        }
        log::info!("extra bridge disabled");
        Ok(())
    };

    match tokio::try_join!(ssh_task, extra_task) {
        Ok(_) => Ok(()),
        Err(e) => Err(other_error(format!("failed to join tasks {:?}", e))),
    }
}
