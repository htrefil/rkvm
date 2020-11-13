use crate::event::Event;
use std::io::Error;
use winapi::um::winuser::{self, INPUT};

pub struct EventWriter(());

impl EventWriter {
    pub async fn new() -> Result<Self, Error> {
        Ok(Self(()))
    }

    pub async fn write(&mut self, event: Event) -> Result<(), Error> {
        if let Some(event) = event.to_raw() {
            return self.write_raw(event).await;
        }

        Ok(())
    }

    async fn write_raw(&mut self, mut event: INPUT) -> Result<(), Error> {
        let written = unsafe {
            winuser::SendInput(1, &mut event as *mut _, std::mem::size_of_val(&event) as _)
        };

        if written != 1 {
            return Err(Error::last_os_error());
        }

        Ok(())
    }
}
