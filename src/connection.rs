use crate::collector::Collector;
use crate::config::Config;
use crate::message_parser::{ByteOrEnd, MessageParser};
use futures::compat::Compat01As03;
use futures::{
    compat::{AsyncRead01CompatExt, AsyncWrite01CompatExt, Compat01As03Sink, Stream01CompatExt},
    future::{FutureExt, TryFutureExt},
    io::{AsyncReadExt, AsyncWriteExt},
    sink::{Sink, SinkExt},
    stream::StreamExt,
};
use native_tls::TlsConnector as NativeTlsConnector;
use parking_lot::RwLock;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::codec::Framed;
use tokio::codec::{Decoder, Encoder, LinesCodec};
use tokio::net::TcpStream;
use tokio::prelude::Stream;
use tokio_tls::TlsConnector;
use unicase::UniCase;

pub struct Connection;

impl Connection {
    pub fn spawn(client: TcpStream, collector: Collector, config: Arc<RwLock<Config>>) {
        tokio::spawn(
            Connection::run(client, collector, config)
                .map_err(|e| {
                    eprintln!("Could not run connection: {:?}", e);
                })
                .boxed()
                .compat(),
        );
    }

    // TODO: Merge the functions `run` and `run_tls`
    // Currently we can't because we have a framed(&client), because we need to reunite the sink/stream later. The issue is that:
    // - TlsStream does not support reading from &client (https://github.com/tokio-rs/tokio/issues/1239)
    // - We can't re-unite client because it's contained in  the Compat layer
    // We can fix this issue when either of the following issues is resolved:
    // - tokio supports futures03: https://github.com/tokio-rs/tokio/issues/1194
    // - futures-rs supports .into_inner(): https://github.com/rust-lang-nursery/futures-rs/pull/1705
    // In the `LineResponse::Upgrade` branch of the inner match statements, we should `reunite` the `split` that happened:
    // https://docs.rs/tokio/0.1.22/tokio/prelude/stream/struct.SplitSink.html#method.reunite
    // That means we don't have to create a linecodec with a reference, but the actual TcpStream
    async fn run(
        client: TcpStream,
        collector: Collector,
        config: Arc<RwLock<Config>>,
    ) -> Result<(), failure::Error> {
        let reader = LinesCodec::new().framed(&client);
        let (sink, stream) = reader.split();
        let mut sink = Compat01As03Sink::<_, String>::new(sink);
        let mut stream = Compat01As03::new(stream);
        let mut state = State::default();

        sink.send(format!("220 {} ESMTP MailServer", config.read().host))
            .await?;

        while let Some(line) = stream.next().await {
            let line = line?;
            match state.message_received(&line) {
                LineResponse::None => {}
                LineResponse::Upgrade => {
                    println!("Upgrading request");
                    sink.send(String::from("220 Go ahead")).await?;
                    drop(sink);
                    drop(stream);
                    return Connection::run_tls(client, collector, state, config).await;
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
                }
                LineResponse::Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    async fn run_tls(
        client: TcpStream,
        collector: Collector,
        state: State,
        config: Arc<RwLock<Config>>,
    ) -> Result<(), failure::Error> {
        let stream = Compat01As03::new(config.read().tls_acceptor.accept(client)).await?;

        let reader = LinesCodec::new().framed(stream);
        let (sink, stream) = reader.split();
        let mut sink = Compat01As03Sink::<_, String>::new(sink);
        let mut stream = Compat01As03::new(stream);
        let mut state = State::default();

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
                }
                LineResponse::Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

#[derive(Default, Debug)]
pub struct State {
    pub from: String,
    pub recipient: Vec<String>,
    pub body: String,
    ehlo_received: bool,
    is_reading_body: bool,
}

type StateFn = &'static (Sync + Fn(&mut State, MessageParser) -> LineResponse);

static SMTP_COMMANDS: phf::Map<&'static [u8; 4], StateFn> = phf::phf_map! {
    b"EHLO" => &handle_ehlo,
    b"MAIL" => &handle_mail,
    b"RCPT" => &handle_recipient,
    b"SIZE" => &handle_size,
    b"DATA" => &handle_data,
    b"VRFY" => &handle_verify,
    b"TURN" => &handle_turn,
    b"AUTH" => &handle_auth,
    b"RSET" => &handle_reset,
    b"EXPN" => &handle_expn,
    b"HELP" => &handle_help,
    b"QUIT" => &handle_quit,
};

fn handle_ehlo(state: &mut State, mut parser: MessageParser) -> LineResponse {
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
            let word = UniCase::ascii(word);
            if word.eq(&UniCase::ascii("FROM")) {
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
            let word = UniCase::ascii(word);
            if word.eq(&UniCase::ascii("TO")) {
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

fn handle_data(state: &mut State, mut parser: MessageParser) -> LineResponse {
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

fn handle_reset(state: &mut State, mut parser: MessageParser) -> LineResponse {
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
    "221 Come back soon!".into()
}

const SPACE: u8 = b' ';
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
        } else if msg.get(..8) == Some("STARTTLS") {
            LineResponse::Upgrade
        } else if let Some(bytes) = msg.as_bytes().get(..4) {
            let bytes: &[u8; 4] = arrayref::array_ref![bytes, 0, 4];
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
    Err(failure::Error),
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
