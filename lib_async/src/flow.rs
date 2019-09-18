use std::borrow::Cow;

#[derive(Debug)]
pub enum Flow {
    Reply(u32, Cow<'static, str>),
    ReplyMultiline(u32, Vec<Cow<'static, str>>),
    EmailReceived {
        sender: String,
        recipient: String,
        body: Vec<u8>,
    },
    UpgradeTls,
    Quit,
}

impl Flow {
    pub const fn status_ok() -> u32 {
        250
    }
    pub const fn status_body_started() -> u32 {
        354
    }
    pub const fn status_err() -> u32 {
        500
    }
}
