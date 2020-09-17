use crate::async_file::{AsyncFile, OpenMode};
use crate::bindings;
use libc::c_int;
use std::io::Error;
use std::os::unix::io::AsRawFd;

pub struct EventWriter {
    file: AsyncFile,
}

impl EventWriter {
    pub async fn new() -> Result<Self, Error> {
        let file = AsyncFile::open("/dev/uinput", OpenMode::Write).await?;
        let fd = file.as_raw_fd();

        for evbit in &[bindings::EV_KEY, bindings::EV_REL] {
            // Doesn't work, UI_SET_KEYBIT not found.
            // Probably too complicated for bindgen to be able to do something with it.
            check_ioctl(unsafe { libc::ioctl(fd, bindings::UI_SET_KEYBIT, evbit) })?;
        }

        Ok(EventWriter { file })
    }
}

fn check_ioctl(ret: c_int) -> Result<(), Error> {
    if ret == -1 {
        return Err(Error::last_os_error());
    }

    Ok(())
}
