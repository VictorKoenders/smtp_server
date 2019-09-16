use super::Command;
use crate::{Config, Flow};

#[derive(Clone)]
pub enum State {
    Initial,
    EhloReceived,
    SenderReceived {
        sender: String,
    },
    RecipientReceived {
        sender: String,
        recipient: String,
    },
    ReceivingBody {
        sender: String,
        recipient: String,
        body: Vec<u8>,
    },
    Done,
}

impl State {
    pub fn handle_command(&mut self, command: Command, config: &Config) -> Result<Flow, Error> {
        match State::handle_command_impl(self.clone(), command, config) {
            Ok((new_state, flow)) => {
                *self = new_state;
                Ok(flow)
            }
            Err(e) => Err(e),
        }
    }

    fn handle_command_impl(
        state: State,
        command: Command,
        config: &Config,
    ) -> Result<(State, Flow), Error> {
        Ok(match (state, command) {
            (State::Initial, Command::Ehlo { host }) => {
                let mut result = vec![format!("Hello {}", host).into()];
                for capability in &config.capabilities {
                    result.push(capability.to_cow_str(config));
                }
                (
                    State::EhloReceived,
                    Flow::ReplyMultiline(Flow::status_ok(), result),
                )
            }
            (State::EhloReceived, Command::MailFrom { address, .. }) => (
                State::SenderReceived { sender: address },
                Flow::Reply(Flow::status_ok(), "Tell them I said hi".into()),
            ),
            (State::SenderReceived { sender }, Command::RecipientTo { address }) => (
                State::RecipientReceived {
                    sender,
                    recipient: address,
                },
                Flow::Reply(
                    Flow::status_ok(),
                    "I'll make sure to get this to them".into(),
                ),
            ),
            (State::RecipientReceived { sender, recipient }, Command::Data) => (
                State::ReceivingBody {
                    sender,
                    recipient,
                    body: Vec::new(),
                },
                Flow::Reply(
                    354,
                    "Go ahead, I'm listening (end with \\r\\n.\\r\\n)".into(),
                ),
            ),
            (State::Done, Command::Reset) => (
                State::EhloReceived,
                Flow::Reply(Flow::status_ok(), "We're ready to go another round!".into()),
            ),
            (_, Command::Quit) => (State::Initial, Flow::Quit),
            (state, _) => {
                return Err(Error::UnknownCommand {
                    expected: state.expected(),
                })
            }
        })
    }

    fn expected(&self) -> &'static str {
        match self {
            State::Initial => "EHLO",
            State::EhloReceived { .. } => "MAIL FROM",
            State::SenderReceived { .. } => "RCPT TO",
            State::RecipientReceived { .. } => "BODY",
            State::ReceivingBody { .. } => unreachable!(),
            State::Done { .. } => "QUIT or RSET",
        }
    }
}

#[derive(Debug)]
pub enum Error {
    UnknownCommand { expected: &'static str },
}
