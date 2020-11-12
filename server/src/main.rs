mod config;

use anyhow::{Context, Error};
use config::Config;
use input::{Direction, Event, EventManager, Key, KeyKind};
use log::LevelFilter;
use net::{self, Message, PROTOCOL_VERSION};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process;
use structopt::StructOpt;
use tokio::fs;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time;
use tokio_native_tls::native_tls::{Identity, TlsAcceptor};

async fn handle_connection<T>(
    mut stream: T,
    mut receiver: UnboundedReceiver<Event>,
) -> Result<(), Error>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    net::write_version(&mut stream, PROTOCOL_VERSION).await?;

    let version = net::read_version(&mut stream).await?;
    if version != PROTOCOL_VERSION {
        return Err(anyhow::anyhow!(
            "Incompatible protocol version (got {}, expecting {})",
            version,
            PROTOCOL_VERSION
        ));
    }

    loop {
        // Send a keep alive message in intervals of half of the timeout just to be on the safe side.
        let message = match time::timeout(net::MESSAGE_TIMEOUT / 2, receiver.recv()).await {
            Ok(Some(message)) => Message::Event(message),
            Ok(None) => return Ok(()),
            Err(_) => Message::KeepAlive,
        };

        time::timeout(
            net::MESSAGE_TIMEOUT,
            net::write_message(&mut stream, &message),
        )
        .await
        .context("Write timeout")??;
    }
}

async fn run(
    listen_address: SocketAddr,
    switch_keys: &HashSet<Key>,
    identity_path: &Path,
    identity_password: &str,
) -> Result<Infallible, Error> {
    let identity = fs::read(identity_path)
        .await
        .context("Failed to read identity")?;
    let identity =
        Identity::from_pkcs12(&identity, identity_password).context("Failed to parse identity")?;
    let acceptor: tokio_native_tls::TlsAcceptor = TlsAcceptor::new(identity)
        .context("Failed to create TLS acceptor")
        .map(Into::into)?;
    let listener = TcpListener::bind(listen_address).await?;

    log::info!("Listening on {}", listen_address);

    let (client_sender, mut client_receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let (stream, address) = match listener.accept().await {
                Ok(sa) => sa,
                Err(err) => {
                    let _ = client_sender.send(Err(err));
                    return;
                }
            };

            let stream = match acceptor.accept(stream).await {
                Ok(stream) => stream,
                Err(err) => {
                    log::error!("{}: TLS error: {}", address, err);
                    continue;
                }
            };

            let (sender, receiver) = mpsc::unbounded_channel();
            if client_sender.send(Ok(sender)).is_err() {
                return;
            }

            tokio::spawn(async move {
                log::info!("{}: connected", address);
                let message = handle_connection(stream, receiver)
                    .await
                    .err()
                    .map(|err| format!(" ({})", err))
                    .unwrap_or_else(String::new);
                log::info!("{}: disconnected{}", address, message);
            });
        }
    });

    let mut clients: Vec<UnboundedSender<Event>> = Vec::new();
    let mut current = 0;
    let mut manager = EventManager::new().await?;
    let mut key_states: HashMap<_, _> = switch_keys
        .iter()
        .copied()
        .map(|key| (key, false))
        .collect();
    loop {
        tokio::select! {
            event = manager.read() => {
                let event = event?;
                if let Event::Key { direction, kind: KeyKind::Key(key) } = event {
                    if let Some(state) = key_states.get_mut(&key) {
                        *state = direction == Direction::Down;
                    }
                }

                // TODO: This won't work with multiple keys.
                if key_states.iter().filter(|(_, state)| **state).count() == key_states.len() {
                    for state in key_states.values_mut() {
                        *state = false;
                    }

                    current = (current + 1) % (clients.len() + 1);
                    log::info!("Switching to client {}", current);
                    continue;
                }

                if current != 0 {
                    let idx = current - 1;
                    if clients[idx].send(event).is_ok() {
                        continue;
                    }

                    clients.remove(idx);
                    current = 0;
                }

                manager.write(event).await?;
            }
            sender = client_receiver.recv() => {
                clients.push(sender.unwrap()?);
            }
        }
    }
}

#[derive(StructOpt)]
#[structopt(name = "rkvm-server", about = "The rkvm server application")]
struct Args {
    #[structopt(
        help = "Path to configuration file",
        default_value = "/etc/rkvm/server.toml"
    )]
    config_path: PathBuf,
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .format_timestamp(None)
        .filter(None, LevelFilter::Info)
        .init();

    let args = Args::from_args();
    let config = match fs::read_to_string(&args.config_path).await {
        Ok(config) => config,
        Err(err) => {
            log::error!("Error loading config: {}", err);
            process::exit(1);
        }
    };

    let config: Config = match toml::from_str(&config) {
        Ok(config) => config,
        Err(err) => {
            log::error!("Error parsing config: {}", err);
            process::exit(1);
        }
    };

    tokio::select! {
        result = run(config.listen_address, &config.switch_keys, &config.identity_path, &config.identity_password) => {
            if let Err(err) = result {
                log::error!("Error: {:#}", err);
                process::exit(1);
            }
        }
        result = tokio::signal::ctrl_c() => {
            if let Err(err) = result {
                log::error!("Error setting up signal handler: {}", err);
                process::exit(1);
            }

            log::info!("Exiting on signal");
        }
    }
}
