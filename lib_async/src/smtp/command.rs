use std::collections::HashMap;

pub enum Command {
    #[allow(dead_code)]
    Ehlo {
        host: String,
    },
    #[allow(dead_code)]
    MailFrom {
        address: String,
        headers: HashMap<String, String>,
    },
    #[allow(dead_code)]
    RecipientTo {
        address: String,
    },
    Data,
    Reset,
    Quit,
    StartTls,
    // Verify {
    //     address: String,
    // },
    // Noop,
}

impl Command {
    pub fn parse(line: &str) -> Result<Self, ParserError> {
        match line
            .get(..4)
            .ok_or(ParserError::InputTooShort)?
            .to_ascii_lowercase()
            .as_str()
        {
            "ehlo" => {
                let host = line.get(4..).unwrap_or("").trim().to_owned();
                Ok(Self::Ehlo { host })
            }
            "mail" => {
                if line
                    .get(5..9)
                    .ok_or(ParserError::InputTooShort)?
                    .to_ascii_lowercase()
                    .as_str()
                    == "from"
                {
                    let remaining = line.get(10..).ok_or(ParserError::InputTooShort)?.trim();
                    let mut parts = remaining.split(' ');
                    let address =
                        trim_brackets(parts.next().ok_or(ParserError::MissingFromAddress)?)
                            .to_owned();
                    let headers = parts
                        .filter_map(|p| {
                            let mut split = p.splitn(2, '=');
                            if let (Some(key), Some(value)) = (split.next(), split.next()) {
                                Some((key.to_owned(), value.to_owned()))
                            } else {
                                None
                            }
                        })
                        .collect();
                    Ok(Self::MailFrom { address, headers })
                } else {
                    Err(ParserError::InvalidSmtpCommand(
                        "MAIL FROM command is missing required fragment FROM",
                    ))
                }
            }
            "rcpt" => {
                if line
                    .get(5..7)
                    .ok_or(ParserError::InputTooShort)?
                    .to_ascii_lowercase()
                    .as_str()
                    == "to"
                {
                    let address =
                        trim_brackets(line.get(8..).ok_or(ParserError::InputTooShort)?.trim())
                            .to_owned();
                    Ok(Self::RecipientTo { address })
                } else {
                    Err(ParserError::InvalidSmtpCommand(
                        "RCPT TO command is missing required fragment TO",
                    ))
                }
            }
            "data" => Ok(Self::Data),
            "rset" => Ok(Self::Reset),
            "quit" => Ok(Self::Quit),
            "star" => {
                if line.trim().to_ascii_lowercase() == "starttls" {
                    Ok(Self::StartTls)
                } else {
                    Err(ParserError::UnknownSmtpCommand)
                }
            }
            _ => {
                println!("Unknown SMTP command: {:?}", line);
                Err(ParserError::UnknownSmtpCommand)
            }
        }
    }
}

fn trim_brackets(s: &str) -> &str {
    if s.starts_with('<') && s.ends_with('>') {
        #[allow(clippy::indexing_slicing)] // Safe because we checked that the two characters exist
        &s[1..s.len() - 1]
    } else {
        s
    }
}

#[test]
fn test_trim_brackets() {
    assert_eq!(trim_brackets("test"), "test");
    assert_eq!(trim_brackets("<test>"), "test");
    assert_eq!(trim_brackets("<test"), "<test");
    assert_eq!(trim_brackets("test>"), "test>");
    assert_eq!(trim_brackets("<"), "<");
    assert_eq!(trim_brackets(">"), ">");
    assert_eq!(trim_brackets(""), "");
    assert_eq!(trim_brackets("a<>"), "a<>");
}

#[derive(Debug)]
pub enum ParserError {
    InputTooShort,
    UnknownSmtpCommand,
    InvalidSmtpCommand(&'static str),
    MissingFromAddress,
}
