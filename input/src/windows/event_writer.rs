use crate::event::{Direction, Event, Key, KeyKind};
use std::io::Error;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot::{self, Receiver};
use tokio::time;
use winapi::um::winuser::{self, INPUT};

pub struct EventWriter {
    event_sender: UnboundedSender<Event>,
    error_receiver: Receiver<Error>,
}

impl EventWriter {
    pub async fn new() -> Result<Self, Error> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let (error_sender, error_receiver) = oneshot::channel();

        tokio::spawn(async move {
            if let Err(err) = handle_events(event_receiver).await {
                let _ = error_sender.send(err);
            }
        });

        Ok(Self {
            event_sender,
            error_receiver,
        })
    }

    pub async fn write(&mut self, event: Event) -> Result<(), Error> {
        if let Ok(err) = self.error_receiver.try_recv() {
            return Err(err);
        }

        self.event_sender.send(event).unwrap();
        Ok(())
    }
}

const REPEAT_INTERVAL: Duration = Duration::from_millis(20);
const REPEAT_AFTER: Duration = Duration::from_millis(500);

async fn handle_events(mut receiver: UnboundedReceiver<Event>) -> Result<(), Error> {
    let mut pressed: Option<(Key, Instant)> = None;
    let mut interval = time::interval(REPEAT_INTERVAL);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Some((key, created)) = pressed {
                    if created.elapsed() < REPEAT_AFTER {
                        continue;
                    }

                    write_event(Event::Key { kind: KeyKind::Key(key), direction: Direction::Down })?;
                }
            }
            event = receiver.recv() => {
                let event = match event {
                    Some(event) => event,
                    None => return Ok(()),
                };

                if let Event::Key { kind: KeyKind::Key(key), direction } = event {
                    match direction {
                        Direction::Up => {
                            if pressed.map(|(k, _)| k == key).unwrap_or(false) {
                                pressed = None;
                            }
                        }
                        Direction::Down => {
                            pressed = Some((key, Instant::now()));
                        }
                    }
                }

                write_event(event)?;
            }
        }
    }
}

fn write_event(event: Event) -> Result<(), Error> {
    if let Some(mut events) = event.to_raw() {
        return write_raw(events.as_mut_slice());
    }

    Ok(())
}

fn write_raw(events: &mut [INPUT]) -> Result<(), Error> {
    let written = unsafe {
        winuser::SendInput(
            events.len() as _,
            events.as_mut_ptr(),
            std::mem::size_of_val(&events[0]) as _,
        )
    };

    if written != 1 {
        return Err(Error::last_os_error());
    }

    Ok(())
}
