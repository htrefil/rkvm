use rkvm_input::{Direction, Event, EventManager, Key, KeyKind};
use rkvm_net::auth::{AuthChallenge, AuthResponse, AuthStatus};
use rkvm_net::message::Message;
use rkvm_net::version::Version;
use slab::Slab;
use std::io;
use std::net::SocketAddr;
use thiserror::Error;
use tokio::io::{AsyncWriteExt, BufStream};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_rustls::server::TlsStream;
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
    switch_key: Key,
) -> Result<(), Error> {
    let listener = TcpListener::bind(&listen).await.map_err(Error::Network)?;
    log::info!("Listening on {}", listen);

    let mut clients = Slab::<Sender<_>>::new();
    let mut current = 0;
    let mut manager = EventManager::new().await.map_err(Error::Input)?;

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
                    let stream = match acceptor.accept(stream).await {
                        Ok(stream) => stream,
                        Err(err) => {
                            log::error!("{}: TLS accept error: {}", addr, err);
                            return;
                        }
                    };

                    log::info!("{}: Connected", addr);

                    match client(receiver, stream, addr, &password).await {
                        Ok(()) => log::info!("{}: Disconnected", addr),
                        Err(err) => log::error!("{}: Disconnected: {}", addr, err),
                    }
                });
            }
            result = manager.read() => {
                let event = result.map_err(Error::Input)?;
                if let Event::Key { direction: Direction::Down, kind: KeyKind::Key(key) } = event {
                    if key == switch_key {
                        current = (current + 1) % (clients.len() + 1);
                        log::info!("Switching to client {}", current);
                        continue;
                    }
                }

                if current == 0 || clients[current - 1].send(event).await.is_err() {
                    current = 0;
                    manager.write(event).await.map_err(Error::Input)?;
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
    #[error("Auth challenge failed (possibly wrong password)")]
    Auth,
    #[error(transparent)]
    Rand(#[from] rand::Error),
}

async fn client(
    mut receiver: Receiver<Event>,
    stream: TlsStream<TcpStream>,
    addr: SocketAddr,
    password: &str,
) -> Result<(), ClientError> {
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

    log::info!("{}: Passed auth check", addr);

    while let Some(event) = receiver.recv().await {
        event.encode(&mut stream).await?;
        stream.flush().await?;

        log::trace!("{}: Sent event", addr);
    }

    Ok(())
}
