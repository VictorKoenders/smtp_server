extern crate lib_async;

use async_trait::async_trait;
use lib_async::{ConfigBuilder, Email, SmtpServer};
use std::net::{IpAddr, Ipv4Addr};

#[tokio::main]
async fn main() {
    let config = ConfigBuilder::default()
        .with_hostname("trangar.com")
        .with_server_name("Trangars NIH mail server")
        .with_max_size(1024 * 1024 * 1024 * 10 /* 10 MB */)
        .with_pkcs12_certificate("certificate.pfx", "")
        .expect("Could not load certificate.pfx")
        .build();
    /*
    {
        max_receive_length: 1024 * 1024 * 1024 * 10, // 10 MB
        hostname: "mail.trangar.com".to_owned(),
        mail_server_name: "Trangar's NIH mail server".to_owned(),
        capabilities: vec![Capability::StartTls, Capability::Size, Capability::SmtpUtf8],
    }
    */
    let mut server = SmtpServer::create(Handler, config);
    server
        .register_listener((IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 25))
        .await
        .expect("Could not listen on port 25");
    server
        .register_listener((IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 587))
        .await
        .expect("Could not listen on port 587");
    server
        .register_tls_listener((IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 465))
        .await
        .expect("Could not listen on port 465");
    server.run().await;
}

#[derive(Clone)]
struct Handler;

#[async_trait]
impl lib_async::Handler for Handler {
    async fn validate_address(&self, _email_address: &str) -> bool {
        true
    }

    #[allow(clippy::needless_lifetimes)]
    async fn save_email<'a>(&self, email: &Email<'a>) -> Result<(), String> {
        println!(
            "Received email from {} to {}",
            email.sender, email.recipient
        );
        println!("raw body: {:?}", std::str::from_utf8(email.raw_body));
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn lib_async::Handler> {
        Box::new(Handler)
    }
}
