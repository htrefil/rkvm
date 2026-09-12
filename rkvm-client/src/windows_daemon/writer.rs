use crate::client::RkvmWriter;

use rkvm_input::{event::Event, writer::EventWriter};
use rkvm_net::Update;

use async_trait::async_trait;
use std::io::Error;

pub struct ClientWriter<W> {
    writer: W,
}

impl<W> ClientWriter<W>
where W: RkvmWriter + Send {
    pub fn new(writer: W) -> Self {
        ClientWriter { writer: writer }
    }
}

#[async_trait]
impl<W> EventWriter for ClientWriter<W>
where W: RkvmWriter + Send {
    async fn event(&mut self, id: usize, event: Event) -> Result<(), Error> {
            tracing::debug!("Forward {:?}", event);
            if let Err(e) = self.writer.send(Update::Event {id, event}).await {
                tracing::warn!("Failed to send update to client {:?}", e);
            }
            Ok(())
    }
}
