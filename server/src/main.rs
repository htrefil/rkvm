mod config;

use config::Config;
use input::{Direction, Event, EventManager};
use net::{self, Message, PROTOCOL_VERSION};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process;
use std::time::Duration;
use structopt::StructOpt;
use tokio::fs;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time;

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
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "Incompatible protocol version (got {}, expecting {})",
                version, PROTOCOL_VERSION
            ),
        ));
    }

    loop {
        let message = match time::timeout(Duration::from_secs(10), receiver.recv()).await {
            Ok(Some(message)) => Message::Event(message),
            Ok(None) => return Ok(()),
            Err(_) => Message::KeepAlive,
        };

        net::write_message(&mut stream, &message).await?;
    }
}

async fn run(listen_address: SocketAddr, switch_keys: &HashSet<u16>) -> Result<Infallible, Error> {
    let mut listener = TcpListener::bind(listen_address).await?;

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
                    .unwrap_or(String::new());
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
                if let Event::Key { direction, code } = event {
                    if let Some(state) = key_states.get_mut(&code) {
                        *state = if direction == Direction::Down {
                            true
                        } else {
                            false
                        };
                    }
                }

                if key_states.iter().filter(|(_, state)| **state).count() == key_states.len() {
                    for (_, state) in &mut key_states {
                        *state = false;
                    }

                    current = (current + 1) % (clients.len() + 1);
                    log::info!("Switching to client {}", current);
                }

                if current != 0 {
                    if clients[current - 1].send(event).is_ok() {
                        continue;
                    }

                    clients.remove(current);
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
    env_logger::builder().format_timestamp(None).init();

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

    if let Err(err) = run(config.listen_address, &config.switch_keys).await {
        log::error!("Error: {}", err);
        process::exit(1);
    }
}
