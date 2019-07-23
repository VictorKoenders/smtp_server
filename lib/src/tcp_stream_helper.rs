use runtime::net::TcpStream;
use std::borrow::Cow;

#[macro_export]
macro_rules! log_and_send {
    ($client:expr, $msg:expr) => {
        let str = $msg;
        let ip = crate::tcp_stream_helper::get_ip(&$client);
        log::trace!("[{}] OUT: {}", ip, str);
        $client.write_all(str.as_bytes()).await?;
        $client.write_all(b"\r\n").await?;
    };
    ($client:expr, $msg:expr $(, $arg:expr)*) => {
        let str: String = format!($msg $(, $arg)*);
        let ip = crate::tcp_stream_helper::get_ip(&$client);
        log::trace!("[{}] OUT: {}", ip, str);
        $client.write_all(str.as_bytes()).await?;
        $client.write_all(b"\r\n").await?;
    }
}

pub fn get_ip(stream: &TcpStream) -> Cow<'static, str> {
    stream
        .peer_addr()
        .map(|a| a.to_string().into())
        .unwrap_or_else(|_| "NO_IP".into())
}
