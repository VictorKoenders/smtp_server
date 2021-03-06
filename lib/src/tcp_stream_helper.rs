#[macro_export]
macro_rules! log_and_send {
    ($client:expr, $addr:expr, $msg:expr) => {
        let str = $msg;
        let addr: SocketAddr = $addr;
        log::trace!("[{}] OUT: {}", addr, str);
        $client.send(str.as_bytes().to_vec()).await?;
        $client.send(b"\r\n".to_vec()).await?;
    };
    ($client:expr, $addr:expr, $msg:expr $(, $arg:expr)*) => {
        let str: String = format!($msg $(, $arg)*);
        let addr: SocketAddr = $addr;
        log::trace!("[{}] OUT: {}", addr, str);
        $client.send(str.as_bytes().to_vec()).await?;
        $client.send(b"\r\n".to_vec()).await?;
    }
}
