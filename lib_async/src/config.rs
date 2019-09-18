use native_tls::{Identity, TlsAcceptor as SyncTlsAcceptor};
use std::borrow::Cow;
use std::sync::Arc;
use tokio_tls::TlsAcceptor as AsyncTlsAcceptor;

#[derive(Clone)]
pub struct Config {
    pub(crate) max_receive_length: usize,
    pub(crate) hostname: String,
    pub(crate) mail_server_name: String,
    pub(crate) tls_acceptor: Option<Arc<AsyncTlsAcceptor>>,

    pub(crate) capabilities: Vec<Capability>,
}

impl Config {
    pub(crate) fn has_capability(&self, capability: Capability) -> bool {
        self.capabilities.iter().any(|c| *c == capability)
    }
}

pub struct ConfigBuilder {
    config: Config,
}

impl Default for ConfigBuilder {
    fn default() -> ConfigBuilder {
        ConfigBuilder {
            config: Config {
                max_receive_length: usize::max_value(),
                hostname: String::from("smtp.example.com"),
                mail_server_name: String::from("Rusty SMTP server"),
                tls_acceptor: None,
                capabilities: vec![],
            },
        }
    }
}

impl ConfigBuilder {
    pub fn with_pkcs12_certificate(
        mut self,
        file: impl AsRef<std::path::Path>,
        password: impl AsRef<str>,
    ) -> Result<ConfigBuilder, ConfigBuilderTlsError> {
        use std::io::Read;
        let mut file = std::fs::File::open(file).map_err(ConfigBuilderTlsError::Io)?;
        let mut identity = vec![];
        file.read_to_end(&mut identity)
            .map_err(ConfigBuilderTlsError::Io)?;

        let identity = Identity::from_pkcs12(&identity, password.as_ref())
            .map_err(ConfigBuilderTlsError::NativeTls)?;
        let tls_acceptor =
            SyncTlsAcceptor::new(identity).map_err(ConfigBuilderTlsError::NativeTls)?;
        self.config.tls_acceptor = Some(Arc::new(tls_acceptor.into()));
        self.config.capabilities.push(Capability::StartTls);

        Ok(self)
    }

    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.config.hostname = hostname.into();
        self
    }

    pub fn with_server_name(mut self, mail_server_name: impl Into<String>) -> Self {
        self.config.mail_server_name = mail_server_name.into();
        self
    }

    pub fn with_max_size(mut self, max_size: usize) -> Self {
        self.config.max_receive_length = max_size;
        self.config.capabilities.push(Capability::Size);
        self
    }

    pub fn build(self) -> Config {
        self.config
    }
}

#[derive(Debug)]
pub enum ConfigBuilderTlsError {
    Io(std::io::Error),
    NativeTls(native_tls::Error),
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

impl Capability {
    pub(crate) fn to_cow_str(&self, config: &Config) -> Cow<'static, str> {
        match self {
            Capability::Size => format!("SIZE {}", config.max_receive_length).into(),
            Capability::StartTls => "STARTTLS".into(),
            Capability::SmtpUtf8 => "SMTPUTF8".into(),
            _ => unimplemented!(),
        }
    }
}
