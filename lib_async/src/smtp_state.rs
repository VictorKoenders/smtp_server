/*#[derive(Clone)]
pub enum SmtpState {
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

impl SmtpState {
    pub fn command_received(&mut self, command: SmtpCommand) -> Result<Flow, SmtpError> {
        let (new_state, flow): (SmtpState, Flow) = match (*self, command) {
            (SmtpState::Initial, SmtpCommand::Ehlo { .. }) => (
                SmtpState::EhloReceived,
                Flow::Continue(String::from("Hello!")),
            ),
            (SmtpState::EhloReceived, SmtpCommand::MailFrom { address }) => (
                SmtpState::SenderReceived { sender: address },
                Flow::Continue("Tell them I said hi".to_owned()),
            ),
            (SmtpState::SenderReceived { sender }, SmtpCommand::RecipientTo { address }) => (
                SmtpState::RecipientReceived {
                    sender,
                    recipient: address,
                },
                Flow::Continue("I'll make sure to get this to them".to_owned()),
            ),
            (SmtpState::RecipientReceived { sender, recipient }, SmtpCommand::Data) => (
                SmtpState::ReceivingBody {
                    sender,
                    recipient,
                    body: Vec::new(),
                },
                Flow::Continue("Go ahead, I'm listening (end with \\r\\n.\\r\\n)".to_owned()),
            ),
            (
                SmtpState::ReceivingBody {
                    body,
                    sender,
                    recipient,
                },
                SmtpCommand::Raw(append),
            ) => {
                let mut body = body.clone();
                body.extend_from_slice(&append);
                if body.ends_with(b"\r\n.\r\n") {
                    println!("Received email from {} to {}", sender, recipient);
                    println!("{:?}", std::str::from_utf8(&body));
                    (
                        SmtpState::Done,
                        Flow::Continue("Message received, over and out!".to_owned()),
                    )
                } else {
                    (
                        SmtpState::ReceivingBody {
                            body,
                            sender,
                            recipient,
                        },
                        Flow::Silent,
                    )
                }
            }
            (SmtpState::Done, SmtpCommand::Reset) => (
                SmtpState::EhloReceived,
                Flow::Continue("We're ready to go another round!".to_owned()),
            ),
            (state, _) => {
                return Err(SmtpError::UnknownCommand {
                    expected: state.expected(),
                })
            }
        };
        *self = new_state;
        Ok(flow)
    }

    fn expected(&self) -> &'static str {
        match self {
            SmtpState::Initial => "EHLO",
            SmtpState::EhloReceived { .. } => "MAIL FROM",
            SmtpState::SenderReceived { .. } => "RCPT TO",
            SmtpState::RecipientReceived { .. } => "BODY",
            SmtpState::ReceivingBody { .. } => unreachable!(),
            SmtpState::Done { .. } => "QUIT or RSET",
        }
    }
}


*/
