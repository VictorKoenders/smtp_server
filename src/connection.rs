use crate::collector::Collector;
use crate::config::Config;
use crate::message_parser::MessageParser;
use futures::io::AsyncWriteExt;
use futures::stream::StreamExt;
use runtime::net::TcpStream;
use std::borrow::Cow;

pub struct Connection;

impl Connection {
    /*
    pub fn spawn_tls(client: TcpStream, collector: Collector, config: Config) {
        runtime::spawn(async {

        }
            Connection::run_tls(client, collector, Default::default(), config)
                .map_err(|e| {
                    eprintln!("Could not run connection: {:?}", e);
                })
                .boxed()
                .compat(),
        );
    }
    */

    pub async fn run(
        mut client: TcpStream,
        mut collector: Collector,
        config: Config,
    ) -> Result<(), failure::Error> {
        /*
        let reader = LinesCodec::new().framed(&client);
        let (sink, stream) = reader.split();
        let mut sink = Compat01As03Sink::<_, String>::new(sink);
        let mut stream = Compat01As03::new(stream);
        */
        let mut state = State::default();

        let msg = format!("220 {} ESMTP MailServer\r\n", config.host);
        client.write_all(msg.as_bytes()).await?;
        println!("Wrote hello");

        let mut reader = crate::line_reader::LineReader::new(client);

        while let Some(line) = reader.next().await {
            let line = line?;
            match state.message_received(&line) {
                LineResponse::None => {}
                LineResponse::Upgrade => {
                    reader.reader.write_all(b"500 Not implemented\r\n").await?;
                    /*println!("Upgrading request");
                    client.write_all(b"220 Go ahead").await?;
                    return Connection::run_tls(client, collector, state, config).await;
                    */
                }
                LineResponse::ReplyWith(msg) => {
                    reader.reader.write_all(msg.as_bytes()).await?;
                    reader.reader.write_all(b"\r\n").await?;
                }
                LineResponse::ReplyWithMultiple(msg) => {
                    for msg in msg {
                        reader.reader.write_all(msg.as_bytes()).await?;
                        reader.reader.write_all(b"\r\n").await?;
                    }
                }
                LineResponse::Done => {
                    reader
                        .reader
                        .write_all(b"250 Ok: Message received, over\r\n")
                        .await?;
                    collector.collect(&mut state).await?;
                    state = Default::default();
                }
                LineResponse::Quit => {
                    reader.reader.write_all(b"200 Come back soon!\r\n").await?;
                    break;
                } // LineResponse::Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    /*
    async fn run_tls(
        client: TcpStream,
        collector: Collector,
        mut state: State,
        config: Config,
    ) -> Result<(), failure::Error> {
        let config = config.read();
        let tls_acceptor = match config.tls_acceptor.as_ref() {
            Some(t) => t,
            None => {
                eprintln!("Tried to accept TLS connection, but TLS was not configured");
                eprintln!("Please call `config_builder.with_tls_from_pfx(\"identity.pfx\").expect(\"Could not load identity.pfx\")`");
                failure::bail!("TLS not implemented");
            }
        };
        let stream = Compat01As03::new(tls_acceptor.accept(client)).await?;

        let reader = LinesCodec::new().framed(stream);
        let (sink, stream) = reader.split();
        let mut sink = Compat01As03Sink::<_, String>::new(sink);
        let mut stream = Compat01As03::new(stream);

        while let Some(line) = stream.next().await {
            let line: String = line?;
            match state.message_received(&line) {
                LineResponse::None => {}
                LineResponse::Upgrade => {
                    sink.send(String::from("500 Already upgraded")).await?;
                }
                LineResponse::ReplyWith(msg) => {
                    sink.send(msg.into_owned()).await?;
                }
                LineResponse::ReplyWithMultiple(msg) => {
                    for msg in msg {
                        sink.send(msg.into_owned()).await?;
                    }
                }
                LineResponse::Done => {
                    sink.send(String::from("250 Ok: Message received, over"))
                        .await?;
                    collector.collect(&mut state);
                    state = Default::default();
                } // LineResponse::Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    */
}

#[derive(Default, Debug)]
pub struct State {
    pub from: String,
    pub recipient: Vec<String>,
    pub body: String,
    ehlo_received: bool,
    is_reading_body: bool,
}

type StateFn = &'static (dyn Sync + Fn(&mut State, MessageParser) -> LineResponse);

lazy_static::lazy_static! {
    static ref SMTP_COMMANDS: std::collections::HashMap<&'static [u8; 4], StateFn> = {
        let mut map = std::collections::HashMap::<&'static [u8; 4], StateFn>::new();
        map.insert(b"EHLO", &handle_ehlo);
        map.insert(b"MAIL", &handle_mail);
        map.insert(b"RCPT", &handle_recipient);
        map.insert(b"SIZE", &handle_size);
        map.insert(b"DATA", &handle_data);
        map.insert(b"VRFY", &handle_verify);
        map.insert(b"TURN", &handle_turn);
        map.insert(b"AUTH", &handle_auth);
        map.insert(b"RSET", &handle_reset);
        map.insert(b"EXPN", &handle_expn);
        map.insert(b"HELP", &handle_help);
        map.insert(b"QUIT", &handle_quit);
        map
    };
}

fn handle_ehlo(state: &mut State, _parser: MessageParser) -> LineResponse {
    state.ehlo_received = true;
    LineResponse::ReplyWithMultiple(vec![
        "250-localhost, I'm glad to meet you".into(),
        "250-AUTH LOGIN PLAIN".into(),
        "250 STARTTLS".into(),
    ])
}

fn handle_mail(state: &mut State, mut parser: MessageParser) -> LineResponse {
    if !state.ehlo_received {
        return "500 Aren't you supposed to introduce yourself? (Send EHLO)".into();
    }
    match parser.consume_word_until(COLON) {
        Some(word) => {
            let word = word.to_ascii_uppercase();
            if word == "FROM" {
                println!("[MAIL] from {}", parser.remaining());
                state.from = parser.remaining().to_owned();
                "250 I'll let them know".into()
            } else {
                "500 Expected FROM after MAIL".into()
            }
        }
        None => "500 Expected FROM after MAIL".into(),
    }
}

fn handle_recipient(state: &mut State, mut parser: MessageParser) -> LineResponse {
    if !state.ehlo_received {
        return "500 Aren't you supposed to introduce yourself? (Send EHLO)".into();
    }
    match parser.consume_word_until(COLON) {
        Some(word) => {
            let word = word.to_ascii_uppercase();
            if word == "TO" {
                println!("[MAIL] to {}", parser.remaining(),);
                let recipient = parser.remaining();
                state.recipient.push(recipient.to_owned());
                format!("250 Say hi to {} for me", recipient).into()
            } else {
                "500 Expected TO after RCPT".into()
            }
        }
        None => "500 Expected TO after RCPT".into(),
    }
}

fn handle_size(_state: &mut State, mut _parser: MessageParser) -> LineResponse {
    println!("TODO: SIZE {}", _parser.remaining());
    "500 Not implemented".into()
}

fn handle_data(state: &mut State, mut _parser: MessageParser) -> LineResponse {
    state.is_reading_body = true;
    "354 Go on, I'm listening... (end with \\r\\n.\\r\\n)".into()
}

fn handle_verify(_state: &mut State, mut _parser: MessageParser) -> LineResponse {
    println!("TODO: VRFY {}", _parser.remaining());
    "500 Not implemented".into()
}

fn handle_turn(_state: &mut State, mut _parser: MessageParser) -> LineResponse {
    println!("TODO: TURN {}", _parser.remaining());
    "500 Not implemented".into()
}

fn handle_auth(_state: &mut State, mut _parser: MessageParser) -> LineResponse {
    println!("TODO: AUTH {}", _parser.remaining());
    "500 Not implemented".into()
}

fn handle_reset(state: &mut State, mut _parser: MessageParser) -> LineResponse {
    *state = Default::default();
    "200 It's all gone".into()
}

fn handle_expn(_state: &mut State, mut _parser: MessageParser) -> LineResponse {
    println!("TODO: EXPN {}", _parser.remaining());
    "500 Not implemented".into()
}

fn handle_help(_state: &mut State, mut _parser: MessageParser) -> LineResponse {
    println!("TODO: HELP {}", _parser.remaining());
    "500 Not implemented".into()
}

fn handle_quit(_state: &mut State, mut _parser: MessageParser) -> LineResponse {
    LineResponse::Quit
}

const COLON: u8 = b':';

impl State {
    fn message_received(&mut self, msg: &str) -> LineResponse {
        if self.is_reading_body {
            println!("[BODY] {}", msg);
            if msg == "." {
                self.is_reading_body = false;
                LineResponse::Done
            } else {
                self.body += msg;
                self.body += "\r\n";
                LineResponse::None
            }
        } else if msg.get(..8).map(|m| m.to_ascii_uppercase()) == Some(String::from("STARTTLS")) {
            LineResponse::Upgrade
        } else if let Some(chars) = msg.get(..4) {
            let upper_case = chars.to_ascii_uppercase();
            let bytes: &[u8; 4] = arrayref::array_ref![upper_case.as_bytes(), 0, 4];
            if let Some(cmd) = SMTP_COMMANDS.get(bytes) {
                let parser = MessageParser::new(msg[4..].trim());
                cmd(self, parser)
            } else {
                println!("Unknown client command: {:?}", msg);
                "500 Unknown command".into()
            }
        } else {
            println!("Unknown client command: {:?}", msg);
            "500 Unknown command".into()
        }
    }
}

#[derive(Debug)]
enum LineResponse {
    None,
    ReplyWith(Cow<'static, str>),
    ReplyWithMultiple(Vec<Cow<'static, str>>),
    Upgrade,
    Done,
    Quit,
    // Err(failure::Error),
}

impl From<String> for LineResponse {
    fn from(s: String) -> LineResponse {
        LineResponse::ReplyWith(s.into())
    }
}

impl From<&'static str> for LineResponse {
    fn from(s: &'static str) -> LineResponse {
        LineResponse::ReplyWith(s.into())
    }
}
