#![deny(clippy::indexing_slicing)]

mod config;
pub mod connection;
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
use tokio_tls::TlsStream;

type IsTls = bool;

pub struct SmtpServer {
    listeners: Vec<(TcpListener, IsTls)>,
    handler: Box<dyn Handler>,
    config: Config,
}

impl SmtpServer {
    pub fn create(handler: impl Handler, config: Config) -> Self {
        SmtpServer {
            listeners: Vec::new(),
            handler: Box::new(handler),
            config,
        }
    }

    pub async fn register_listener<A: tokio_net::ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("Listening on {}", listener.local_addr().unwrap());
        self.listeners.push((listener, false));
        Ok(())
    }

    pub async fn register_tls_listener<A: tokio_net::ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> std::io::Result<()> {
        if !self.config.has_capability(Capability::StartTls) {
            panic!("Can not register TLS listener when TLS is not configured\nTry calling ConfigBuilder::default().with_pkcs12_certificate(..)");
        }
        let listener = TcpListener::bind(addr).await?;
        println!("Listening on {} (TLS)", listener.local_addr().unwrap());
        self.listeners.push((listener, true));
        Ok(())
    }

    pub async fn run(mut self) {
        let (first_listener, first_is_tls) = self.listeners.pop().unwrap();
        for (listener, is_tls) in self.listeners {
            let config = self.config.clone();
            let handler = self.handler.clone_box();
            tokio::spawn(async move {
                blocking_read(listener, is_tls, config, handler).await;
            });
        }
        blocking_read(first_listener, first_is_tls, self.config, self.handler).await;
    }
}

async fn blocking_read(
    mut listener: TcpListener,
    is_tls: bool,
    config: Config,
    handler: Box<dyn Handler>,
) {
    loop {
        let (socket, addr) = listener.accept().await.expect("Tcp listener crashed");
        let state = Connection::new(config.clone(), handler.clone_box());
        if is_tls {
            tokio::spawn(async move { process_tls(socket, addr, state).await });
        } else {
            tokio::spawn(async move { process(socket, addr, state).await });
        }
    }
}

async fn process(mut socket: TcpStream, addr: std::net::SocketAddr, mut state: Connection) {
    println!("[{}] Connected", addr);
    let connection_message = state.new_connection_message();
    println!("OUT {}", connection_message.trim());
    if let Err(e) = socket.write_all(connection_message.as_bytes()).await {
        eprintln!("Can not send initial message to the client: {:?}", e);
        return;
    }
    let upgrade_tls = process_impl(&mut socket, addr, &mut state, false).await;
    if upgrade_tls {
        println!("OUT 220 Go ahead");
        if let Err(e) = socket.write_all(b"220 Go ahead\r\n").await {
            eprintln!("[{}] Could not upgrade TLS, {:?}", addr, e);
        } else {
            match state.upgrade_tls(socket).await {
                Ok(tls_stream) => process_tls_inner(tls_stream, addr, state, true).await,
                Err(e) => {
                    eprintln!("[{}] Failed TLS handshake, disconnecting", addr);
                    eprintln!("[{}] {:?}", addr, e);
                }
            }
        }
    }
}

async fn process_tls(mut socket: TcpStream, addr: std::net::SocketAddr, state: Connection) {
    println!("[{}] Connected", addr);
    let connection_message = state.new_connection_message();
    println!("OUT {}", connection_message.trim());
    if let Err(e) = socket.write_all(connection_message.as_bytes()).await {
        eprintln!("Can not send initial message to the client: {:?}", e);
        return;
    }
    let result = state.upgrade_tls(socket).await;
    match result {
        Ok(tls_stream) => process_tls_inner(tls_stream, addr, state, true).await,
        Err(e) => {
            eprintln!("[{}] Failed TLS handshake, disconnecting", addr);
            eprintln!("[{}] {:?}", addr, e);
        }
    }
}

async fn process_tls_inner(
    mut socket: TlsStream<TcpStream>,
    addr: std::net::SocketAddr,
    mut state: Connection,
    is_upgrade: bool,
) {
    if is_upgrade {
        println!("[{}] TLS upgrade", addr);
    } else {
        println!("[{}] Connected with TLS", addr);
    }
    let upgrade_tls = process_impl(&mut socket, addr, &mut state, true).await;
    if upgrade_tls {
        eprintln!(
            "[{}] Tried to upgrade a TLS connection while already upgraded",
            addr
        );
        eprintln!("[{}] This is a bug, please report this", addr);
    }
}

type ShouldUpgrade = bool;
async fn process_impl<SOCK>(
    socket: &mut SOCK,
    addr: std::net::SocketAddr,
    state: &mut Connection,
    is_tls: bool,
) -> ShouldUpgrade
where
    SOCK: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let mut bytes = BytesMut::new();
    'outer: loop {
        let mut buffer = [0u8; 1024];
        match socket.read(&mut buffer).await {
            Ok(0) => {
                println!("[{}] Client disconnected", addr);
                break 'outer;
            }
            Ok(n) => {
                #[allow(clippy::indexing_slicing)]
                // Safe because this is the length returned from `socket.read`
                bytes.extend_from_slice(&buffer[..n]);
            }
            Err(e) => {
                eprintln!("[{}] Client error: {:?}", addr, e);
                break 'outer;
            }
        }

        let mut result = state.data_received(&mut bytes, is_tls);
        if let Ok(Some(Flow::EmailReceived {
            sender,
            recipient,
            body,
        })) = result
        {
            result = Ok(Some(
                match state.try_send_email(sender, recipient, body).await {
                    Ok(()) => {
                        Flow::Reply(Flow::status_ok(), "Email received, over and out!".into())
                    }
                    Err(e) => Flow::Reply(Flow::status_err(), format!("{:?}", e).into()),
                },
            ));
        }

        match result {
            Ok(Some(Flow::Reply(code, msg))) => {
                let string = format!("{} {}\r\n", code, msg);
                println!("OUT: {}", string.trim());
                if let Err(e) = socket.write_all(string.as_bytes()).await {
                    eprintln!("[{}] Could not send message to client: {:?}", addr, e);
                    break 'outer;
                }
            }
            Ok(Some(Flow::ReplyMultiline(code, lines))) => {
                for (index, line) in lines.iter().enumerate() {
                    let is_last = index + 1 == lines.len();

                    println!("OUT: {}{}{}", code, if is_last { ' ' } else { '-' }, line);
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
            Ok(Some(Flow::EmailReceived { .. })) => {
                eprintln!("Received Flow::EmailReceived, but this should be handled by state.try_send_email");
                eprintln!("This is a bug! Please report this");
                panic!();
            }
            Ok(Some(Flow::UpgradeTls)) => {
                return true;
            }
            Err(e) => {
                eprintln!("[{}] Client state error: {:?}", addr, e);
                let mut str = format!("500 {:?}", e).replace("\n", ", ").replace("\r", "");
                println!("OUT {:?}", str);
                str += "\r\n";
                if let Err(e) = socket.write_all(str.as_bytes()).await {
                    eprintln!("[{}] Could not send message to client: {:?}", addr, e);
                    break 'outer;
                }
            }
        }
    }
    false
}
