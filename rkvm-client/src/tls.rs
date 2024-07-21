use quinn::crypto::rustls::{NoInitialCipherSuite, QuicClientConfig};
use quinn::rustls;
use quinn::rustls::RootCertStore;
use quinn::ClientConfig;
use std::io;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rustls(#[from] rustls::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    NoInitialCipherSuite(#[from] NoInitialCipherSuite),
}

pub async fn configure(certificate: &Path) -> Result<ClientConfig, Error> {
    let certificate = fs::read(certificate).await?;
    let certificate = rustls_pemfile::certs(&mut &*certificate).collect::<Result<Vec<_>, _>>()?;

    let mut store = RootCertStore::empty();
    for certificate in certificate {
        store.add(certificate)?;
    }

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(store)
        .with_no_client_auth();

    let config = QuicClientConfig::try_from(config)?;
    let config = Arc::new(config);
    let config = ClientConfig::new(config);

    Ok(config)
}
