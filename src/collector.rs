use crate::connection::State;
use std::mem;
use tokio::prelude::{Future, Sink, Stream};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

#[derive(Clone)]
pub struct Collector {
    sender: UnboundedSender<Email>,
}

#[derive(Debug)]
struct Email {
    from: String,
    to: Vec<String>,
    body: String,
}

impl Collector {
    pub fn spawn() -> Collector {
        let (sender, receiver) = unbounded_channel();
        tokio::spawn(
            receiver
                .map_err(|e| {
                    eprintln!("Collector crashed: {:?}", e);
                })
                .for_each(|email| {
                    handle_email(email);
                    Ok(())
                }),
        );

        Collector { sender }
    }
    pub fn collect(&self, message: &mut State) {
        let email = Email {
            from: mem::replace(&mut message.from, Default::default()),
            to: mem::replace(&mut message.recipient, Default::default()),
            body: mem::replace(&mut message.body, Default::default()),
        };
        tokio::spawn(
            self.sender
                .clone()
                .send(email)
                .map_err(|e| {
                    eprintln!("Could not send email to collector: {:?}", e);
                })
                .map(|_| ()),
        );
    }
}

fn handle_email(email: Email) {
    println!("Received email");
    println!("FROM: {:?}", email.from);
    for to in email.to {
        println!("TO: {:?}", to);
    }

    let mail = match mailparse::parse_mail(email.body.as_bytes()) {
        Ok(b) => b,
        Err(e) => {
            println!("Could not parse body: {:?}", e);
            return;
        }
    };

    print_mail(&mail, 2);
}

fn print_mail(mail: &mailparse::ParsedMail, indent: usize) {
    let indent_str = String::from(" ").repeat(indent);
    println!("{}[HEADERS]", indent_str);
    for header in &mail.headers {
        println!(
            "{}{} = {}",
            indent_str,
            header
                .get_key()
                .unwrap_or_else(|e| format!("[ERR {:?}]", e)),
            header
                .get_value()
                .unwrap_or_else(|e| format!("[ERR {:?}]", e))
        );
    }
    println!();
    println!("{}[BODY]", indent_str);
    println!(
        "{}{}",
        indent_str,
        mail.get_body()
            .unwrap_or_else(|e| format!("Could not get body: {:?}", e))
    );
    println!();
    println!("{}[CHILDREN]", indent_str);
    for subpart in &mail.subparts {
        print_mail(subpart, indent + 2);
    }
    println!();
}
