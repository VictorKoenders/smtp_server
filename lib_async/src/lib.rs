#![cfg_attr(debug_assertions, allow(clippy::never_loop))]

mod config;
mod connection;
mod flow;
mod handler;
mod smtp;

pub use self::config::{Capability, Config, ConfigBuilder};
use self::connection::Connection;
pub use self::flow::Flow;
pub use self::handler::{Email, Handler};

use bytes::BytesMut;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

pub struct SmtpServer {
    listener: TcpListener,
    handler: Box<dyn Handler>,
    config: Config,
}

impl SmtpServer {
    pub async fn create<A: tokio_net::ToSocketAddrs>(
        addrs: A,
        handler: impl Handler,
        config: Config,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addrs).await?;
        Ok(SmtpServer {
            listener,
            handler: Box::new(handler),
            config,
        })
    }

    pub async fn run(mut self) -> std::io::Result<()> {
        loop {
            let (socket, addr) = self.listener.accept().await?;
            let state = Connection::new(self.config.clone(), self.handler.clone_box());
            tokio::spawn(async move { process(socket, addr, state).await });
        }
    }
}

async fn process(mut socket: TcpStream, addr: std::net::SocketAddr, mut state: Connection) {
    println!("[{}] Connected", addr);
    if let Err(e) = socket
        .write_all(b"220 smtp.server.com Simple Mail Transfer Service Ready\r\n")
        .await
    {
        eprintln!("Can not send initial message to the client: {:?}", e);
        return;
    }
    let mut bytes = BytesMut::new();
    'outer: loop {
        let mut buffer = [0u8; 1024];
        match socket.read(&mut buffer).await {
            Ok(0) => {
                println!("[{}] Client disconnected", addr);
                break 'outer;
            }
            Ok(n) => {
                bytes.extend_from_slice(&buffer[..n]);
            }
            Err(e) => {
                eprintln!("[{}] Client error: {:?}", addr, e);
                break 'outer;
            }
        }

        match state.data_received(&mut bytes).await {
            Ok(Some(Flow::Reply(code, msg))) => {
                let string = format!("{} {}\r\n", code, msg);
                if let Err(e) = socket.write_all(string.as_bytes()).await {
                    eprintln!("[{}] Could not send message to client: {:?}", addr, e);
                    break 'outer;
                }
            }
            Ok(Some(Flow::ReplyMultiline(code, lines))) => {
                for (index, line) in lines.iter().enumerate() {
                    let is_last = index + 1 == lines.len();

                    for msg in &[
                        code.to_string().as_str(),
                        if is_last { " " } else { "-" },
                        line,
                        "\r\n",
                    ] {
                        if let Err(e) = socket.write_all(msg.as_bytes()).await {
                            eprintln!("[{}] Could not send message to client: {:?}", addr, e);
                            break 'outer;
                        }
                    }
                }
            }
            Ok(None) => {}
            Ok(Some(Flow::Quit)) => {
                println!("[{}] Client quit", addr);
                break 'outer;
            }
            Ok(Some(Flow::UpgradeTls)) => unimplemented!(),
            Err(e) => {
                eprintln!("[{}] Client state error: {:?}", addr, e);
            }
        }
    }
}
