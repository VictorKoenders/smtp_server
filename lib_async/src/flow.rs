use std::borrow::Cow;

#[derive(Debug)]
pub enum Flow {
    Silent,
    Reply(Cow<'static, str>),
    ReplyWithCode(u32, Cow<'static, str>),
    UpgradeTls,
    Quit,
}
