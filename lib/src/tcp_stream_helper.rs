use runtime::net::TcpStream;
use std::borrow::Cow;

#[macro_export]
macro_rules! log_and_send {
    ($client:expr, $addr:expr, $msg:expr) => {
        let str = $msg;
        log::trace!("[{}] OUT: {}", $addr, str);
        $client.send(str.as_bytes().to_vec()).await?;
        $client.send(b"\r\n".to_vec()).await?;
    };
    ($client:expr, $addr:expr, $msg:expr $(, $arg:expr)*) => {
        let str: String = format!($msg $(, $arg)*);
        log::trace!("[{}] OUT: {}", $addr, str);
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
