use crate::connection::State;
use crate::MailHandlerAsync;
use futures::channel::mpsc::{unbounded, UnboundedSender};
use futures::{FutureExt, SinkExt, StreamExt};
use std::mem;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct Collector {
    sender: UnboundedSender<OwnedEmail>,
}

struct OwnedEmail {
    peer_addr: SocketAddr,
    used_ssl: bool,
    from: String,
    to: Vec<String>,
    body: String,
}

#[derive(Debug)]
pub struct Email<'a> {
    pub peer_addr: SocketAddr,
    pub used_ssl: bool,
    pub from: String,
    pub to: Vec<String>,
    pub body: mailparse::ParsedMail<'a>,
}

impl Collector {
    pub async fn spawn(
        mut handler: impl MailHandlerAsync + 'static,
    ) -> (crate::Future<Result<(), failure::Error>>, Collector) {
        let (sender, mut receiver) = unbounded::<OwnedEmail>();
        let fut = runtime::spawn(async move {
            while let Some(email) = receiver.next().await {
                println!("{:?}", email.body);
                let body = email.body;
                let parsed_body = match mailparse::parse_mail(body.as_bytes()) {
                    Ok(b) => b,
                    Err(e) => {
                        log::error!("Could not parse mail body");
                        log::error!("{:?}", body);
                        log::error!("{:?}", e);
                        log::error!("-- Ignoring email --");
                        return Ok(());
                    }
                };

                let email = Email {
                    peer_addr: email.peer_addr,
                    used_ssl: email.used_ssl,
                    from: email.from,
                    to: email.to,
                    body: parsed_body,
                };

                handler.handle_mail_async(email).await;
            }
            Ok(())
        });

        (fut.boxed(), Collector { sender })
    }

    pub async fn collect(
        &mut self,
        message: &mut State,
        peer_addr: SocketAddr,
        is_ssl: bool,
    ) -> Result<(), failure::Error> {
        let from = mem::replace(&mut message.from, Default::default());
        let to = mem::replace(&mut message.recipient, Default::default());
        let body = mem::replace(&mut message.body, Default::default());
        self.sender
            .send(OwnedEmail {
                from,
                to,
                body,
                peer_addr,
                used_ssl: is_ssl,
            })
            .await?;
        Ok(())
    }
}
