use async_trait::async_trait;
use mailparse::ParsedMail;
use std::net::SocketAddr;

#[async_trait]
pub trait Handler: Sync + Send + 'static {
    async fn validate_address(&self, email_address: &str) -> bool;
    async fn save_email<'a>(&self, email: &Email<'a>) -> Result<(), String>;
    fn clone_box(&self) -> Box<dyn Handler>;
}

#[cfg(test)]
pub struct TestHandler;

#[cfg(test)]
#[async_trait]
impl Handler for TestHandler {
    async fn validate_address(&self, email_address: &str) -> bool {
        true
    }
    async fn save_email<'a>(&self, email: &Email<'a>) -> Result<(), String> {
        Ok(())
    }
    fn clone_box(&self) -> Box<dyn Handler> {
        Box::new(TestHandler)
    }
}

impl<'a> Email<'a> {
    pub fn parse(
        peer_addr: SocketAddr,
        used_ssl: bool,
        sender: &'a str,
        recipient: &'a str,
        body: &'a [u8],
    ) -> Result<Self, mailparse::MailParseError> {
        Ok(Self {
            peer_addr,
            used_ssl,
            sender,
            recipient,
            email: mailparse::parse_mail(body)?,
            raw_body: body,
        })
    }
}

pub struct Email<'a> {
    pub peer_addr: SocketAddr,
    pub used_ssl: bool,
    pub sender: &'a str,
    pub recipient: &'a str,
    pub email: ParsedMail<'a>,
    pub raw_body: &'a [u8],
}
