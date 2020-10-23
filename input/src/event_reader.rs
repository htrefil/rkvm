use crate::async_file::{AsyncFile, OpenMode};
use crate::setup::{self, input_event};
use std::io::Error;
use std::mem;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use tokio::io::AsyncReadExt;

pub(crate) struct EventReader {
    file: AsyncFile,
}

impl EventReader {
    pub async fn new(path: &Path) -> Result<Self, OpenError> {
        let file = AsyncFile::open(path, OpenMode::Read)
            .await
            .map_err(OpenError::Io)?;
        if unsafe { setup::setup_read_fd(file.as_raw_fd()) == 0 } {
            let err = Error::last_os_error();
            if err.raw_os_error() == Some(libc::ENOTTY) {
                return Err(OpenError::NotSupported);
            }

            return Err(OpenError::Io(err));
        }

        Ok(Self { file })
    }

    pub async fn read(&mut self) -> Result<input_event, Error> {
        let mut buffer = [0u8; mem::size_of::<input_event>()];
        self.file
            .read_exact(&mut buffer)
            .await
            .map(|_| unsafe { mem::transmute(buffer) })
    }
}

#[derive(Debug)]
pub enum OpenError {
    NotSupported,
    Io(Error),
}
