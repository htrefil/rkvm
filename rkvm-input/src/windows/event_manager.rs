use crate::event::Event;
use std::io::{Error, ErrorKind};

pub struct EventManager(());

impl EventManager {
    pub async fn new() -> Result<Self, Error> {
        Err(Error::new(ErrorKind::Other, "Not implemented"))
    }

    pub async fn read(&mut self) -> Result<Event, Error> {
        todo!()
    }

    pub async fn write(&mut self, _event: Event) -> Result<(), Error> {
        todo!()
    }
}
