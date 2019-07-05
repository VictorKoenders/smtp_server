pub struct Config {
    pub host: String,
    pub tls_acceptor: tokio_tls::TlsAcceptor,
    pub features: Vec<ConfigFeature>,
}

pub enum ConfigFeature {
    Auth(String),
    Tls,
}
