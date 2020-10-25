use crate::async_file::{AsyncFile, OpenMode};
use crate::event::Event;
use crate::setup::{self, input_event, timeval};
use std::io::Error;
use std::mem;
use std::os::unix::io::AsRawFd;
use tokio::io::AsyncWriteExt;

pub struct EventWriter {
    file: AsyncFile,
}

impl EventWriter {
    pub async fn new() -> Result<Self, Error> {
        let file = AsyncFile::open("/dev/uinput", OpenMode::Write).await?;
        if unsafe { setup::setup_write_fd(file.as_raw_fd()) == 0 } {
            return Err(Error::last_os_error());
        }

        Ok(Self { file })
    }

    pub async fn write(&mut self, event: Event) -> Result<(), Error> {
        self.write_raw(event.to_raw()).await
    }

    pub(crate) async fn write_raw(&mut self, mut event: input_event) -> Result<(), Error> {
        event.time = timeval {
            tv_sec: 0,
            tv_usec: 0,
        };

        let data: [u8; mem::size_of::<input_event>()] = unsafe { mem::transmute(event) };
        self.file.write_all(&data).await
    }
}

impl Drop for EventWriter {
    fn drop(&mut self) {
        unsafe {
            setup::destroy_write_fd(self.file.as_raw_fd());
        }
    }
}
