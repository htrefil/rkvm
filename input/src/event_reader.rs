use crate::glue::input_event;
use std::io::Error;
use std::path::Path;

pub(crate) struct EventReader {}

impl EventReader {
    pub async fn new(path: &Path) -> Result<Self, OpenError> {
        todo!()
    }

    pub async fn read(&mut self) -> Result<input_event, Error> {
        todo!()
    }
}

#[derive(Debug)]
pub enum OpenError {
    NotSupported,
    Io(Error),
}
