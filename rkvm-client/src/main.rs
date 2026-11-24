mod client;
mod config;
mod output;
mod tls;
mod uinput;
mod usb;

use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;
use tokio::signal::unix::{self, SignalKind};
use tokio::{fs, signal};
use tokio_rustls::TlsConnector;
use tracing::subscriber;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

use crate::config::Config;
use crate::output::Output;
use crate::uinput::Uinput;
use crate::usb::Usb;

#[derive(Parser)]
#[structopt(name = "rkvm-client", about = "The rkvm client application")]
struct Args {
    #[clap(help = "Path to configuration file")]
    config_path: PathBuf,
}

#[tokio::main]
async fn main() -> ExitCode {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().without_time());

    subscriber::set_global_default(registry).unwrap();

    let args = Args::parse();
    let config = match fs::read_to_string(&args.config_path).await {
        Ok(config) => config,
        Err(err) => {
            tracing::error!("Error reading config: {}", err);
            return ExitCode::FAILURE;
        }
    };

    let config = match toml::from_str::<Config>(&config) {
        Ok(config) => config,
        Err(err) => {
            tracing::error!("Error parsing config: {}", err);
            return ExitCode::FAILURE;
        }
    };

    let connector = match &config.certificate {
        Some(certificate) => match tls::configure(certificate).await {
            Ok(connector) => Some(connector),
            Err(err) => {
                tracing::error!("Error configuring TLS: {}", err);
                return ExitCode::FAILURE;
            }
        },
        None => {
            tracing::warn!("TLS is disabled, all traffic including keystrokes is sent unencrypted");
            None
        }
    };

    // Set up before connecting, so that misconfiguration is reported right away.
    match &config.output {
        config::Output::Uinput => run(&config, connector, Uinput::new()).await,
        config::Output::Usb(usb) => {
            let usb = match Usb::new(usb).await {
                Ok(usb) => usb,
                Err(err) => {
                    tracing::error!("Error setting up USB gadget: {}", err);
                    return ExitCode::FAILURE;
                }
            };

            run(&config, connector, usb).await
        }
    }
}

async fn run<T: Output>(
    config: &Config,
    connector: Option<TlsConnector>,
    mut output: T,
) -> ExitCode {
    let mut terminate = match unix::signal(SignalKind::terminate()) {
        Ok(terminate) => terminate,
        Err(err) => {
            tracing::error!("Error setting up signal handler: {}", err);
            return ExitCode::FAILURE;
        }
    };

    tokio::select! {
        result = client::run(&config.server.hostname, config.server.port, connector, &config.password, &mut output) => {
            let Err(err) = result;
            tracing::error!("Error: {}", err);

            return ExitCode::FAILURE;
        }
        _ = terminate.recv() => {
            tracing::info!("Exiting on signal");
        }
        result = signal::ctrl_c() => {
            if let Err(err) = result {
                tracing::error!("Error setting up signal handler: {}", err);
                return ExitCode::FAILURE;
            }

            tracing::info!("Exiting on signal");
        }
    }

    ExitCode::SUCCESS
}
