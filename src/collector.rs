use crate::connection::State;
use crate::MailHandlerAsync;
use futures::channel::mpsc::{unbounded, UnboundedSender};
use futures::{FutureExt, SinkExt, StreamExt};
use std::mem;

#[derive(Clone)]
pub struct Collector {
    sender: UnboundedSender<OwnedEmail>,
}

struct OwnedEmail {
    from: String,
    to: Vec<String>,
    body: String,
}

#[derive(Debug)]
pub struct Email<'a> {
    pub from: String,
    pub to: Vec<String>,
    pub body: mailparse::ParsedMail<'a>,
}

impl Collector {
    pub async fn spawn(mut handler: impl MailHandlerAsync + 'static) -> (crate::Future<Result<(), failure::Error>>, Collector) {
        let (sender, mut receiver) = unbounded::<OwnedEmail>();
        let fut = runtime::spawn(async move {
            while let Some(email) = receiver.next().await {
                let body = email.body;
                let parsed_body = match mailparse::parse_mail(body.as_bytes()) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("Could not parse mail body");
                        eprintln!("{:?}", body);
                        eprintln!("{:?}", e);
                        eprintln!("-- Ignoring email --");
                        return Ok(());
                    }
                };

                let email = Email {
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

    pub async fn collect(&mut self, message: &mut State) -> Result<(), failure::Error> {
        let from = mem::replace(&mut message.from, Default::default());
        let to = mem::replace(&mut message.recipient, Default::default());
        let body = mem::replace(&mut message.body, Default::default());
        self.sender.send(OwnedEmail { from, to, body }).await?;
        Ok(())
    }
}
