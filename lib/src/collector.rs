use crate::connection::State;
use crate::MailHandlerAsync;
use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, SinkExt, StreamExt};
use std::mem;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct Collector {
    sender: mpsc::UnboundedSender<OwnedEmail>,
}

struct OwnedEmail {
    peer_addr: SocketAddr,
    used_ssl: bool,
    from: String,
    to: Vec<String>,
    body: String,
    returner: oneshot::Sender<bool>,
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
        let (sender, mut receiver) = mpsc::unbounded::<OwnedEmail>();
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
                let returner = email.returner;

                let email = Email {
                    peer_addr: email.peer_addr,
                    used_ssl: email.used_ssl,
                    from: email.from,
                    to: email.to,
                    body: parsed_body,
                };

                let result = handler.handle_mail_async(email).await;
                let _ = returner.send(result);
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
    ) -> Result<bool, failure::Error> {
        let from = mem::replace(&mut message.from, Default::default());
        let to = mem::replace(&mut message.recipient, Default::default());
        let body = mem::replace(&mut message.body, Default::default());
        let (sender, receiver) = futures::channel::oneshot::channel();
        self.sender
            .send(OwnedEmail {
                from,
                to,
                body,
                peer_addr,
                used_ssl: is_ssl,
                returner: sender
            })
            .await?;
        let result = receiver.await?;
        Ok(result)
    }
}
