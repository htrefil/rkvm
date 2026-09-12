use crate::client::Error;
use crate::config::Config;

use rkvm_net::message::Message;
use rkvm_net::auth::{AuthStatus, AuthChallenge};
use rkvm_net::version::Version;

use std::sync::Arc;
use std::path::Path;
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufStream};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::{Certificate, ClientConfig, RootCertStore, ServerName};
use tokio_rustls::TlsConnector;

async fn init_config<P: AsRef<Path> + ?Sized> (path: &P) -> Result<Config,Error> {
    let config = fs::read_to_string(path).await?;
    let config = toml::from_str::<Config>(&config)?;
    return Ok(config);
}

async fn configure_tls(certificate: &Path) -> Result<TlsConnector, Error> {
    let certificate = fs::read(certificate).await?;
    let certificates = rustls_pemfile::certs(&mut certificate.as_slice())?;

    let mut store = RootCertStore::empty();
    for certificate in certificates {
        store.add(&Certificate(certificate))?;
    }

    let config = Arc::new(ClientConfig::builder().with_safe_defaults().with_root_certificates(store).with_no_client_auth(),);

    Ok(config.into())
}

async fn connect(hostname: &ServerName, port: u16) -> Result<TcpStream,Error> {
    // Intentionally don't impose any timeout for TCP connect.
    match hostname {
        ServerName::DnsName(name) => TcpStream::connect(&(name.as_ref(), port)).await,
        ServerName::IpAddress(address) => TcpStream::connect(&(*address, port)).await,
        _ => unimplemented!("Unhandled rustls ServerName variant: {:?}", hostname),
    }.map_err(Error::Network)
}

pub async fn init_stream<P: AsRef<Path> + ?Sized>(config_path: &P) -> Result<BufStream<TlsStream<TcpStream>>,Error> {
    let config = init_config(config_path).await?;
    let connector = configure_tls(&config.certificate).await?;

    tracing::info!("Connected to server");

    let stream = connect(&config.server.hostname, config.server.port).await?;
    let stream = rkvm_net::timeout(
        rkvm_net::TLS_TIMEOUT,
        connector.connect(config.server.hostname.clone(), stream),
    )
    .await
    .map_err(Error::Network)?;

    tracing::info!("TLS connected");

    let mut stream = BufStream::with_capacity(1024, 1024, stream);

    rkvm_net::timeout(rkvm_net::WRITE_TIMEOUT, async {
        Version::CURRENT.encode(&mut stream).await?;
        stream.flush().await?;

        Ok(())
    })
    .await
    .map_err(Error::Network)?;

    let version = rkvm_net::timeout(rkvm_net::READ_TIMEOUT, Version::decode(&mut stream))
        .await
        .map_err(Error::Network)?;

    if version != Version::CURRENT {
        return Err(Error::Version {
            server: Version::CURRENT,
            client: version,
        });
    }

    let challenge = rkvm_net::timeout(rkvm_net::READ_TIMEOUT, AuthChallenge::decode(&mut stream))
        .await
        .map_err(Error::Network)?;

    let response = challenge.respond(&config.password);

    rkvm_net::timeout(rkvm_net::WRITE_TIMEOUT, async {
        response.encode(&mut stream).await?;
        stream.flush().await?;

        Ok(())
    })
    .await
    .map_err(Error::Network)?;

    let status = rkvm_net::timeout(rkvm_net::READ_TIMEOUT, AuthStatus::decode(&mut stream))
        .await
        .map_err(Error::Network)?;

    match status {
        AuthStatus::Passed => {}
        AuthStatus::Failed => return Err(Error::Auth),
    }

    tracing::info!("Authenticated successfully");
    Ok(stream)
}
