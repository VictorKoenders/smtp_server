#![feature(
    async_await,
    async_closure,
    await_macro,
    proc_macro_hygiene,
    pin_into_inner
)]

pub extern crate log;
pub extern crate mailparse;

#[macro_use]
mod tcp_stream_helper;
mod collector;
mod config;
mod connection;
mod line_reader;
mod message_parser;

pub use crate::collector::Email;
pub use crate::config::{Config, ConfigFeature};

use crate::collector::Collector;
use futures::{FutureExt, TryStreamExt};
use runtime::net::TcpListener;
use std::pin::Pin;

type Future<T> = Pin<Box<dyn std::future::Future<Output = T> + Send>>;

pub trait MailHandler: Send {
    fn handle_mail(&mut self, mail: Email);
}

pub trait MailHandlerAsync: Send {
    fn handle_mail_async(&mut self, mail: Email) -> Future<()>;
}

impl<T> MailHandlerAsync for T
where
    T: MailHandler,
{
    fn handle_mail_async(&mut self, mail: Email) -> Future<()> {
        self.handle_mail(mail);
        futures::future::ready(()).boxed()
    }
}

pub async fn spawn(
    config: Config,
    handler: impl MailHandlerAsync + 'static,
) -> Result<(), failure::Error> {
    let (collector_future, collector) = Collector::spawn(handler).await;

    let tcp_future = spawn_tcp(config, collector);

    futures::future::try_join(collector_future, tcp_future).await?;
    Ok(())
}

async fn spawn_tcp(config: Config, collector: Collector) -> Result<(), failure::Error> {
    let mut streams = vec![];
    streams.push(TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, 25))?);

    let select = futures::stream::select_all(
        streams
            .iter_mut()
            .map(|s| s.incoming().map_err(failure::Error::from)),
    );
    log::info!("Listening on port 25");

    select
        .try_for_each_concurrent(None, |client| {
            let config = config.clone();
            let collector = collector.clone();
            runtime::spawn(async move {
                let peer_addr = client.peer_addr();
                let local_port = client.local_addr().map(|a| a.port()).unwrap_or(0);
                log::info!("Received client {:?} on port {}", peer_addr, local_port);
                if let Err(e) =
                    crate::connection::run(client, collector.clone(), config.clone()).await
                {
                    log::error!("Client error: {:?}", e);
                }
                log::info!("Client {:?} done", peer_addr);
                Ok(())
            })
        })
        .await?;

    Ok(())
}

pub fn run(config: Config, handler: impl MailHandlerAsync + 'static) -> failure::Error {
    futures::executor::block_on(spawn(config, handler)).unwrap_err()
}
