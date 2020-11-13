mod config;

use anyhow::{Context, Error};
use config::Config;
use input::EventWriter;
use log::LevelFilter;
use net::{self, Message, PROTOCOL_VERSION};
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::process;
use structopt::StructOpt;
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

    net::write_version(&mut stream, PROTOCOL_VERSION).await?;

    let version = net::read_version(&mut stream).await?;
    if version != PROTOCOL_VERSION {
        return Err(anyhow::anyhow!(
            "Incompatible protocol version (got {}, expecting {})",
            version,
            PROTOCOL_VERSION
        ));
    }

    let mut writer = EventWriter::new().await?;
    loop {
        let message = time::timeout(net::MESSAGE_TIMEOUT, net::read_message(&mut stream))
            .await
            .context("Read timed out")??;
        match message {
            Message::Event(event) => writer.write(event).await?,
            Message::KeepAlive => {}
        }
    }
}

#[derive(StructOpt)]
#[structopt(name = "rkvm-client", about = "The rkvm client application")]
struct Args {
    #[structopt(help = "Path to configuration file")]
    #[cfg_attr(
        target_os = "linux",
        structopt(default_value = "/etc/rkvm/client.toml")
    )]
    #[cfg_attr(
        target_os = "windows",
        structopt(default_value = "C:/rkvm/client.toml")
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
