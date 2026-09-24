use std::io;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use tokio_rustls::rustls::pki_types::pem::{self, PemObject};
use tokio_rustls::rustls::pki_types::CertificateDer;
use tokio_rustls::rustls::{self, ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rustls(#[from] rustls::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Pem(#[from] pem::Error),
}

pub async fn configure(certificate: &Path) -> Result<TlsConnector, Error> {
    let certificate = fs::read(certificate).await?;

    let mut store = RootCertStore::empty();
    for certificate in CertificateDer::pem_slice_iter(&certificate) {
        store.add(certificate?)?;
    }

    let config = Arc::new(
        ClientConfig::builder()
            .with_root_certificates(store)
            .with_no_client_auth(),
    );

    Ok(config.into())
}
