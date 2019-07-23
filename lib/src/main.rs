#![feature(async_await)]

use smtp_server::{mailparse::ParsedMail, Config, Email, MailHandler};

#[runtime::main]
async fn main() {
    env_logger::init();
    let config = Config::build("localhost")
        // .with_tls_from_pfx("identity.pfx").expect("Could not load identity.pfx")
        .build();

    if let Err(err) = smtp_server::spawn(config, Handler).await {
        eprintln!("SMTP server crashed: {:?}", err);
    }
    eprintln!("Server stopping");
}

struct Handler;

impl MailHandler for Handler {
    fn handle_mail(&mut self, email: Email) {
        println!("Received email");
        println!("FROM: {:?}", email.from);
        for to in email.to {
            println!("TO: {:?}", to);
        }

        print_mail(&email.body, 2);
    }
}

fn print_mail(mail: &ParsedMail, indent: usize) {
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
