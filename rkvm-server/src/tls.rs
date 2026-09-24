use std::io;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use tokio_rustls::rustls::pki_types::pem::{self, PemObject};
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::{self, ServerConfig};
use tokio_rustls::TlsAcceptor;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rustls(#[from] rustls::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Pem(#[from] pem::Error),
    #[error("Multiple private keys provided")]
    MultipleKeys,
    #[error("No suitable private keys provided")]
    NoKeys,
}

pub async fn configure(certificate: &Path, key: &Path) -> Result<TlsAcceptor, Error> {
    let certificate = fs::read(certificate).await?;
    let key = fs::read(key).await?;

    let certificates =
        CertificateDer::pem_slice_iter(&certificate).collect::<Result<Vec<_>, _>>()?;

    let mut keys = PrivateKeyDer::pem_slice_iter(&key);
    let key = keys.next().transpose()?.ok_or(Error::NoKeys)?;

    if keys.next().transpose()?.is_some() {
        return Err(Error::MultipleKeys);
    }

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .map(Arc::new)
        .map(Into::into)
        .map_err(Into::into)
}
