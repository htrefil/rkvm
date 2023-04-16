use rustls_pemfile::Item;
use std::path::Path;
use std::sync::Arc;
use std::{io, iter};
use thiserror::Error;
use tokio::fs;
use tokio_rustls::rustls::{self, Certificate, PrivateKey, ServerConfig};
use tokio_rustls::TlsAcceptor;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rustls(#[from] rustls::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("Multiple private keys provided")]
    MultipleKeys,
    #[error("No suitable private keys provided")]
    NoKeys,
}

pub async fn configure(certificate: &Path, key: &Path) -> Result<TlsAcceptor, Error> {
    enum LoadedItem {
        Certificate(Vec<u8>),
        Key(Vec<u8>),
    }

    let certificate = fs::read_to_string(certificate).await?;
    let key = fs::read_to_string(key).await?;

    let certificates_iter = iter::from_fn({
        let mut buffer = certificate.as_bytes();

        move || rustls_pemfile::read_one(&mut buffer).transpose()
    })
    .filter_map(|item| match item {
        Ok(Item::X509Certificate(data)) => Some(Ok(LoadedItem::Certificate(data))),
        Err(err) => Some(Err(err)),
        _ => None,
    });

    let keys_iter = iter::from_fn({
        let mut buffer = key.as_bytes();

        move || rustls_pemfile::read_one(&mut buffer).transpose()
    })
    .filter_map(|item| match item {
        Ok(Item::RSAKey(data)) | Ok(Item::PKCS8Key(data)) | Ok(Item::ECKey(data)) => {
            Some(Ok(LoadedItem::Key(data)))
        }
        Err(err) => Some(Err(err)),
        _ => None,
    });

    let mut certificates = Vec::new();
    let mut key = None;

    for item in certificates_iter.chain(keys_iter) {
        let item = item?;

        match item {
            LoadedItem::Certificate(data) => certificates.push(Certificate(data)),
            LoadedItem::Key(data) => {
                if key.is_some() {
                    return Err(Error::MultipleKeys);
                }

                key = Some(PrivateKey(data));
            }
        }
    }

    let key = key.ok_or(Error::NoKeys)?;

    ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .map(Arc::new)
        .map(Into::into)
        .map_err(Into::into)
}
