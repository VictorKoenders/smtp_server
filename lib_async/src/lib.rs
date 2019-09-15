#![cfg_attr(debug_assertions, allow(clippy::never_loop))]

mod config;
mod connection;
mod flow;
mod smtp;

use self::config::{Capability, Config};
use self::connection::Connection;
use self::flow::Flow;
use async_trait::async_trait;
use bytes::BytesMut;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

pub struct SmtpServer<H: Handler> {
    listener: TcpListener,
    handler: H,
    config: Config,
}

#[async_trait]
pub trait Handler: Send + Sync + Clone + 'static {
    async fn validate_address(&self, email_address: &str) -> bool;
}

impl<H: Handler> SmtpServer<H> {
    pub async fn create<A: tokio_net::ToSocketAddrs>(
        addrs: A,
        handler: H,
        config: Config,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addrs).await?;
        Ok(SmtpServer {
            listener,
            handler,
            config,
        })
    }

    pub async fn run(mut self) -> std::io::Result<()> {
        loop {
            let (socket, addr) = self.listener.accept().await?;
            let handler = self.handler.clone();
            let config = self.config.clone();
            tokio::spawn(async move { process(socket, addr, config, handler).await });
        }
    }
}

async fn process<H: Handler>(
    mut socket: TcpStream,
    addr: std::net::SocketAddr,
    config: Config,
    _handler: H,
) {
    println!("[{}] Connected", addr);
    if let Err(e) = socket
        .write_all(b"220 smtp.server.com Simple Mail Transfer Service Ready\r\n")
        .await
    {
        eprintln!("Can not send initial message to the client: {:?}", e);
        return;
    }
    let mut state = Connection::new(config.clone());
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

        match state.data_received(&mut bytes) {
            Ok(Some(Flow::Reply(msg))) => {
                let string = format!("220 {}\r\n", msg);
                if let Err(e) = socket.write_all(string.as_bytes()).await {
                    eprintln!("[{}] Could not send message to client: {:?}", addr, e);
                    break 'outer;
                }
            }
            Ok(Some(Flow::ReplyWithCode(code, msg))) => {
                let string = format!("{} {}\r\n", code, msg);
                if let Err(e) = socket.write_all(string.as_bytes()).await {
                    eprintln!("[{}] Could not send message to client: {:?}", addr, e);
                    break 'outer;
                }
            }
            Ok(Some(Flow::Silent)) | Ok(None) => {}
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
