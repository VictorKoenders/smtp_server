use std::borrow::Cow;

#[derive(Debug)]
pub enum Flow {
    Reply(u32, Cow<'static, str>),
    ReplyMultiline(u32, Vec<Cow<'static, str>>),
    UpgradeTls,
    Quit,
}

impl Flow {
    pub const fn status_ok() -> u32 {
        250
    }
    pub const fn status_err() -> u32 {
        500
    }
}
