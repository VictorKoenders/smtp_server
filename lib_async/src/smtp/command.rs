use std::collections::HashMap;

pub enum Command {
    Ehlo {
        host: String,
    },
    MailFrom {
        address: String,
        headers: HashMap<String, String>,
    },
    RecipientTo {
        address: String,
    },
    Data,
    Reset,
    Quit,
    // Verify {
    //     address: String,
    // },
    // Noop,
}

impl Command {
    pub fn parse(line: &str) -> Result<Command, ParserError> {
        match line[..4].to_ascii_lowercase().as_str() {
            "ehlo" => {
                let host = line[4..].trim().to_owned();
                Ok(Command::Ehlo { host })
            }
            "mail" => {
                if line[5..9].to_ascii_lowercase().as_str() != "from" {
                    Err(ParserError::InvalidSmtpCommand(
                        "MAIL FROM command is missing required fragment FROM",
                    ))
                } else {
                    let remaining = line[10..].trim();
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
                    Ok(Command::MailFrom { address, headers })
                }
            }
            "rcpt" => {
                if line[5..7].to_ascii_lowercase().as_str() != "to" {
                    Err(ParserError::InvalidSmtpCommand(
                        "RCPT TO command is missing required fragment TO",
                    ))
                } else {
                    let address = trim_brackets(line[8..].trim()).to_owned();
                    Ok(Command::RecipientTo { address })
                }
            }
            "data" => Ok(Command::Data),
            "rset" => Ok(Command::Reset),
            "quit" => Ok(Command::Quit),
            _ => {
                println!("Unknown SMTP command: {:?}", line);
                Err(ParserError::UnknownSmtpCommand)
            }
        }
    }
}

fn trim_brackets(s: &str) -> &str {
    if s.starts_with('<') && s.ends_with('>') {
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
    UnknownSmtpCommand,
    InvalidSmtpCommand(&'static str),
    MissingFromAddress,
}
