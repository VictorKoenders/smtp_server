use crate::Flow;
use bytes::BytesMut;
use tokio::codec::{Decoder, LinesCodec, LinesCodecError};

use crate::smtp::{
    Command as SmtpCommand, CommandParserError as SmtpCommandParserError, State as SmtpState,
    StateError as SmtpStateError,
};
use crate::{Config, Email, Handler};

pub struct Connection {
    codec: LinesCodec,
    smtp: SmtpState,
    config: Config,
    handler: Box<dyn Handler>,
}

impl Connection {
    pub fn new(config: Config, handler: Box<dyn Handler>) -> Self {
        Self {
            codec: LinesCodec::new_with_max_length(config.max_receive_length),
            smtp: SmtpState::Initial,
            config,
            handler,
        }
    }

    pub async fn data_received(
        &mut self,
        bytes: &mut BytesMut,
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
                    let email =
                        Email::parse(sender, recipient, body).map_err(ClientError::EmailParse)?;
                    Some(if let Err(e) = self.handler.save_email(&email).await {
                        self.smtp = SmtpState::Done;
                        Flow::Reply(Flow::status_err(), e.into())
                    } else {
                        self.smtp = SmtpState::Done;
                        Flow::Reply(Flow::status_ok(), "Email received, over and out!".into())
                    })
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
            let command = SmtpCommand::parse(&line).map_err(ClientError::InvalidSmtpCommand)?;
            let flow = self
                .smtp
                .handle_command(command, &self.config)
                .map_err(ClientError::StateError)?;

            last_flow_result = Some(flow)
        }
        Ok(last_flow_result)
    }
}

#[derive(Debug)]
pub enum ClientError {
    MaxLength,
    InvalidSmtpCommand(SmtpCommandParserError),
    StateError(SmtpStateError),
    LinesCodecError(LinesCodecError),
    EmailParse(mailparse::MailParseError),
}
