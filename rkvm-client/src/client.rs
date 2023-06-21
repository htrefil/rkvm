use rkvm_input::writer::Writer;
use rkvm_net::auth::{AuthChallenge, AuthStatus};
use rkvm_net::message::Message;
use rkvm_net::version::Version;
use rkvm_net::Update;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
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
    #[error("Invalid password")]
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

    stream.set_linger(None).map_err(Error::Network)?;
    stream.set_nodelay(false).map_err(Error::Network)?;

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

    let mut writers = HashMap::new();

    loop {
        match Update::decode(&mut stream).await.map_err(Error::Network)? {
            Update::CreateDevice {
                id,
                name,
                vendor,
                product,
                version,
                rel,
                abs,
                keys,
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
                        .build()
                        .await
                }
                .await
                .map_err(Error::Input)?;

                entry.or_insert(writer);

                log::info!(
                    "Created new device {} (name {:?}, vendor {}, product {}, version {})",
                    id,
                    name,
                    vendor,
                    product,
                    version
                );
            }
            Update::DestroyDevice { id } => {
                if writers.remove(&id).is_none() {
                    return Err(Error::Network(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Server destroyed a nonexistent device",
                    )));
                }

                log::info!("Destroyed device {}", id);
            }
            Update::Event { id, event } => {
                let writer = writers.get_mut(&id).ok_or_else(|| {
                    Error::Network(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Server sent an event to a nonexistent device",
                    ))
                })?;

                writer.write(&event).await.map_err(Error::Input)?;

                log::trace!("Wrote an event to device {}", id);
            }
        }
    }
}
