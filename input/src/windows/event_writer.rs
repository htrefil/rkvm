use crate::event::Event;
use std::io::Error;
use winapi::um::winuser::{self, INPUT};

pub struct EventWriter(());

impl EventWriter {
    pub async fn new() -> Result<Self, Error> {
        Ok(Self(()))
    }

    pub async fn write(&mut self, event: Event) -> Result<(), Error> {
        if let Some(mut events) = event.to_raw() {
            return self.write_raw(events.as_mut_slice());
        }

        Ok(())
    }

    fn write_raw(&mut self, events: &mut [INPUT]) -> Result<(), Error> {
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
}
