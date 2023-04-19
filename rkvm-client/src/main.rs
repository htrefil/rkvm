mod client;
mod config;
mod tls;

use clap::Parser;
use config::Config;
use log::LevelFilter;
use std::path::PathBuf;
use std::process::ExitCode;
use tokio::fs;
use tokio::signal;

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
        result = client::run(&config.server.hostname, config.server.port, connector, &config.password) => {
            if let Err(err) = result {
                log::error!("Error: {}", err);
                return ExitCode::FAILURE;
            }
        }
        // This is needed to properly clean libevdev stuff up.
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
