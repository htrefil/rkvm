use crate::event::Event;
use crate::glue::input_event;
use std::io::Error;

pub struct EventWriter {}

impl EventWriter {
    pub async fn new() -> Result<Self, Error> {
        todo!()
    }

    pub async fn write(&mut self, event: Event) -> Result<(), Error> {
        self.write_raw(event.to_raw()).await
    }

    pub(crate) async fn write_raw(&mut self, event: input_event) -> Result<(), Error> {
        todo!()
    }
}
