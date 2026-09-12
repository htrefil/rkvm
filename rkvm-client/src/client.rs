use rkvm_input::writer::{DeviceWriter};
use rkvm_net::message::Message;
use rkvm_net::version::Version;
use rkvm_net::Update;

use async_trait::async_trait;
use std::fs::{rename, OpenOptions};
use std::io::{self, stdout, BufWriter};
use std::path::Path;
use std::time::Instant;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::time;
use tokio_rustls::rustls;
use tracing_subscriber::{fmt, Registry,EnvFilter};
use tracing_subscriber::fmt::time::LocalTime;
use tracing_subscriber::prelude::*;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(io::Error),
    #[error("Io error: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Rustls(#[from] rustls::Error),
    #[error("Toml error: {0}")]
    Toml(#[from] toml::de::Error),
    #[cfg(target_os="windows")]
    #[error("Windows API error: {0}")]
    Windows(#[from] windows::core::Error),
    #[cfg(all(target_os="windows",feature="windows-service"))]
    #[error("Windows Service error: {0}")]
    WindowsService(#[from] windows_service::Error),
    #[allow(dead_code)]
    #[error("Incompatible server version (got {server}, expected {client})")]
    Version { server: Version, client: Version },
    #[allow(dead_code)]
    #[error("Invalid password")]
    Auth,
}

pub fn init_tracing<P: AsRef<Path>>(log_level: &String, log_file: &Option<P>) {
    let filter = EnvFilter::new(log_level);
    if let Some(path) = log_file {
        let path = path.as_ref();
        // rotate log
        for i in (1..10).rev() {
            let old = format!("{}.{}", path.display(), i);
            let old = Path::new(&old);
            if old.exists() {
                let new = format!("{}.{}", path.display(), i + 1);
                let _ = rename(&old, &new);
            }
        }

        if path.exists() {
            let new = format!("{}.1", path.display());
            let _ = rename(path, &new);
        }
        let file = OpenOptions::new().create(true).append(true).open(path).unwrap();
        let fmt_layer = fmt::layer().with_ansi(false).with_timer(LocalTime::rfc_3339()).with_writer(move || BufWriter::new(file.try_clone().unwrap()));
        let registry = Registry::default().with(filter).with(fmt_layer);
        tracing::subscriber::set_global_default(registry).unwrap();
    } else {
        let fmt_layer = fmt::layer().with_writer(stdout).without_time();
        let registry = Registry::default().with(filter).with(fmt_layer);
        tracing::subscriber::set_global_default(registry).unwrap();
    }
}

pub async fn run<R,W,H>(reader: &mut R, writer: &mut W, mut handler: H) -> Result<(), Error> 
    where
        R: AsyncRead + Send + Unpin,
        W: RkvmWriter + Send,
        H: DeviceWriter {

    let mut start = Instant::now();

    let timeout_duration = rkvm_net::PING_INTERVAL + rkvm_net::READ_TIMEOUT;

    loop {
        let update = match time::timeout(timeout_duration, Update::decode(reader)).await {
            Err(_) => Err(Error::Network(io::Error::new(io::ErrorKind::TimedOut, "Ping timeout"))),
            Ok(res) => res.map_err(Error::Network)
        }?;

        let duration = start.elapsed();
        tracing::debug!(duration = ?duration, "received {:?}", update);
        start = Instant::now();

        match update {
            Update::CreateDevice { id,name,vendor,product,version,rel,abs,keys,delay,period,} => {
                handler.create_device(id, &name, vendor, product, version, rel, abs, keys, delay, period).await?;
                tracing::info!(id = %id, name = ?name, vendor = %vendor, product = %product, version = %version, "Created new device");
            }
            Update::DestroyDevice { id } => {
                handler.destroy_device(id).await?;
                tracing::info!(id = %id, "Destroyed device");
            }
            Update::Event { id, event } => {
                handler.event(id, event).await?;
                tracing::trace!(id = %id, "Wrote an event to device");
            }
            Update::Ping => {
                writer.send(Update::Pong).await?;
                let duration = start.elapsed();
                tracing::debug!(duration = ?duration, "Sent pong");
            }
            Update::Pong => {}
            Update::Stop => {
                tracing::info!("Stoping..");
                return Ok(());
            }
        }
    }
}

#[async_trait]
pub trait RkvmWriter {
    async fn send(&mut self, update: Update) -> Result<(), Error>;
}

#[async_trait]
impl<W> RkvmWriter for W
where
    W: AsyncWrite + Unpin + Send,
{
    async fn send(&mut self, update: Update) -> Result<(), Error> {
        update.encode(self).await?;
        self.flush().await?;
        Ok(())
    }
}
