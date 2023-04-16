mod config;
mod tls;

use clap::Parser;
use config::Config;
use log::LevelFilter;
use rkvm_input::EventWriter;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;
use std::process::ExitCode;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::signal;
use tokio_rustls::rustls::ServerName;
use tokio_rustls::TlsConnector;

#[derive(Parser)]
#[structopt(name = "rkvm-client", about = "The rkvm client application")]
struct Args {
    #[clap(help = "Path to configuration file")]
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

    let connector = match tls::configure(&config.certificate).await {
        Ok(connector) => connector,
        Err(err) => {
            log::error!("Error configuring TLS: {}", err);
            return ExitCode::FAILURE;
        }
    };

    tokio::select! {
        result = run(&config.server.hostname, config.server.port, connector) => {
            if let Err(err) = result {
                log::error!("Error running client: {}", err);
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

async fn run(hostname: &ServerName, port: u16, connector: TlsConnector) -> Result<(), Error> {
    let stream = match hostname {
        ServerName::DnsName(name) => TcpStream::connect(&(name.as_ref(), port)).await?,
        ServerName::IpAddress(address) => TcpStream::connect(&(*address, port)).await?,
        _ => unimplemented!("Unhandled rustls ServerName variant"),
    };

    let mut stream = connector.connect(hostname.clone(), stream).await?;
    log::info!("Connected to server");

    rkvm_net::write_version(&mut stream, rkvm_net::PROTOCOL_VERSION).await?;
    stream.flush().await?;

    let version = rkvm_net::read_version(&mut stream).await?;
    if version != rkvm_net::PROTOCOL_VERSION {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Invalid server protocol version",
        ));
    }

    let mut writer = EventWriter::new().await?;
    loop {
        let event = rkvm_net::read_message(&mut stream).await?;
        writer.write(event).await?;
    }
}
