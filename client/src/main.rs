mod config;

use config::Config;
use input::EventWriter;
use net::{self, Message, PROTOCOL_VERSION};
use std::convert::Infallible;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;
use tokio::fs;
use tokio::net::TcpStream;

async fn run(server: &str, port: u16) -> Result<Infallible, Error> {
    let mut stream = TcpStream::connect((server, port)).await?;

    log::info!("Connected to {}:{}", server, port);

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

    let mut writer = EventWriter::new().await?;
    loop {
        let message = net::read_message(&mut stream).await?;
        match message {
            Message::Event(event) => writer.write(event).await?,
            Message::KeepAlive => {}
        }
    }
}

#[derive(StructOpt)]
#[structopt(name = "rkvm-client", about = "The rkvm client application")]
struct Args {
    #[structopt(
        help = "Path to configuration file",
        default_value = "/etc/rkvm/client.toml"
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

    if let Err(err) = run(&config.server.hostname, config.server.port).await {
        log::error!("Error: {}", err);
        process::exit(1);
    }
}
