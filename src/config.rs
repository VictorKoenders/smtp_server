/*use std::path::Path;
use std::io::Read;
use std::fs::File;
use failure::{ResultExt, format_err};*/

#[derive(Clone)]
pub struct Config {
    pub host: String,
    // pub tls_acceptor: Option<tokio_tls::TlsAcceptor>,
    pub features: Vec<ConfigFeature>,
}

impl Config {
    pub fn build(host: impl Into<String>) -> ConfigBuilder {
        ConfigBuilder {
            host: host.into(),
            ..Default::default()
        }
    }
}

#[derive(Clone)]
pub enum ConfigFeature {
    Auth(String),
    Tls,
}

#[derive(Default)]
pub struct ConfigBuilder {
    host: String,
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

    pub fn build(self) -> Config {
        Config {
            host: self.host,
            // tls_acceptor: self.tls_acceptor,
            features: self.features,
        }
    }
}
