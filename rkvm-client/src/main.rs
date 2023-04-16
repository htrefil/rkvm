mod config;

use anyhow::{Context, Error};
use config::Config;
use rkvm_input::EventWriter;
use log::LevelFilter;
use rkvm_net::{self, Message, PROTOCOL_VERSION};
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::process;
use clap::Parser;
use tokio::fs;
use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::time;
use tokio_native_tls::native_tls::{Certificate, TlsConnector};

async fn run(server: &str, port: u16, certificate_path: &Path) -> Result<Infallible, Error> {
    let certificate = fs::read(certificate_path)
        .await
        .context("Failed to read certificate")?;
    let certificate = Certificate::from_der(&certificate)
        .or_else(|_| Certificate::from_pem(&certificate))
        .context("Failed to parse certificate")?;

    let connector: tokio_native_tls::TlsConnector = TlsConnector::builder()
        .add_root_certificate(certificate)
        .build()
        .context("Failed to create connector")?
        .into();

    let stream = TcpStream::connect((server, port)).await?;
    let stream = BufReader::new(stream);
    let mut stream = connector
        .connect(server, stream)
        .await
        .context("Failed to connect")?;

    log::info!("Connected to {}:{}", server, port);

    rkvm_net::write_version(&mut stream, PROTOCOL_VERSION).await?;

    let version = rkvm_net::read_version(&mut stream).await?;
    if version != PROTOCOL_VERSION {
        return Err(anyhow::anyhow!(
            "Incompatible protocol version (got {}, expecting {})",
            version,
            PROTOCOL_VERSION
        ));
    }

    let mut writer = EventWriter::new().await?;
    loop {
        let message = time::timeout(rkvm_net::MESSAGE_TIMEOUT, rkvm_net::read_message(&mut stream))
            .await
            .context("Read timed out")??;
        match message {
            Message::Event(event) => writer.write(event).await?,
            Message::KeepAlive => {}
        }
    }
}

#[derive(Parser)]
#[structopt(name = "rkvm-client", about = "The rkvm client application")]
struct Args {
    #[clap(help = "Path to configuration file")]
    config_path: PathBuf,
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .format_timestamp(None)
        .filter(None, LevelFilter::Info)
        .init();

    let args = Args::parse();
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
        result = run(&config.server.hostname, config.server.port, &config.certificate_path) => {
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