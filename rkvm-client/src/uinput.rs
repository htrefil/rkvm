use rkvm_input::event::Event;
use rkvm_input::writer::Writer;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::client::{self, Error};
use crate::output::{Device, Output};

// Creates a uinput device for every device of the server.
pub struct Uinput {
    writers: HashMap<usize, Writer>,
}

impl Uinput {
    pub fn new() -> Self {
        Self {
            writers: HashMap::new(),
        }
    }
}

impl Output for Uinput {
    async fn create_device(&mut self, id: usize, device: Device) -> Result<(), Error> {
        let Entry::Vacant(entry) = self.writers.entry(id) else {
            return Err(client::protocol_error(
                "Server created the same device twice",
            ));
        };

        let writer = async {
            Writer::builder(device.bus_type)?
                .name(&device.name)
                .vendor(device.vendor)
                .product(device.product)
                .version(device.version)
                .rel(device.rel)?
                .abs(device.abs)?
                .key(device.keys)?
                .property(device.properties)?
                .delay(device.delay)?
                .period(device.period)?
                .build()
                .await
        }
        .await
        .map_err(Error::Input)?;

        entry.insert(writer);

        Ok(())
    }

    async fn destroy_device(&mut self, id: usize) -> Result<(), Error> {
        if self.writers.remove(&id).is_none() {
            return Err(client::protocol_error(
                "Server destroyed a nonexistent device",
            ));
        }

        Ok(())
    }

    async fn write(&mut self, id: usize, event: &Event) -> Result<(), Error> {
        let writer = self.writers.get_mut(&id).ok_or_else(|| {
            client::protocol_error("Server sent an event to a nonexistent device")
        })?;

        writer.write(event).await.map_err(Error::Input)
    }
}
