use super::Command;
use crate::{Capability, Config, Flow};

#[derive(Clone)]
pub enum State {
    Initial,
    EhloReceived,
    #[allow(dead_code)]
    SenderReceived {
        sender: String,
    },
    #[allow(dead_code)]
    RecipientReceived {
        sender: String,
        recipient: String,
    },
    #[allow(dead_code)]
    ReceivingBody {
        sender: String,
        recipient: String,
        body: Vec<u8>,
    },
    Done,
}

impl State {
    pub fn handle_command(
        &mut self,
        command: Command,
        config: &Config,
        is_tls: bool,
    ) -> Result<Flow, Error> {
        match Self::handle_command_impl(self.clone(), command, config, is_tls) {
            Ok((new_state, flow)) => {
                *self = new_state;
                Ok(flow)
            }
            Err(e) => Err(e),
        }
    }

    fn handle_command_impl(
        state: Self,
        command: Command,
        config: &Config,
        is_tls: bool,
    ) -> Result<(Self, Flow), Error> {
        Ok(match (state, command) {
            (Self::Initial, Command::Ehlo { host }) => {
                let mut result = vec![format!("{}, nice to meet you!", host).into()];
                for capability in &config.capabilities {
                    if is_tls && capability == &Capability::StartTls {
                        // https://tools.ietf.org/html/rfc3207
                        // page 4: A server MUST NOT return the STARTTLS extension in response to an EHLO command received after a TLS handshake has completed.
                        continue;
                    }
                    result.push(capability.to_cow_str(config));
                }
                (
                    Self::EhloReceived,
                    Flow::ReplyMultiline(Flow::status_ok(), result),
                )
            }
            (Self::EhloReceived, Command::MailFrom { address, .. }) => (
                Self::SenderReceived { sender: address },
                Flow::Reply(Flow::status_ok(), "Tell them I said hi".into()),
            ),
            (Self::SenderReceived { sender }, Command::RecipientTo { address }) => (
                Self::RecipientReceived {
                    sender,
                    recipient: address,
                },
                Flow::Reply(
                    Flow::status_ok(),
                    "I'll make sure to get this to them".into(),
                ),
            ),
            (Self::RecipientReceived { sender, recipient }, Command::Data) => (
                Self::ReceivingBody {
                    sender,
                    recipient,
                    body: Vec::new(),
                },
                Flow::Reply(
                    Flow::status_body_started(),
                    "Go ahead, I'm listening (end with \\r\\n.\\r\\n)".into(),
                ),
            ),
            (Self::Done, Command::Reset) => (
                Self::EhloReceived,
                Flow::Reply(Flow::status_ok(), "We're ready to go another round!".into()),
            ),
            (_, Command::Quit) => (Self::Initial, Flow::Quit),
            (_, Command::Reset) => (
                Self::EhloReceived,
                Flow::Reply(Flow::status_ok(), "I'm sorry, who are you again?".into()),
            ),
            (Self::EhloReceived, Command::StartTls)
                if config.has_capability(&Capability::StartTls) =>
            {
                (Self::Initial, Flow::UpgradeTls)
            }
            (state, _) => {
                return Err(Error::UnknownCommand {
                    expected: state.expected(),
                })
            }
        })
    }

    fn expected(&self) -> &'static str {
        match self {
            Self::Initial => "EHLO",
            Self::EhloReceived { .. } => "MAIL FROM",
            Self::SenderReceived { .. } => "RCPT TO",
            Self::RecipientReceived { .. } => "BODY",
            Self::ReceivingBody { .. } => unreachable!(),
            Self::Done { .. } => "QUIT or RSET",
        }
    }
}

#[derive(Debug)]
pub enum Error {
    UnknownCommand { expected: &'static str },
}
