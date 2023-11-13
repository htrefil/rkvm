use crate::interceptor::{Interceptor, OpenError};
use crate::registry::Registry;

use futures::StreamExt;
use inotify::{Inotify, WatchMask};
use std::ffi::OsStr;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use tokio::fs;
use tokio::sync::mpsc::{self, Receiver, Sender};

const EVENT_PATH: &str = "/dev/input";

pub struct Monitor {
    receiver: Receiver<Result<Interceptor, Error>>,
}

impl Monitor {
    pub fn new(input_device_paths: &HashSet<String>) -> Self {
        let (sender, receiver) = mpsc::channel(1);
        tokio::spawn(monitor(sender, input_device_paths.clone()));

        Self { receiver }
    }

    pub async fn read(&mut self) -> Result<Interceptor, Error> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| Error::new(ErrorKind::BrokenPipe, "Monitor task exited"))?
    }
}

async fn monitor(sender: Sender<Result<Interceptor, Error>>, input_device_paths: HashSet<String>) {
    let run = async {
        let registry = Registry::new();

        let mut read_dir = fs::read_dir(EVENT_PATH).await?;

        let mut inotify = Inotify::init()?;
        inotify.add_watch(EVENT_PATH, WatchMask::CREATE)?;

        // This buffer size should be OK, since we don't expect a lot of devices
        // to be plugged in frequently.
        let mut stream = inotify.event_stream([0; 512])?;

        loop {
            let path = match read_dir.next_entry().await? {
                Some(entry) => entry.path(),
                None => match stream.next().await {
                    Some(event) => {
                        let event = event?;
                        let name = match event.name {
                            Some(name) => name,
                            None => continue,
                        };

                        Path::new(EVENT_PATH).join(&name)
                    }
                    None => break,
                },
            };

            if !path
                .file_name()
                .and_then(OsStr::to_str)
                .map_or(false, |name| name.starts_with("event"))
            {
                tracing::debug!("Skipping non event file {:?}", path);
                continue;
            }

            if register_input_device(&input_device_paths, path.clone()) {
                let interceptor = match Interceptor::open(&path, &registry).await {
                    Ok(interceptor) => interceptor,
                    Err(OpenError::Io(err)) => return Err(err),
                    Err(OpenError::NotAppliable) => continue,
                };

                if sender.send(Ok(interceptor)).await.is_err() {
                    return Ok(());
                }
            }
        }

        Ok(())
    };

    tokio::select! {
        result = run => match result {
            Ok(_) => {},
            Err(err) => {
                let _ = sender.send(Err(err)).await;
            }
        },
        _ = sender.closed() => {}
    }
}

fn register_input_device(input_device_paths: &HashSet<String>, input_device_path: PathBuf) -> bool {
    if input_device_paths.len() > 0 {
        match input_device_path.into_os_string().into_string() {
            Ok(path) => return input_device_paths.contains(&path),
            Err(err) => {
                tracing::error!("Can't convert a path into string! {:?}", err);
                return false;
            },
        }
    } else {
        return true;
    }
}
