use quinn::crypto::rustls::{NoInitialCipherSuite, QuicServerConfig};
use quinn::{rustls, ServerConfig};
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
    #[error("No private key provided")]
    NoKeys,
    #[error(transparent)]
    NoInitialCipherSuite(#[from] NoInitialCipherSuite),
}

pub async fn configure(certificate: &Path, key: &Path) -> Result<ServerConfig, Error> {
    let certificate = fs::read(certificate).await?;
    let certificate = rustls_pemfile::certs(&mut &*certificate).collect::<Result<_, _>>()?;

    let key = fs::read(key).await?;
    let key = rustls_pemfile::private_key(&mut &*key)?.ok_or(Error::NoKeys)?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificate, key)?;

    let config = QuicServerConfig::try_from(config)?;
    let config = Arc::new(config);
    let config = ServerConfig::with_crypto(config);

    Ok(config)
}
