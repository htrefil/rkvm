mod config;
mod tls;

use clap::Parser;
use config::Config;
use log::LevelFilter;
use rkvm_input::{Direction, Event, EventManager, Key, KeyKind};
use slab::Slab;
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufStream};
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::mpsc::{self, Sender};
use tokio_rustls::TlsAcceptor;

#[derive(Parser)]
#[structopt(name = "rkvm-server", about = "The rkvm server application")]
struct Args {
    #[structopt(help = "Path to configuration file")]
    config_path: PathBuf,
}

#[tokio::main]
async fn main() -> ExitCode {
    env_logger::builder()
        .format_timestamp(None)
        .filter(None, LevelFilter::Info)
        .parse_default_env()
        .init();

    let args = Args::parse();
    let config = match fs::read_to_string(&args.config_path).await {
        Ok(config) => config,
        Err(err) => {
            log::error!("Error reading config: {}", err);
            return ExitCode::FAILURE;
        }
    };

    let config = match toml::from_str::<Config>(&config) {
        Ok(config) => config,
        Err(err) => {
            log::error!("Error parsing config: {}", err);
            return ExitCode::FAILURE;
        }
    };

    let acceptor = match tls::configure(&config.certificate, &config.key).await {
        Ok(acceptor) => acceptor,
        Err(err) => {
            log::error!("Error configuring TLS: {}", err);
            return ExitCode::FAILURE;
        }
    };

    tokio::select! {
        result = run(config.listen, acceptor, config.switch_key) => {
            if let Err(err) = result {
                log::error!("Error running server: {}", err);
                return ExitCode::FAILURE;
            }
        }
        // This is needed to properly clean libevent stuff up.
        result = signal::ctrl_c() => {
            if let Err(err) = result {
                log::error!("Error setting up signal handler: {}", err);
                return ExitCode::FAILURE;
            }

            log::info!("Exiting on signal");
        }
    }

    ExitCode::SUCCESS
}

async fn run(listen: SocketAddr, acceptor: TlsAcceptor, switch_key: Key) -> Result<(), Error> {
    let listener = TcpListener::bind(&listen).await?;
    log::info!("Listening on {}", listen);

    let mut clients = Slab::<Sender<_>>::new();
    let mut current = 0;
    let mut manager = EventManager::new().await?;

    loop {
        tokio::select! {
            result = listener.accept() => {
                // Remove dead clients.
                clients.retain(|_, client| !client.is_closed());
                if !clients.contains(current) {
                    current = 0;
                }

                let (stream, addr) = result?;
                let acceptor = acceptor.clone();

                let (sender, mut receiver) = mpsc::channel::<Event>(1);
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

                    let result = async {
                        let mut stream = BufStream::with_capacity(1024, 1024, stream);

                        rkvm_net::write_version(&mut stream, rkvm_net::PROTOCOL_VERSION).await?;
                        stream.flush().await?;

                        let version = rkvm_net::read_version(&mut stream).await?;
                        if version != rkvm_net::PROTOCOL_VERSION {
                            return Err(Error::new(ErrorKind::InvalidData, "Invalid client protocol version"));
                        }

                        loop {
                            let event = match receiver.recv().await {
                                Some(event) => event,
                                None => break,
                            };

                            rkvm_net::write_message(&mut stream, &event).await?;
                            stream.flush().await?;
                        }

                        Ok::<_, Error>(())
                    }
                    .await;

                    match result {
                        Ok(()) => log::info!("{}: Disconnected", addr),
                        Err(err) => log::error!("{}: Disconnected: {}", addr, err),
                    }
                });
            }
            result = manager.read() => {
                let event = result?;
                if let Event::Key { direction: Direction::Down, kind: KeyKind::Key(key) } = event {
                    if key == switch_key {
                        current = (current + 1) % (clients.len() + 1);
                        log::info!("Switching to client {}", current);
                    }
                }

                if current != 0 && clients[current].send(event).await.is_err() {
                    current = 0;
                    manager.write(event).await?;
                }
            }
        }
    }
}
