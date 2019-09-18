use crate::Flow;
use bytes::BytesMut;
use tokio::codec::{Decoder, LinesCodec, LinesCodecError};
use tokio::net::TcpStream;
use tokio_tls::TlsStream;

use crate::smtp::{
    Command as SmtpCommand, CommandParserError as SmtpCommandParserError, State as SmtpState,
    StateError as SmtpStateError,
};
use crate::{Config, Email, Handler};

pub struct Connection {
    codec: LinesCodec,
    smtp: SmtpState,
    pub(crate) config: Config,
    handler: Box<dyn Handler>,
}

#[test]
fn data_received() {
    for test in &[
        &[0x0a][..],
        &[0x0d, 0x0a][..]
    ] {
        let mut connection = Connection::new_test();
        let mut bytes = bytes::BytesMut::new();
        for line in test.split(|b| b == &b'\n') {
            bytes.extend_from_slice(line);
            let _result = connection.data_received(
                &mut bytes,
                true
            );
        }
    }
}

impl Connection {
    #[cfg(test)]
    fn new_test() -> Self {
        let config = crate::ConfigBuilder::default()
            .with_max_size(1024 * 1024)
            .build();
        Self {
            codec: LinesCodec::new_with_max_length(config.max_receive_length),
            smtp: SmtpState::Initial,
            config,
            handler: Box::new(crate::handler::TestHandler),
        }
    }

    pub fn new(config: Config, handler: Box<dyn Handler>) -> Self {
        Self {
            codec: LinesCodec::new_with_max_length(config.max_receive_length),
            smtp: SmtpState::Initial,
            config,
            handler,
        }
    }

    pub fn new_connection_message(&self) -> String {
        format!(
            "220 {} {}\r\n",
            self.config.hostname, self.config.mail_server_name
        )
    }

    pub async fn try_send_email(
        &self,
        sender: String,
        recipient: String,
        body: Vec<u8>,
    ) -> Result<(), ClientError> {
        let email = Email::parse(&sender, &recipient, &body).map_err(ClientError::EmailParse)?;
        self.handler
            .save_email(&email)
            .await
            .map_err(ClientError::String)?;
        Ok(())
    }

    pub fn data_received(
        &mut self,
        bytes: &mut BytesMut,
        is_tls: bool,
    ) -> Result<Option<Flow>, ClientError> {
        if let SmtpState::ReceivingBody {
            body,
            recipient,
            sender,
        } = &mut self.smtp
        {
            body.extend_from_slice(&bytes[..]);
            bytes.clear();
            return if body.len() > self.config.max_receive_length {
                body.clear();
                Err(ClientError::MaxLength)
            } else {
                Ok(if body.ends_with(b"\r\n.\r\n") {
                    let flow = Flow::EmailReceived {
                        sender: sender.clone(),
                        recipient: recipient.clone(),
                        body: body.clone(),
                    };
                    self.smtp = SmtpState::Done;
                    Some(flow)
                } else {
                    None
                })
            };
        }

        let mut last_flow_result = None;
        while let Some(line) = self
            .codec
            .decode(bytes)
            .map_err(ClientError::LinesCodecError)?
        {
            println!("IN: {}", line.trim());
            let command = SmtpCommand::parse(&line).map_err(ClientError::InvalidSmtpCommand)?;
            let flow = self
                .smtp
                .handle_command(command, &self.config, is_tls)
                .map_err(ClientError::StateError)?;

            last_flow_result = Some(flow)
        }
        Ok(last_flow_result)
    }

    pub async fn upgrade_tls(
        &self,
        stream: TcpStream,
    ) -> Result<TlsStream<TcpStream>, native_tls::Error> {
        let acceptor = self.config.tls_acceptor.as_ref().expect("Software tried to upgrade a TCP stream, but no TLS acceptor was configured. This is a bug");
        acceptor.accept(stream).await
    }
}

#[derive(Debug)]
pub enum ClientError {
    MaxLength,
    InvalidSmtpCommand(SmtpCommandParserError),
    StateError(SmtpStateError),
    LinesCodecError(LinesCodecError),
    EmailParse(mailparse::MailParseError),
    String(String),
}
