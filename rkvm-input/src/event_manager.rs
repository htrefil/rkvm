use crate::event::{Event, EventBatch};
use crate::event_reader::{EventReader, OpenError};
use crate::event_writer::EventWriter;

use futures::StreamExt;
use inotify::{Inotify, WatchMask};
use std::io::Error;
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::time;

const EVENT_PATH: &str = "/dev/input";

pub struct EventManager {
    event_writer: EventWriter,
    event_receiver: Receiver<Result<EventBatch, Error>>,
}

impl EventManager {
    pub async fn new() -> Result<Self, Error> {
        // HACK: When rkvm is run from the terminal, a race condition happens where the enter key
        // release event is swallowed and the key will remain in a "pressed" state until the user manually presses it again.
        // This is presumably due to the event being generated while we're in the process of grabbing
        // the keyboard input device.
        //
        // This won't prevent this from happenning with other keys if they happen to be pressed at an
        // unfortunate time, but that is unlikely to happen and will ease the life of people who run rkvm
        // directly from the terminal for the time being until a proper fix is made.
        time::sleep(Duration::from_millis(500)).await;

        let (event_sender, event_receiver) = mpsc::channel(1);

        let mut read_dir = fs::read_dir(EVENT_PATH).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            spawn_reader(&entry.path(), event_sender.clone()).await?;
        }

        let event_writer = EventWriter::new().await?;

        // Sleep for a while to give userspace time to register our devices.
        time::sleep(Duration::from_secs(1)).await;

        tokio::spawn(async move {
            let run = async {
                let mut inotify = Inotify::init()?;
                inotify.add_watch(EVENT_PATH, WatchMask::CREATE)?;

                // This buffer size should be OK, since we don't expect a lot of devices
                // to be plugged in frequently.
                let mut stream = inotify.event_stream([0u8; 512])?;
                while let Some(event) = stream.next().await {
                    let event = event?;

                    if let Some(name) = event.name {
                        let path = Path::new(EVENT_PATH).join(&name);
                        spawn_reader(&path, event_sender.clone()).await?;
                    }
                }

                Ok(())
            };

            tokio::select! {
                result = run => {
                    if let Err(err) = result {
                        let _ = event_sender.send(Err(err)).await;
                    }
                }
                _ = event_sender.closed() => {}
            }
        });

        Ok(Self {
            event_writer,
            event_receiver,
        })
    }

    pub async fn read(&mut self) -> Result<EventBatch, Error> {
        self.event_receiver.recv().await.unwrap()
    }

    pub async fn write(&mut self, events: &[Event]) -> Result<(), Error> {
        self.event_writer.write(events).await
    }
}

async fn spawn_reader(path: &Path, sender: Sender<Result<EventBatch, Error>>) -> Result<(), Error> {
    if path.is_dir() {
        return Ok(());
    }

    // Skip non input event files.
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| !name.starts_with("event"))
        .unwrap_or(true)
    {
        return Ok(());
    }

    let mut reader = match EventReader::open(&path).await {
        Ok(reader) => reader,
        Err(OpenError::Io(err)) => return Err(err),
        Err(OpenError::NotAppliable) => return Ok(()),
    };

    tokio::spawn(async move {
        let run = async {
            loop {
                let result = match reader.read().await {
                    Ok(events) => sender.send(Ok(events)).await.is_ok(),
                    // This happens if the device is disconnected.
                    // In that case simply terminate the reading task.
                    Err(ref err) if err.raw_os_error() == Some(libc::ENODEV) => false,
                    Err(err) => {
                        let _ = sender.send(Err(err));
                        false
                    }
                };

                if !result {
                    break;
                }
            }
        };

        tokio::select! {
            _ = run => {}
            _ = sender.closed() => {}
        }
    });

    Ok(())
}
