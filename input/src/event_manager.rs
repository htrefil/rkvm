use crate::event::Event;
use crate::event_reader::EventReader;
use crate::event_writer::EventWriter;
use crate::glue::input_event;
use std::io::{Error, ErrorKind};
use tokio::fs;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

pub struct EventManager {
    writer: EventWriter,
    receiver: UnboundedReceiver<Result<input_event, Error>>,
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
        Ok(EventManager { writer, receiver })
    }

    pub async fn read(&mut self) -> Result<Event, Error> {
        loop {
            let event = self
                .receiver
                .recv()
                .await
                .ok_or_else(|| Error::new(ErrorKind::Other, "All devices closed"))??;
            if let Some(event) = Event::from_raw(event) {
                return Ok(event);
            }

            // Not understood. Write it back.
            self.writer.write_raw(event).await?;
        }
    }

    pub async fn write(&mut self, event: Event) -> Result<(), Error> {
        self.writer.write(event).await
    }
}

async fn handle_events(
    mut reader: EventReader,
    sender: UnboundedSender<Result<input_event, Error>>,
) {
    loop {
        let result = match reader.read().await {
            Ok(event) => sender.send(Ok(event)).is_ok(),
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
