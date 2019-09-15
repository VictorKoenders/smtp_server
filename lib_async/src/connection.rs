use crate::Flow;
use bytes::BytesMut;
use tokio::codec::{Decoder, LinesCodec, LinesCodecError};

use crate::smtp::{
    Command as SmtpCommand, CommandParserError as SmtpCommandParserError, State as SmtpState,
    StateError as SmtpStateError,
};
use crate::Config;

pub struct Connection {
    codec: LinesCodec,
    smtp: SmtpState,
    config: Config,
}

impl Connection {
    pub fn new(config: Config) -> Self {
        Self {
            codec: LinesCodec::new_with_max_length(config.max_receive_length),
            smtp: SmtpState::Initial,
            config,
        }
    }

    pub fn data_received(&mut self, bytes: &mut BytesMut) -> Result<Option<Flow>, ClientError> {
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
                    println!("Received email from {} to {}", sender, recipient);
                    if let Ok(body) = std::str::from_utf8(&body) {
                        println!("{}", body);
                    } else {
                        println!("{:?}", body);
                    }
                    self.smtp = SmtpState::Done;
                    Some(Flow::Reply("Email received, over and out!".into()))
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
                .handle_command(command)
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
}
