use rkvm_net::auth::{AuthChallenge, AuthStatus};
use rkvm_net::message::Message;
use rkvm_net::version::Version;
use rkvm_net::{Pong, Update};
use std::io;
use std::net::IpAddr;
use std::time::Instant;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufStream};
use tokio::net::TcpStream;
use tokio::time;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::TlsConnector;

use crate::output::{Device, Output};

#[derive(Error, Debug)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(io::Error),
    #[error("Input error: {0}")]
    Input(io::Error),
    #[error("Incompatible server version (got {server}, expected {client})")]
    Version { server: Version, client: Version },
    #[error("Invalid password")]
    Auth,
}

// The server sent something that doesn't make sense.
pub fn protocol_error(message: &str) -> Error {
    Error::Network(io::Error::new(io::ErrorKind::InvalidData, message))
}

pub async fn run<T: Output>(
    hostname: &ServerName<'static>,
    port: u16,
    connector: Option<TlsConnector>,
    password: &str,
    output: &mut T,
) -> Result<!, Error> {
    // Intentionally don't impose any timeout for TCP connect.
    let stream = match hostname {
        ServerName::DnsName(name) => TcpStream::connect(&(name.as_ref(), port)).await,
        ServerName::IpAddress(address) => TcpStream::connect(&(IpAddr::from(*address), port)).await,
        _ => unimplemented!("Unhandled rustls ServerName variant: {:?}", hostname),
    }
    .map_err(Error::Network)?;

    tracing::info!("Connected to server");

    match connector {
        Some(connector) => {
            let stream = rkvm_net::timeout(
                rkvm_net::TLS_TIMEOUT,
                connector.connect(hostname.clone(), stream),
            )
            .await
            .map_err(Error::Network)?;

            tracing::info!("TLS connected");

            session(stream, password, output).await
        }
        None => session(stream, password, output).await,
    }
}

async fn session<S: AsyncRead + AsyncWrite + Send + Unpin, T: Output>(
    stream: S,
    password: &str,
    output: &mut T,
) -> Result<!, Error> {
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
            server: version,
            client: Version::CURRENT,
        });
    }

    let challenge = rkvm_net::timeout(rkvm_net::READ_TIMEOUT, AuthChallenge::decode(&mut stream))
        .await
        .map_err(Error::Network)?;

    let response = challenge.respond(password);

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

    let mut start = Instant::now();

    let mut interval = time::interval(rkvm_net::PING_INTERVAL + rkvm_net::READ_TIMEOUT);

    // Interval ticks immediately after creation.
    interval.tick().await;

    loop {
        let update = tokio::select! {
            update = Update::decode(&mut stream) => update.map_err(Error::Network)?,
            _ = interval.tick() => return Err(Error::Network(io::Error::new(io::ErrorKind::TimedOut, "Ping timed out"))),
        };

        match update {
            Update::CreateDevice {
                id,
                name,
                bus_type,
                vendor,
                product,
                version,
                rel,
                abs,
                keys,
                properties,
                delay,
                period,
            } => {
                output
                    .create_device(
                        id,
                        Device {
                            name: name.clone(),
                            bus_type,
                            vendor,
                            product,
                            version,
                            rel,
                            abs,
                            keys,
                            properties,
                            delay,
                            period,
                        },
                    )
                    .await?;

                tracing::info!(
                    %id,
                    ?name,
                    %vendor,
                    %product,
                    %version,
                    "Created new device"
                );
            }
            Update::DestroyDevice { id } => {
                output.destroy_device(id).await?;

                tracing::info!(%id, "Destroyed device");
            }
            Update::Event { id, event } => {
                output.write(id, &event).await?;

                tracing::trace!(%id, "Wrote an event to device");
            }
            Update::Ping => {
                let duration = start.elapsed();
                tracing::debug!(?duration, "Received ping");

                start = Instant::now();
                interval.reset();

                rkvm_net::timeout(rkvm_net::WRITE_TIMEOUT, async {
                    Pong.encode(&mut stream).await?;
                    stream.flush().await?;

                    Ok(())
                })
                .await
                .map_err(Error::Network)?;

                let duration = start.elapsed();
                tracing::debug!(?duration, "Sent pong");
            }
        }
    }
}
