mod client;
mod init;
#[cfg(any(target_os="linux",not(feature="windows-service")))]
mod connection;
#[cfg(any(target_os="linux",not(feature="windows-service")))]
mod config;

use rkvm_net::Update;

use client::{init_tracing,RkvmWriter};

use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;
use tokio::io::split;
use tokio::signal;

#[derive(Parser)]
#[structopt(name = "rkvm-client", about = "The rkvm client application")]
struct Args {
    #[clap(help = "Path to configuration file")]
    config_path: PathBuf,
    #[clap(long, default_value = "info", help = "log filter")]
    log_level: String,
    #[clap(long, help = "output file for the logs")]
    log_file: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    init_tracing(&args.log_level, &args.log_file);

    tracing::info!("Client starting...");
    let stream = match init::stream(&args.config_path).await {
        Ok(stream) => stream,
        Err(e) => {
            tracing::error!("Failed to open stream {}", e);
            return ExitCode::FAILURE;
        }
    };


    let writers = init::writers();

    let (mut r, mut w) = split(stream);
    tokio::select! {
        result = client::run(&mut r, &mut w, writers) => {
            if let Err(err) = result {
                tracing::error!("Error: {}", err);
                return ExitCode::FAILURE;
            }
        }
        // This is needed to properly clean libevdev stuff up.
        result = signal::ctrl_c() => {
            let _ = w.send(Update::Stop);
            if let Err(err) = result {
                tracing::error!("Error setting up signal handler: {}", err);
                return ExitCode::FAILURE;
            }

            tracing::info!("Exiting on signal");
        }
    }

    ExitCode::SUCCESS
}

