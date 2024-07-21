use quinn::ClientConfig;
use quinn::ConnectError;
use quinn::Connection;
use quinn::ConnectionError;
use quinn::Endpoint;
use rkvm_input::event::Event;
use rkvm_input::writer::Writer;
use rkvm_net::auth::{AuthChallenge, AuthStatus};
use rkvm_net::message::Message;
use rkvm_net::version::Version;
use rkvm_net::Datagram;
use rkvm_net::DeviceInfo;
use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use thiserror::Error;
use tokio::io::AsyncRead;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::BufWriter;
use tokio::net;
use tokio::sync::mpsc::{self, Sender};
use tracing::Instrument;
use tracing::Span;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),
    #[error("Input error: {0}")]
    Input(io::Error),
    #[error("Incompatible server version (got {server}, expected {client})")]
    Version { server: Version, client: Version },
    #[error("Invalid password")]
    Auth,
}

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Connect(#[from] ConnectError),
    #[error(transparent)]
    Connection(#[from] ConnectionError),
}

pub async fn run(
    hostname: &str,
    port: u16,
    mut config: ClientConfig,
    password: &str,
) -> Result<(), Error> {
    config.transport_config(rkvm_net::transport_config().into());

    let connection = connect(hostname, port, config).await?;
    let (data_write, data_read) = connection.accept_bi().await.map_err(NetworkError::from)?;

    let mut data_write = BufWriter::new(data_write);
    let mut data_read = BufReader::new(data_read);

    Version::CURRENT
        .encode(&mut data_write)
        .await
        .map_err(NetworkError::from)?;
    data_write.flush().await.map_err(NetworkError::from)?;

    let version = Version::decode(&mut data_read)
        .await
        .map_err(NetworkError::from)?;

    if version != Version::CURRENT {
        return Err(Error::Version {
            server: Version::CURRENT,
            client: version,
        });
    }

    let challenge = AuthChallenge::decode(&mut data_read)
        .await
        .map_err(NetworkError::from)?;

    let response = challenge.respond(password);

    response
        .encode(&mut data_write)
        .await
        .map_err(NetworkError::from)?;
    data_write.flush().await.map_err(NetworkError::from)?;

    let status = AuthStatus::decode(&mut data_read)
        .await
        .map_err(NetworkError::from)?;

    match status {
        AuthStatus::Passed => {}
        AuthStatus::Failed => return Err(Error::Auth),
    }

    tracing::info!("Authenticated successfully");

    data_write.shutdown().await.map_err(NetworkError::from)?;

    let (error_sender, mut error_receiver) = mpsc::channel(1);
    let (device_sender, mut device_receiver) = mpsc::channel(1);

    let mut devices = HashMap::new();

    loop {
        let device = async { device_receiver.recv().await.unwrap() };
        let read = tokio::select! {
            read = connection.accept_uni() => read.map_err(NetworkError::from)?,
            device = device => {
                match device {
                    DeviceEvent::Create { id, sender } => {
                        if devices.insert(id, sender).is_some() {
                            return Err(NetworkError::from(io::Error::new(io::ErrorKind::AlreadyExists, "Device already exists")).into());
                        }

                        continue;
                    }
                    DeviceEvent::Destroy { id } => {
                        devices.remove(&id).unwrap();
                        continue;
                    }
                }
            }
            datagram = connection.read_datagram() => {
                let datagram = datagram.map_err(NetworkError::from)?;
                let datagram = Datagram::decode(&mut &*datagram).await.map_err(NetworkError::from)?;

                let sender = match devices.get(&datagram.id) {
                    Some(sender) => sender,
                    None => {
                        tracing::warn!(id = %datagram.id, "Received datagram for unknown device");
                        continue;
                    }
                };

                // TODO: Since this is a datagram, do we really want to wait here?
                let _ = sender.send(datagram.events.into_owned()).await;
                continue;
            }
            err = error_receiver.recv() => return Err(err.unwrap()),
        };

        let stream_id = read.id();

        let read = BufReader::new(read);

        let error_sender = error_sender.clone();
        let device_sender = device_sender.clone();
        let span = tracing::debug_span!("stream", id = %stream_id);

        tokio::spawn(
            async move {
                tracing::debug!("Stream connected");

                match stream(read, device_sender).await {
                    Ok(()) => {
                        tracing::debug!("Stream disconnected");
                    }
                    Err(err) => {
                        tracing::debug!("Stream disconnected: {}", err);
                        let _ = error_sender.send(err).await;
                    }
                }
            }
            .instrument(span),
        );
    }
}

async fn connect(
    hostname: &str,
    port: u16,
    config: ClientConfig,
) -> Result<Connection, NetworkError> {
    let mut last_err = None;

    let addrs = net::lookup_host((hostname, port))
        .await
        .map_err(NetworkError::from)?;

    for addr in addrs {
        let bind = match addr {
            SocketAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
            SocketAddr::V6(_) => (Ipv6Addr::UNSPECIFIED, 0).into(),
        };

        let endpoint = match Endpoint::client(bind) {
            Ok(endpoint) => endpoint,
            Err(err) => {
                tracing::debug!(addr = %addr, "Error binding: {}", err);
                last_err = Some(err.into());
                continue;
            }
        };

        let connection = match endpoint.connect_with(config.clone(), addr, hostname) {
            Ok(connection) => connection,
            Err(err) => {
                tracing::debug!(addr = %addr, "Error connecting: {}", err);
                last_err = Some(err.into());
                continue;
            }
        };

        let connection = match connection.await {
            Ok(connection) => connection,
            Err(err) => {
                tracing::debug!(addr = %addr, "Error connecting: {}", err);
                last_err = Some(err.into());
                continue;
            }
        };

        tracing::info!(addr = %addr, "Connected");

        return Ok(connection);
    }

    Err(last_err.unwrap_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "No addresses resolved").into()
    }))
}

enum DeviceEvent {
    Create {
        id: usize,
        sender: Sender<Vec<Event>>,
    },
    Destroy {
        id: usize,
    },
}

async fn stream<T: AsyncRead + Send + Unpin + 'static>(
    mut read: T,
    device_sender: Sender<DeviceEvent>,
) -> Result<(), Error> {
    let device_info = DeviceInfo::decode(&mut read)
        .await
        .map_err(NetworkError::from)?;

    let span = tracing::info_span!("device", id = %device_info.id);
    async {
        let mut writer = build(&device_info).await.map_err(Error::Input)?;

        tracing::info!(
            name = ?device_info.name,
            vendor = %device_info.vendor,
            product = %device_info.product,
            version = %device_info.version,
            "Created new device"
        );

        let (datagram_sender, mut datagram_receiver) = mpsc::channel(1);
        let _ = device_sender
            .send(DeviceEvent::Create {
                id: device_info.id,
                sender: datagram_sender,
            })
            .await;

        let (event_sender, mut event_receiver) = mpsc::channel(1);
        tokio::spawn(
            async move {
                loop {
                    let event = tokio::select! {
                        event = Event::decode(&mut read) => event,
                        _ = event_sender.closed() => break,
                    };

                    if event.is_err() | event_sender.send(event).await.is_err() {
                        break;
                    }
                }
            }
            .instrument(Span::current()),
        );

        let result = async {
            loop {
                let event = async { event_receiver.recv().await.unwrap() };

                tokio::select! {
                    event = event => {
                        let event = event.map_err(NetworkError::from)?;
                        writer.write(&event).await.map_err(Error::Input)?;

                        tracing::trace!("Wrote an event");
                    }
                    datagram = datagram_receiver.recv() => {
                        let datagram = match datagram {
                            Some(datagram) => datagram,
                            None => break,
                        };

                        let length = datagram.len();
                        for event in datagram {
                            writer.write(&event).await.map_err(Error::Input)?;
                        }

                        tracing::trace!(
                            "Wrote {} unreliable event{}",
                            length,
                            if length == 1 { "" } else { "s" }
                        );
                    }
                }
            }

            Ok(())
        }
        .await;

        let _ = device_sender
            .send(DeviceEvent::Destroy { id: device_info.id })
            .await;

        // Drop explicitly to make the log properly ordered.
        drop(writer);
        tracing::info!("Destroyed device");

        result
    }
    .instrument(span)
    .await
}

async fn build(device_info: &DeviceInfo) -> Result<Writer, io::Error> {
    Writer::builder()?
        .name(&device_info.name)
        .vendor(device_info.vendor)
        .product(device_info.product)
        .version(device_info.version)
        .rel(device_info.rel.iter().copied())?
        .abs(device_info.abs.iter().map(|(axis, info)| (*axis, *info)))?
        .key(device_info.keys.iter().copied())?
        .delay(device_info.delay)?
        .period(device_info.period)?
        .build()
        .await
}
