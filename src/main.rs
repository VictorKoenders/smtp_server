#![feature(async_await, await_macro, proc_macro_hygiene)]
#![allow(warnings)]

mod collector;
mod config;
mod connection;
mod message_parser;

pub use crate::collector::Collector;
pub use crate::config::{Config, ConfigFeature};
pub use crate::connection::{Connection, State};
use futures::{
    compat::{AsyncRead01CompatExt, AsyncWrite01CompatExt, Stream01CompatExt},
    future::{FutureExt, TryFutureExt},
    io::{AsyncReadExt, AsyncWriteExt},
    stream::StreamExt,
};
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::io::Read;
use std::sync::Arc;
use tokio::io::AsyncRead;
use tokio_tcp::{TcpListener, TcpStream};

fn main() {
    let future03 = futures::future::lazy(|_| {
        let mut file = std::fs::File::open("identity.pfx").expect("Could not load identity.pfx");
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .expect("Could not read identity.pfx");
        drop(file);

        let acceptor = native_tls::TlsAcceptor::new(
            native_tls::Identity::from_pkcs12(&contents, "").expect("Could not parse identity.pfx"),
        )
        .expect("Could not create a TLS Acceptor");
        let acceptor = tokio_tls::TlsAcceptor::from(acceptor);

        let config = Arc::new(RwLock::new(Config {
            host: String::from("localhost"),
            tls_acceptor: acceptor,
            features: vec![
                ConfigFeature::Tls,
                ConfigFeature::Auth(String::from("TEXT PLAIN")),
            ],
        }));

        let collector = Collector::spawn();
        tokio::spawn(
            start_on_port(25, collector.clone(), config.clone())
                .map_err(|e| {
                    eprintln!("Start failed: {:?}", e);
                })
                .boxed()
                .compat(),
        );
        tokio::spawn(
            start_on_port(587, collector.clone(), config.clone())
                .map_err(|e| {
                    eprintln!("Start failed: {:?}", e);
                })
                .boxed()
                .compat(),
        );
    });
    tokio::run(future03.unit_error().compat());
}

async fn start_on_port(
    port: u16,
    collector: Collector,
    config: Arc<RwLock<Config>>,
) -> Result<(), failure::Error> {
    println!("Listening on port {:?}", port);
    let srv = TcpListener::bind(&([0u8, 0, 0, 0], port).into())?;
    let mut stream = srv.incoming().compat();

    while let Some(client) = stream.next().await {
        let client = match client {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Could not accept client: {:?}", e);
                break;
            }
        };
        let peer_addr = client.peer_addr();
        println!("Received client {:?} on port {}", peer_addr, port);
        crate::connection::Connection::spawn(client, collector.clone(), config.clone());
    }

    Ok(())
}
