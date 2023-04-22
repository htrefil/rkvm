use rkvm_input::{EventPack, EventWriter};
use rkvm_net::auth::{AuthChallenge, AuthStatus};
use rkvm_net::message::Message;
use rkvm_net::version::Version;
use std::io;
use thiserror::Error;
use tokio::io::{AsyncWriteExt, BufStream};
use tokio::net::TcpStream;
use tokio_rustls::rustls::ServerName;
use tokio_rustls::TlsConnector;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(io::Error),
    #[error("Input error: {0}")]
    Input(io::Error),
    #[error("Incompatible server version (got {server}, expected {client})")]
    Version { server: Version, client: Version },
    #[error("Auth challenge failed (possibly wrong password)")]
    Auth,
}

pub async fn run(
    hostname: &ServerName,
    port: u16,
    connector: TlsConnector,
    password: &str,
) -> Result<(), Error> {
    let stream = match hostname {
        ServerName::DnsName(name) => TcpStream::connect(&(name.as_ref(), port))
            .await
            .map_err(Error::Network)?,
        ServerName::IpAddress(address) => TcpStream::connect(&(*address, port))
            .await
            .map_err(Error::Network)?,
        _ => unimplemented!("Unhandled rustls ServerName variant: {:?}", hostname),
    };

    log::info!("Connected to server");

    let stream = connector
        .connect(hostname.clone(), stream)
        .await
        .map_err(Error::Network)?;

    log::info!("TLS connected");

    let mut stream = BufStream::with_capacity(1024, 1024, stream);

    Version::CURRENT
        .encode(&mut stream)
        .await
        .map_err(Error::Network)?;
    stream.flush().await.map_err(Error::Network)?;

    let version = Version::decode(&mut stream).await.map_err(Error::Network)?;
    if version != Version::CURRENT {
        return Err(Error::Version {
            server: Version::CURRENT,
            client: version,
        });
    }

    let challenge = AuthChallenge::decode(&mut stream)
        .await
        .map_err(Error::Network)?;
    let response = challenge.respond(password);

    response.encode(&mut stream).await.map_err(Error::Network)?;
    stream.flush().await.map_err(Error::Network)?;

    match Message::decode(&mut stream).await.map_err(Error::Network)? {
        AuthStatus::Passed => {}
        AuthStatus::Failed => return Err(Error::Auth),
    }

    log::info!("Authenticated successfully");

    let mut writer = EventWriter::new().await.map_err(Error::Input)?;
    loop {
        let events = EventPack::decode(&mut stream)
            .await
            .map_err(Error::Network)?;
        writer.write(&events).await.map_err(Error::Input)?;

        log::trace!(
            "Wrote {} event{}",
            events.len(),
            if events.len() == 1 { "" } else { "s" }
        );
    }
}
