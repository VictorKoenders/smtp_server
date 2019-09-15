#[derive(Clone)]
pub struct Config {
    pub max_receive_length: usize,
    pub hostname: String,
    pub mail_server_name: String,

    pub capabilities: Vec<Capability>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Capability {
    /// Message size declaration, https://tools.ietf.org/html/rfc1870
    ///
    /// Allows the client to declare what the size of the message is going to be
    Size,

    /// Secure SMTP over Transport Layer, https://tools.ietf.org/html/rfc3207
    ///
    /// Allows the client and server to switch to a TLS solution, This will be enabled automatically when you call `Config::set_tls_identity`
    StartTls,

    /// Allow UTF-8 encoding in mailbox names and header fields, https://tools.ietf.org/html/rfc6531
    SmtpUtf8,

    /// 8 bit data transmission, https://tools.ietf.org/html/rfc6152
    ///
    /// Currently not implemented
    #[deprecated]
    EightBitMime,

    /// Authenticated TURN for On-Demand Mail Relay, https://tools.ietf.org/html/rfc2645
    ///
    /// Currently not implemented
    #[deprecated]
    AuthenticatedTurn,

    /// Authenticated SMTP, https://tools.ietf.org/html/rfc4954
    ///
    /// Currently not implemented
    #[deprecated]
    Authentication,

    /// Chunking, https://tools.ietf.org/html/rfc3030
    ///
    /// Currently not implemented
    #[deprecated]
    Chunking,

    /// Delivery status notification, https://tools.ietf.org/html/rfc3461
    ///
    /// Currently not implemented
    #[deprecated]
    DeliveryStatusNotification,

    /// Delivery status notification, https://tools.ietf.org/html/rfc3461
    ///
    /// Currently not implemented
    #[deprecated]
    ExtendedRemoteMessageQueue,

    /// Supply helpful information, https://tools.ietf.org/html/rfc821
    ///
    /// Currently not implemented
    #[deprecated]
    Help,

    /// Command pipelining, https://tools.ietf.org/html/rfc2920
    ///
    /// Currently not implemented
    #[deprecated]
    Pipelining,
}
