use std::borrow::Cow;
/*use std::path::Path;
use std::io::Read;
use std::fs::File;
use failure::{ResultExt, format_err};*/

#[derive(Clone, Debug)]
pub struct Config {
    pub(crate) host: String,
    pub(crate) max_size: usize,
    // pub tls_acceptor: Option<tokio_tls::TlsAcceptor>,
    pub(crate) features: Vec<ConfigFeature>,
}

impl Config {
    pub fn build(host: impl Into<String>) -> ConfigBuilder {
        ConfigBuilder {
            host: host.into(),
            max_size: 4 * 1024 * 1024, // 4MB
            ..Default::default()
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ConfigFeature {
    Auth(String),
    Tls,
}

impl ConfigFeature {
    pub fn as_ehlo_tag(&self) -> Option<Cow<'static, str>> {
        match self {
            ConfigFeature::Auth(s) => Some(format!("AUTH {}", s).into()),
            ConfigFeature::Tls => Some("STARTTLS".into()),
        }
    }
}

#[derive(Default)]
pub struct ConfigBuilder {
    host: String,
    max_size: usize,
    // tls_acceptor: Option<tokio_tls::TlsAcceptor>,
    features: Vec<ConfigFeature>,
}

impl ConfigBuilder {
    /*pub fn with_tls_from_pfx(mut self, file: impl AsRef<Path>) -> Result<ConfigBuilder, failure::Error> {
        let file = file.as_ref();
        let mut file = File::open(file).with_context(|e| format_err!("Could not load {:?}: {:?}", file, e))?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .with_context(|e| format_err!("Could not read {:?}: {:?}", file, e))?;
        drop(file);

        let acceptor = native_tls::TlsAcceptor::new(
            native_tls::Identity::from_pkcs12(&contents, "").with_context(|e| format_err!("Could not parse {:?}: {:?}", file, e))?,
        )
        .context("Could not create a TLS Acceptor")?;
        let acceptor = tokio_tls::TlsAcceptor::from(acceptor);

        self.features.push(ConfigFeature::Tls);
        self.tls_acceptor = Some(acceptor);

        Ok(self)
    }*/

    pub fn max_message_size_kb(mut self, max_size_kb: usize) -> Self {
        self.max_size = max_size_kb * 1024;
        self
    }
    pub fn max_message_size_mb(mut self, max_size_mb: usize) -> Self {
        self.max_size = max_size_mb * 1024 * 1024;
        self
    }

    pub fn build(self) -> Config {
        Config {
            host: self.host,
            max_size: self.max_size,
            // tls_acceptor: self.tls_acceptor,
            features: self.features,
        }
    }
}
