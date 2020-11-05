use crate::event::Event;
use crate::event_reader::EventReader;
use crate::event_writer::EventWriter;
use std::io::{Error, ErrorKind};
use std::time::Duration;
use tokio::fs;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

pub struct EventManager {
    writer: EventWriter,
    receiver: UnboundedReceiver<Result<Event, Error>>,
}

impl EventManager {
    pub async fn new() -> Result<Self, Error> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut read_dir = fs::read_dir("/dev/input").await?;
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                continue;
            }

            // Skip non input event files.
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| !name.starts_with("event"))
                .unwrap_or(true)
            {
                continue;
            }

            let reader = EventReader::new(&path).await?;
            let sender = sender.clone();

            tokio::spawn(handle_events(reader, sender));
        }

        let writer = EventWriter::new().await?;

        // Sleep for a while to give userspace time to register our devices.
        tokio::time::sleep(Duration::from_secs(1)).await;

        Ok(EventManager { writer, receiver })
    }

    pub async fn read(&mut self) -> Result<Event, Error> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| Error::new(ErrorKind::Other, "All devices closed"))?
    }

    pub async fn write(&mut self, event: Event) -> Result<(), Error> {
        self.writer.write(event).await
    }
}

async fn handle_events(mut reader: EventReader, sender: UnboundedSender<Result<Event, Error>>) {
    loop {
        let result = match reader.read().await {
            Ok(event) => sender.send(Ok(event)).is_ok(),
            // This happens if the device is disconnected.
            // In that case simply terminate the reading task.
            Err(ref err) if err.raw_os_error() == Some(libc::ENOTTY) => false,
            Err(err) => {
                let _ = sender.send(Err(err));
                false
            }
        };

        if !result {
            break;
        }
    }
}
