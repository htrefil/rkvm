use rkvm_input::{Direction, Event, EventBatch, EventManager, Key, KeyKind};
use rkvm_net::auth::{AuthChallenge, AuthResponse, AuthStatus};
use rkvm_net::message::Message;
use rkvm_net::version::Version;
use slab::Slab;
use std::collections::HashSet;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncWriteExt, BufStream};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::time;
use tokio_rustls::TlsAcceptor;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(io::Error),
    #[error("Input error: {0}")]
    Input(io::Error),
}

pub async fn run(
    listen: SocketAddr,
    acceptor: TlsAcceptor,
    password: &str,
    switch_keys: &HashSet<Key>,
) -> Result<(), Error> {
    let listener = TcpListener::bind(&listen).await.map_err(Error::Network)?;
    log::info!("Listening on {}", listen);

    let mut clients = Slab::<Sender<_>>::new();
    let mut current = 0;
    let mut manager = EventManager::new().await.map_err(Error::Input)?;

    let mut pressed_keys = HashSet::new();

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, addr) = result.map_err(Error::Network)?;
                let acceptor = acceptor.clone();
                let password = password.to_owned();

                // Remove dead clients.
                clients.retain(|_, client| !client.is_closed());
                if !clients.contains(current) {
                    current = 0;
                }

                let (sender, receiver) = mpsc::channel(1);
                clients.insert(sender);

                tokio::spawn(async move {
                    log::info!("{}: Connected", addr);

                    match client(receiver, stream, addr, acceptor, &password).await {
                        Ok(()) => log::info!("{}: Disconnected", addr),
                        Err(err) => log::error!("{}: Disconnected: {}", addr, err),
                    }
                });
            }
            result = manager.read() => {
                let mut changed = false;

                let events = result.map_err(Error::Input)?;
                for event in &events {
                    let (direction, key) = match event {
                        Event::Key { direction, kind: KeyKind::Key(key) } => (direction, key),
                        _ => continue,
                    };

                    if !switch_keys.contains(key) {
                        continue;
                    }

                    changed = true;

                    match direction {
                        Direction::Up => pressed_keys.remove(key),
                        Direction::Down => pressed_keys.insert(*key),
                    };
                }

                // Who to send this batch of events to.
                let idx = current;

                if changed && pressed_keys.len() == switch_keys.len() {
                    let exists = |idx| idx == 0 || clients.contains(idx - 1);
                    loop {
                        current = (current + 1) % (clients.len() + 1);
                        if exists(current) {
                            break;
                        }
                    }
                }

                if idx == 0 {
                    manager.write(&events).await.map_err(Error::Input)?;

                    log::trace!(
                        "Wrote {} event{}",
                        events.len(),
                        if events.len() == 1 { "" } else { "s" }
                    );

                    continue;
                }

                if clients[idx - 1].send(events).await.is_err() {
                    clients.remove(idx - 1);

                    if current == idx {
                        current = 0;
                    }
                }
            }
        }
    }
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
    mut receiver: Receiver<EventBatch>,
    stream: TcpStream,
    addr: SocketAddr,
    acceptor: TlsAcceptor,
    password: &str,
) -> Result<(), ClientError> {
    let negotiate = async {
        let stream = acceptor.accept(stream).await?;
        log::info!("{}: TLS connected", addr);

        let mut stream = BufStream::with_capacity(1024, 1024, stream);

        Version::CURRENT.encode(&mut stream).await?;
        stream.flush().await?;

        let version = Version::decode(&mut stream).await?;
        if version != Version::CURRENT {
            return Err(ClientError::Version {
                server: Version::CURRENT,
                client: version,
            });
        }

        let challenge = AuthChallenge::generate().await?;

        challenge.encode(&mut stream).await?;
        stream.flush().await?;

        let response = AuthResponse::decode(&mut stream).await?;
        let status = match response.verify(&challenge, password) {
            true => AuthStatus::Passed,
            false => AuthStatus::Failed,
        };

        status.encode(&mut stream).await?;
        stream.flush().await?;

        if status == AuthStatus::Failed {
            return Err(ClientError::Auth);
        }

        log::info!("{}: Authenticated successfully", addr);

        Ok(stream)
    };

    let mut stream = time::timeout(Duration::from_secs(1), negotiate)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Negotiation took too long"))??;

    while let Some(events) = receiver.recv().await {
        let write = async {
            events.encode(&mut stream).await?;
            stream.flush().await
        };

        time::timeout(Duration::from_millis(500), write)
            .await
            .map_err(|_| {
                io::Error::new(io::ErrorKind::TimedOut, "Event writing took too long")
            })??;

        log::trace!(
            "{}: Sent {} event{}",
            addr,
            events.len(),
            if events.len() == 1 { "" } else { "s" }
        );
    }

    Ok(())
}
