use rkvm_config::Timeout;
use rkvm_input::writer::Writer;
use rkvm_net::auth::{AuthChallenge, AuthStatus};
use rkvm_net::message::Message;
use rkvm_net::version::Version;
use rkvm_net::{Pong, Update};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io;
use std::time::Instant;
use thiserror::Error;
use tokio::io::{AsyncWriteExt, BufStream};
use tokio::net::TcpStream;
use tokio::time;
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
    #[error("Invalid password")]
    Auth,
}

pub async fn run(
    hostname: &ServerName,
    port: u16,
    connector: TlsConnector,
    password: &str,
    timeout: Timeout,
) -> Result<(), Error> {
    // Intentionally don't impose any timeout for TCP connect.
    let stream = match hostname {
        ServerName::DnsName(name) => TcpStream::connect(&(name.as_ref(), port)).await,
        ServerName::IpAddress(address) => TcpStream::connect(&(*address, port)).await,
        _ => unimplemented!("Unhandled rustls ServerName variant: {:?}", hostname),
    }
    .map_err(Error::Network)?;

    tracing::info!("Connected to server");

    let stream = rkvm_net::timeout(timeout.tls, connector.connect(hostname.clone(), stream))
        .await
        .map_err(Error::Network)?;

    tracing::info!("TLS connected");

    let mut stream = BufStream::with_capacity(1024, 1024, stream);

    rkvm_net::timeout(timeout.write, async {
        Version::CURRENT.encode(&mut stream).await?;
        stream.flush().await?;

        Ok(())
    })
    .await
    .map_err(Error::Network)?;

    let version = rkvm_net::timeout(timeout.read, Version::decode(&mut stream))
        .await
        .map_err(Error::Network)?;

    if version != Version::CURRENT {
        return Err(Error::Version {
            server: Version::CURRENT,
            client: version,
        });
    }

    let challenge = rkvm_net::timeout(timeout.read, AuthChallenge::decode(&mut stream))
        .await
        .map_err(Error::Network)?;

    let response = challenge.respond(password);

    rkvm_net::timeout(timeout.write, async {
        response.encode(&mut stream).await?;
        stream.flush().await?;

        Ok(())
    })
    .await
    .map_err(Error::Network)?;

    let status = rkvm_net::timeout(timeout.read, AuthStatus::decode(&mut stream))
        .await
        .map_err(Error::Network)?;

    match status {
        AuthStatus::Passed => {}
        AuthStatus::Failed => return Err(Error::Auth),
    }

    tracing::info!("Authenticated successfully");

    let mut start = Instant::now();

    let mut interval = time::interval(rkvm_net::PING_INTERVAL + timeout.read);
    let mut writers = HashMap::new();

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
                vendor,
                product,
                version,
                rel,
                abs,
                keys,
                delay,
                period,
            } => {
                let entry = writers.entry(id);
                if let Entry::Occupied(_) = entry {
                    return Err(Error::Network(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Server created the same device twice",
                    )));
                }

                let writer = async {
                    Writer::builder()?
                        .name(&name)
                        .vendor(vendor)
                        .product(product)
                        .version(version)
                        .rel(rel)?
                        .abs(abs)?
                        .key(keys)?
                        .delay(delay)?
                        .period(period)?
                        .build()
                        .await
                }
                .await
                .map_err(Error::Input)?;

                entry.or_insert(writer);

                tracing::info!(
                    id = %id,
                    name = ?name,
                    vendor = %vendor,
                    product = %product,
                    version = %version,
                    "Created new device"
                );
            }
            Update::DestroyDevice { id } => {
                if writers.remove(&id).is_none() {
                    return Err(Error::Network(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Server destroyed a nonexistent device",
                    )));
                }

                tracing::info!(id = %id, "Destroyed device");
            }
            Update::Event { id, event } => {
                let writer = writers.get_mut(&id).ok_or_else(|| {
                    Error::Network(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Server sent an event to a nonexistent device",
                    ))
                })?;

                writer.write(&event).await.map_err(Error::Input)?;

                tracing::trace!(id = %id, "Wrote an event to device");
            }
            Update::Ping => {
                let duration = start.elapsed();
                tracing::debug!(duration = ?duration, "Received ping");

                start = Instant::now();
                interval.reset();

                rkvm_net::timeout(timeout.write, async {
                    Pong.encode(&mut stream).await?;
                    stream.flush().await?;

                    Ok(())
                })
                .await
                .map_err(Error::Network)?;

                let duration = start.elapsed();
                tracing::debug!(duration = ?duration, "Sent pong");
            }
        }
    }
}
