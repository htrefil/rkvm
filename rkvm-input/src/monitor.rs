use crate::interceptor::{Interceptor, OpenError};

use futures::StreamExt;
use inotify::{Inotify, WatchMask};
use std::io::{Error, ErrorKind};
use std::path::Path;
use tokio::fs;
use tokio::sync::mpsc::{self, Receiver};

const EVENT_PATH: &str = "/dev/input";

pub struct Monitor {
    receiver: Receiver<Result<Interceptor, Error>>,
}

impl Monitor {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(1);
        tokio::spawn(async move {
            let run = async {
                let mut read_dir = fs::read_dir(EVENT_PATH).await?;
                while let Some(entry) = read_dir.next_entry().await? {
                    let interceptor = match open(&entry.path()).await? {
                        Some(interceptor) => interceptor,
                        None => continue,
                    };

                    if sender.send(Ok(interceptor)).await.is_err() {
                        return Ok(());
                    }
                }

                let mut inotify = Inotify::init()?;
                inotify.add_watch(EVENT_PATH, WatchMask::CREATE)?;

                // This buffer size should be OK, since we don't expect a lot of devices
                // to be plugged in frequently.
                let mut stream = inotify.event_stream([0; 512])?;
                while let Some(event) = stream.next().await {
                    let event = event?;

                    if let Some(name) = event.name {
                        let interceptor = match open(&Path::new(EVENT_PATH).join(&name)).await? {
                            Some(interceptor) => interceptor,
                            None => continue,
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
        });

        Self { receiver }
    }

    pub async fn read(&mut self) -> Result<Interceptor, Error> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| Error::new(ErrorKind::BrokenPipe, "Monitor task exited"))?
    }
}

async fn open(path: &Path) -> Result<Option<Interceptor>, Error> {
    // Skip non input event files.
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| !name.starts_with("event"))
        .unwrap_or(true)
    {
        return Ok(None);
    }

    let interceptor = match Interceptor::open(&path).await {
        Ok(interceptor) => interceptor,
        Err(OpenError::Io(err)) => return Err(err),
        Err(OpenError::NotAppliable) => return Ok(None),
    };

    Ok(Some(interceptor))
}
