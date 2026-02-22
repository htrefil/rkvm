use rkvm_input::abs::{AbsAxis, AbsInfo};
use rkvm_input::event::Event;
use rkvm_input::key::{Key, KeyEvent};
use rkvm_input::monitor::Monitor;
use rkvm_input::rel::RelAxis;
use rkvm_input::sync::SyncEvent;
use rkvm_net::auth::{AuthChallenge, AuthResponse, AuthStatus};
use rkvm_net::message::Message;
use rkvm_net::version::Version;
use rkvm_net::{Pong, Update};
use slab::Slab;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::CString;
use std::io::{self, ErrorKind};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::time::Instant;
use thiserror::Error;
use tokio::io::{AsyncWriteExt, BufStream};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::time;
use tokio_rustls::TlsAcceptor;
use tracing::Instrument;

use crate::config::ClientConfig;

const ADDR_UNKNOWN: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED),0);

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
    acceptor: TlsAcceptor,
    password: &str,
    switch_keys: &HashSet<Key>,
    propagate_switch_keys: bool,
    server_goto_keys: &Option<Vec<Key>>,
    clients_config: &Vec<ClientConfig>,
) -> Result<(), Error> {
    let listener = TcpListener::bind(&listen).await.map_err(Error::Network)?;
    tracing::info!("Listening on {}", listen);

    let mut monitor = Monitor::new();
    let mut devices = Slab::<Device>::new();
    let mut clients = Slab::<Option<(Sender<_>, SocketAddr)>>::new();
    let mut current = 0;
    let mut previous = 0;
    let mut changed = false;
    let mut pressed_keys = HashSet::new();
    let mut all_switch_keys = switch_keys.clone();
    let mut static_client = Vec::new();
    let mut goto_keys: HashMap<Vec<Key>,usize> = HashMap::new();


    if let Some(keys) = server_goto_keys {
         goto_keys.insert(keys.clone(), 0);
         all_switch_keys.extend(keys);
    }

    for c in clients_config {
        clients.insert(None);
        static_client.push(c.addr);
        if let Some(k) = &c.goto_keys {
            let keys:Vec<Key> = k.clone().into_iter().map(Into::into).collect();
            goto_keys.insert(keys.clone(), static_client.len());
            all_switch_keys.extend(keys);
        }
    }

    let (events_sender, mut events_receiver) = mpsc::channel(1);

    loop {
        let event = async { events_receiver.recv().await.unwrap() };

        tokio::select! {
            result = listener.accept() => {
                let (stream, addr) = result.map_err(Error::Network)?;
                let acceptor = acceptor.clone();
                let password = password.to_owned();

                // Remove dead clients.
                for id in 0..static_client.len() {
                    if let Some((client, _)) = &clients[id] {
                        if client.is_closed() {
                            clients[id] = None
                        }
                    }
                }
                clients.retain(|idx, e|
                    if idx < static_client.len() {
                        true
                    } else {
                        match e {
                            Some((client, _)) => !client.is_closed(),
                            None => true,
                        }
                    }
                );
                if current > 0 && (!clients.contains(current) || clients[current].is_none())  {
                    current = 0;
                }

                let init_updates = devices
                    .iter()
                    .map(|(id, device)| Update::CreateDevice {
                        id,
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

                let index = static_client.iter().position(|ip| *ip == addr.ip());
                let idx = match index {
                    Some(idx) => {
                        if clients[idx].is_some() {
                            tracing::warn!("client {} already connected", addr);
                            clients.insert(Some((sender, addr)))
                        } else {
                            clients[idx] = Some((sender, addr));
                            idx
                        }
                    },
                    None => clients.insert(Some((sender, addr))),
                };
                
                let span = tracing::info_span!("connection", addr = %addr, idx = %idx);
                tokio::spawn(
                    async move {
                        tracing::info!("Connected");

                        match client(init_updates, receiver, stream, acceptor, &password).await {
                            Ok(()) => tracing::info!("Disconnected"),
                            Err(err) => tracing::error!("Disconnected: {}", err),
                        }
                    }
                    .instrument(span),
                );
            }
            result = monitor.read() => {
                let mut interceptor = result.map_err(Error::Input)?;

                let name = interceptor.name().to_owned();
                let id = devices.vacant_key();
                let version = interceptor.version();
                let vendor = interceptor.vendor();
                let product = interceptor.product();
                let rel = interceptor.rel().collect::<HashSet<_>>();
                let abs = interceptor.abs().collect::<HashMap<_,_>>();
                let keys = interceptor.key().collect::<HashSet<_>>();
                let repeat = interceptor.repeat();

                for (_, e) in &clients {
                    match e {
                        Some((sender, _)) => {
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
                        },
                        None => {}
                    }
                }

                let (interceptor_sender, mut interceptor_receiver) = mpsc::channel(32);
                devices.insert(Device {
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

                let device = &devices[id];

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
                        if all_switch_keys.contains(&key) {
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
                        let exists = |idx| idx == 0 || clients.get(idx - 1).is_some_and(Option::is_some);

                        // we change in previous event keyup should be send to previous
                        if changed {
                            idx = previous;

                            if pressed_keys.is_empty() {
                                changed = false
                            }
                        } else {
                            for (keys,&i) in &goto_keys {
                                if exists(i) && keys.iter().all(|k| pressed_keys.contains(k)) {
                                    current = i;
                                    changed = true;
                                    break;
                                }
                            }

                            if !changed && switch_keys.is_subset(&pressed_keys) {
                                loop {
                                    current = (current + 1) % (clients.len() + 1);
                                    if exists(current) {
                                        break;
                                    }
                                }

                                changed = true;
                            }
                            if changed {
                                previous = idx;
                                if current != 0 {
                                    tracing::info!(idx = %current, addr = %clients[current - 1].as_ref().map_or_else(|| &ADDR_UNKNOWN, |(_,a)| a), "Switched client");
                                } else {
                                    tracing::info!(idx = %current, "Switched client");
                                }
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
                        for event in events {
                            match devices[id].sender.try_send(event) {
                                Ok(()) | Err(TrySendError::Closed(_)) => {},
                                Err(TrySendError::Full(_)) => return Err(Error::Overflow),
                            }
                        }

                        continue;
                    }

                    for event in events {
                        if let Some((s,_)) = &clients[idx -1] {
                            if s.send(Update::Event { id, event }).await.is_err() {
                                if idx - 1 < static_client.len() {
                                    clients[idx -1] = None
                                } else {
                                    clients.remove(idx - 1);
                                }

                                if current == idx {
                                    current = 0;
                                }
                            }
                        }
                    }
                }
                Err(err) if err.kind() == ErrorKind::BrokenPipe => {
                    for (_, e) in &clients {
                        let _ = match e {
                            Some((sender,_)) => sender.send(Update::DestroyDevice { id }).await,
                            None => Ok(()),
                        };
                    }
                    devices.remove(id);

                    tracing::info!(id = %id, "Destroyed device");
                }
                Err(err) => return Err(Error::Input(err)),
            }
        }
    }
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
}

async fn client(
    mut init_updates: VecDeque<Update>,
    mut receiver: Receiver<Update>,
    stream: TcpStream,
    acceptor: TlsAcceptor,
    password: &str,
) -> Result<(), ClientError> {
    let stream = rkvm_net::timeout(rkvm_net::TLS_TIMEOUT, acceptor.accept(stream)).await?;
    tracing::info!("TLS connected");

    let mut stream = BufStream::with_capacity(1024, 1024, stream);

    rkvm_net::timeout(rkvm_net::WRITE_TIMEOUT, async {
        Version::CURRENT.encode(&mut stream).await?;
        stream.flush().await?;

        Ok(())
    })
    .await?;

    let version = rkvm_net::timeout(rkvm_net::READ_TIMEOUT, Version::decode(&mut stream)).await?;
    if version != Version::CURRENT {
        return Err(ClientError::Version {
            server: Version::CURRENT,
            client: version,
        });
    }

    let challenge = AuthChallenge::generate().await?;

    rkvm_net::timeout(rkvm_net::WRITE_TIMEOUT, async {
        challenge.encode(&mut stream).await?;
        stream.flush().await?;

        Ok(())
    })
    .await?;

    let response =
        rkvm_net::timeout(rkvm_net::READ_TIMEOUT, AuthResponse::decode(&mut stream)).await?;
    let status = match response.verify(&challenge, password) {
        true => AuthStatus::Passed,
        false => AuthStatus::Failed,
    };

    rkvm_net::timeout(rkvm_net::WRITE_TIMEOUT, async {
        status.encode(&mut stream).await?;
        stream.flush().await?;

        Ok(())
    })
    .await?;

    if status == AuthStatus::Failed {
        return Err(ClientError::Auth);
    }

    tracing::info!("Authenticated successfully");

    let mut interval = time::interval(rkvm_net::PING_INTERVAL);

    loop {
        let recv = async {
            match init_updates.pop_front() {
                Some(update) => Some(update),
                None => receiver.recv().await,
            }
        };

        let update = tokio::select! {
            // Make sure pings have priority.
            // The client could time out otherwise.
            biased;

            _ = interval.tick() => Some(Update::Ping),
            recv = recv => recv,
        };

        let update = match update {
            Some(update) => update,
            None => break,
        };

        let start = Instant::now();
        rkvm_net::timeout(rkvm_net::WRITE_TIMEOUT, async {
            update.encode(&mut stream).await?;
            stream.flush().await?;

            Ok(())
        })
        .await?;
        let duration = start.elapsed();

        if let Update::Ping = update {
            // Keeping these as debug because it's not as frequent as other updates.
            tracing::debug!(duration = ?duration, "Sent ping");

            let start = Instant::now();
            rkvm_net::timeout(rkvm_net::READ_TIMEOUT, Pong::decode(&mut stream)).await?;
            let duration = start.elapsed();

            tracing::debug!(duration = ?duration, "Received pong");
        }

        tracing::trace!("Wrote an update");
    }

    Ok(())
}
