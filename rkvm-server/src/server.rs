use quinn::{ConnectionError, Endpoint, Incoming, SendDatagramError, ServerConfig};
use rkvm_input::abs::{AbsAxis, AbsInfo};
use rkvm_input::event::Event;
use rkvm_input::key::{Key, KeyEvent};
use rkvm_input::monitor::Monitor;
use rkvm_input::rel::RelAxis;
use rkvm_input::sync::SyncEvent;
use rkvm_net::auth::{AuthChallenge, AuthResponse, AuthStatus};
use rkvm_net::message::Message;
use rkvm_net::version::Version;
use rkvm_net::{Datagram, DeviceInfo};
use slab::Slab;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::CString;
use std::io::{self, ErrorKind};
use std::iter;
use std::net::SocketAddr;
use thiserror::Error;
use tokio::io::{AsyncWrite, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::Instrument;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(io::Error),
    #[error("Input error: {0}")]
    Input(io::Error),
    #[error("Event queue overflow")]
    Overflow,
}

pub async fn run(
    listen: SocketAddr,
    mut config: ServerConfig,
    password: &str,
    switch_keys: &HashSet<Key>,
    propagate_switch_keys: bool,
    enable_datagrams: bool,
) -> Result<(), Error> {
    config.transport_config(rkvm_net::transport_config().into());

    let endpoint = Endpoint::server(config, listen).map_err(Error::Network)?;
    tracing::info!("Listening on {}", listen);

    let (connection_sender, mut connection_receiver) = mpsc::channel(1);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                connection = endpoint.accept() => {
                    let connection = match connection {
                        Some(connection) => connection,
                        None => break,
                    };

                    if connection_sender.send(connection).await.is_err() {
                        break;
                    }
                }
                _ = connection_sender.closed() => break,
            }
        }
    });

    let mut monitor = Monitor::new();
    let mut id = 0usize;
    let mut devices = HashMap::<usize, Device>::new();
    let mut clients = Slab::<(Sender<_>, SocketAddr)>::new();
    let mut current = 0;
    let mut previous = 0;
    let mut changed = false;
    let mut pressed_keys = HashSet::new();

    let (events_sender, mut events_receiver) = mpsc::channel(1);
    loop {
        let event = async { events_receiver.recv().await.unwrap() };

        tokio::select! {
            connection = connection_receiver.recv() => {
                let connection = match connection {
                    Some(connection) => connection,
                    None => break,
                };

                let addr = connection.remote_address();
                let password = password.to_owned();

                // Remove dead clients.
                clients.retain(|_, (client, _)| !client.is_closed());
                if !clients.contains(current) {
                    current = 0;
                }

                let init_updates = devices
                    .iter()
                    .map(|(id, device)| Update::CreateDevice {
                        id: *id,
                        name: device.name.clone(),
                        version: device.version,
                        vendor: device.vendor,
                        product: device.product,
                        rel: device.rel.clone(),
                        abs: device.abs.clone(),
                        keys: device.keys.clone(),
                        delay: device.delay,
                        period: device.period,
                    })
                    .collect();

                let (sender, receiver) = mpsc::channel(1);
                clients.insert((sender, addr));

                let span = tracing::info_span!("client", addr = %addr);

                tokio::spawn(
                    async move {
                        tracing::info!("Connected");

                        match client(init_updates, receiver, connection, &password, enable_datagrams).await {
                            Ok(()) => tracing::info!("Disconnected"),
                            Err(err) => tracing::error!("Disconnected: {}", err),
                        }
                    }
                    .instrument(span),
                );
            }
            interceptor = monitor.read() => {
                let mut interceptor = interceptor.map_err(Error::Input)?;

                id = id.checked_add(1).unwrap();

                let name = interceptor.name().to_owned();
                let version = interceptor.version();
                let vendor = interceptor.vendor();
                let product = interceptor.product();
                let rel = interceptor.rel().collect::<HashSet<_>>();
                let abs = interceptor.abs().collect::<HashMap<_,_>>();
                let keys = interceptor.key().collect::<HashSet<_>>();
                let repeat = interceptor.repeat();

                for (_, (sender, _)) in &clients {
                    let update = Update::CreateDevice {
                        id,
                        name: name.clone(),
                        version: version.clone(),
                        vendor: vendor.clone(),
                        product: product.clone(),
                        rel: rel.clone(),
                        abs: abs.clone(),
                        keys: keys.clone(),
                        delay: repeat.delay,
                        period: repeat.period,
                    };

                    let _ = sender.send(update).await;
                }

                let (interceptor_sender, mut interceptor_receiver) = mpsc::channel(32);
                devices.insert(id, Device {
                    name,
                    version,
                    vendor,
                    product,
                    rel,
                    abs,
                    keys,
                    delay: repeat.delay,
                    period: repeat.period,
                    sender: interceptor_sender,
                });

                let events_sender = events_sender.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            event = interceptor.read() => {
                                if event.is_err() | events_sender.send((id, event)).await.is_err() {
                                    break;
                                }
                            }
                            event = interceptor_receiver.recv() => {
                                let event = match event {
                                    Some(event) => event,
                                    None => break,
                                };

                                match interceptor.write(&event).await {
                                    Ok(()) => {},
                                    Err(err) => {
                                        let _ = events_sender.send((id, Err(err))).await;
                                        break;
                                    }
                                }

                                tracing::trace!(id = %id, "Wrote an event to device");
                            }
                        }
                    }
                });

                let device = &devices[&id];

                tracing::info!(
                    id = %id,
                    name = ?device.name,
                    vendor = %device.vendor,
                    product = %device.product,
                    version = %device.version,
                    "Registered new device"
                );
            }
            (id, result) = event => match result {
                Ok(event) => {
                    let mut press = false;

                    if let Event::Key(KeyEvent { key, down }) = event {
                        if switch_keys.contains(&key) {
                            press = true;

                            match down {
                                true => pressed_keys.insert(key),
                                false => pressed_keys.remove(&key),
                            };
                        }
                    }

                    // Who to send this event to.
                    let mut idx = current;

                    if press {
                        if pressed_keys.len() == switch_keys.len() {
                            let exists = |idx| idx == 0 || clients.contains(idx - 1);
                            loop {
                                current = (current + 1) % (clients.len() + 1);
                                if exists(current) {
                                    break;
                                }
                            }

                            previous = idx;
                            changed = true;

                            if current != 0 {
                                tracing::info!(idx = %current, addr = %clients[current - 1].1, "Switched client");
                            } else {
                                tracing::info!(idx = %current, "Switched client");
                            }
                        } else if changed {
                            idx = previous;

                            if pressed_keys.is_empty() {
                                changed = false;
                            }
                        }
                    }

                    if press && !propagate_switch_keys {
                        continue;
                    }

                    let events = [event]
                        .into_iter()
                        .chain(press.then_some(Event::Sync(SyncEvent::All)));

                    // Index 0 - special case to keep the modular arithmetic above working.
                    if idx == 0 {
                        // We do a try_send() here rather than a "blocking" send in order to prevent deadlocks.
                        // In this scenario, the interceptor task is sending events to the main task,
                        // while the main task is simultaneously sending events back to the interceptor.
                        // This creates a classic deadlock situation where both tasks are waiting for each other.
                        let sender = &devices[&id].sender;
                        for event in events {
                            match sender.try_send(event) {
                                Ok(()) | Err(TrySendError::Closed(_)) => {},
                                Err(TrySendError::Full(_)) => return Err(Error::Overflow),
                            }
                        }

                        continue;
                    }

                    for event in events {
                        if clients[idx - 1].0.send(Update::Event { id, event }).await.is_err() {
                            clients.remove(idx - 1);

                            if current == idx {
                                current = 0;
                            }
                        }
                    }
                }
                Err(err) if err.kind() == ErrorKind::BrokenPipe => {
                    for (_, (sender, _)) in &clients {
                        let _ = sender.send(Update::DestroyDevice { id }).await;
                    }
                    devices.remove(&id);

                    tracing::info!(id = %id, "Destroyed device");
                }
                Err(err) => return Err(Error::Input(err)),
            }
        }
    }

    Ok(())
}
enum Update {
    CreateDevice {
        id: usize,
        name: CString,
        vendor: u16,
        product: u16,
        version: u16,
        rel: HashSet<RelAxis>,
        abs: HashMap<AbsAxis, AbsInfo>,
        keys: HashSet<Key>,
        delay: Option<i32>,
        period: Option<i32>,
    },
    DestroyDevice {
        id: usize,
    },
    Event {
        id: usize,
        event: Event,
    },
}
struct Device {
    name: CString,
    vendor: u16,
    product: u16,
    version: u16,
    rel: HashSet<RelAxis>,
    abs: HashMap<AbsAxis, AbsInfo>,
    keys: HashSet<Key>,
    delay: Option<i32>,
    period: Option<i32>,
    sender: Sender<Event>,
}

#[derive(Error, Debug)]
enum ClientError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("Incompatible client version (got {client}, expected {server})")]
    Version { server: Version, client: Version },
    #[error("Invalid password")]
    Auth,
    #[error(transparent)]
    Rand(#[from] rand::Error),
    #[error(transparent)]
    Connection(#[from] ConnectionError),
}

async fn client(
    mut init_updates: VecDeque<Update>,
    mut receiver: Receiver<Update>,
    connection: Incoming,
    password: &str,
    mut enable_datagrams: bool,
) -> Result<(), ClientError> {
    let connection = connection.await?;

    let (data_write, data_read) = connection.open_bi().await?;

    let mut data_write = BufWriter::new(data_write);
    let mut data_read = BufReader::new(data_read);

    Version::CURRENT.encode(&mut data_write).await?;
    data_write.flush().await?;

    let version = Version::decode(&mut data_read).await?;
    if version != Version::CURRENT {
        return Err(ClientError::Version {
            server: Version::CURRENT,
            client: version,
        });
    }

    let challenge = AuthChallenge::generate().await?;

    challenge.encode(&mut data_write).await?;
    data_write.flush().await?;

    let response = AuthResponse::decode(&mut data_read).await?;

    let status = match response.verify(&challenge, password) {
        true => AuthStatus::Passed,
        false => AuthStatus::Failed,
    };

    status.encode(&mut data_write).await?;
    data_write.flush().await?;

    if status == AuthStatus::Failed {
        return Err(ClientError::Auth);
    }

    tracing::info!("Authenticated successfully");

    let mut senders = HashMap::new();
    let (error_sender, mut error_receiver) = mpsc::channel(1);

    data_write.shutdown().await?;

    let mut datagram_events = Vec::new();

    loop {
        let update = async {
            match init_updates.pop_front() {
                Some(update) => Some(update),
                None => receiver.recv().await,
            }
        };

        let update = tokio::select! {
            update = update => update,
            err = connection.closed() => return Err(err.into()),
            err = error_receiver.recv() => return Err(err.unwrap()),
        };

        let update = match update {
            Some(update) => update,
            None => break,
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
                let write = connection.open_uni().await?;
                let stream_id = write.id();

                let mut write = BufWriter::new(write);

                let device_info = DeviceInfo {
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
                };

                let (device_sender, stream_receiver) = mpsc::channel(1);
                senders.insert(id, device_sender);

                let error_sender = error_sender.clone();
                let span = tracing::debug_span!("stream", id = %stream_id);

                tokio::spawn(
                    async move {
                        tracing::debug!("Stream connected");

                        match stream(&mut write, &device_info, stream_receiver).await {
                            Ok(()) => tracing::debug!("Stream disconnected"),
                            Err(err) => {
                                tracing::debug!("Stream disconnected: {}", err);
                                let _ = error_sender.send(err).await;
                            }
                        }
                    }
                    .instrument(span),
                );
            }
            Update::DestroyDevice { id } => {
                senders.remove(&id).unwrap();
            }
            Update::Event { id, event } => match event {
                Event::Rel(_) if enable_datagrams => {
                    datagram_events.push(event);
                }
                Event::Sync(SyncEvent::All) if enable_datagrams && !datagram_events.is_empty() => {
                    datagram_events.push(event);

                    let mut message = Vec::new();
                    Datagram {
                        id,
                        events: datagram_events.as_slice().into(),
                    }
                    .encode(&mut message)
                    .await?;

                    let length = message.len();

                    let err = match connection.send_datagram(message.into()) {
                        Ok(()) => {
                            tracing::trace!(
                                "Wrote {} unreliable event{}",
                                datagram_events.len(),
                                if datagram_events.len() == 1 { "" } else { "s" }
                            );

                            datagram_events.clear();
                            continue;
                        }
                        Err(err) => err,
                    };

                    match err {
                        SendDatagramError::UnsupportedByPeer
                        | SendDatagramError::Disabled
                        | SendDatagramError::TooLarge => {
                            let sender = &senders[&id];
                            for event in datagram_events.drain(..) {
                                let _ = sender.send(event).await;
                            }

                            if matches!(err, SendDatagramError::TooLarge) {
                                tracing::warn!(length = %length, "Datagram too large");
                            } else {
                                tracing::warn!("Disabling datagram support: {}", err);
                                enable_datagrams = false;
                            }
                        }
                        SendDatagramError::ConnectionLost(err) => return Err(err.into()),
                    }
                }
                _ => {
                    // Send only consecutive relative events as datagrams.
                    let sender = &senders[&id];
                    for event in datagram_events.drain(..).chain(iter::once(event)) {
                        let _ = sender.send(event).await;
                    }
                }
            },
        }
    }

    Ok(())
}

async fn stream<T: AsyncWrite + Send + Unpin>(
    write: &mut T,
    device_info: &DeviceInfo,
    mut receiver: Receiver<Event>,
) -> Result<(), ClientError> {
    let span = tracing::info_span!("device", id = device_info.id);

    async {
        device_info.encode(write).await?;
        write.flush().await?;

        let mut events = 0usize;

        while let Some(event) = receiver.recv().await {
            event.encode(write).await?;
            events += 1;

            // Coalesce multiple events into a single QUIC packet.
            // The `Interceptor` won't emit them until it receives a sync event anyway.
            if let Event::Sync(_) = event {
                write.flush().await?;

                tracing::trace!(
                    "Wrote {} event{}",
                    events,
                    if events == 1 { "" } else { "s" }
                );

                events = 0;
            }
        }

        write.shutdown().await?;

        Ok(())
    }
    .instrument(span)
    .await
}
