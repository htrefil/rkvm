use crate::glue::{self, libevdev};

use std::fs::File;
use std::io::{Error, ErrorKind};
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::ptr::NonNull;
use tokio::fs::OpenOptions;
use tokio::io::unix::AsyncFd;

pub struct Evdev {
    evdev: NonNull<libevdev>,
    file: Option<AsyncFd<File>>,
}

impl Evdev {
    pub fn new() -> Result<Self, Error> {
        let evdev = unsafe { glue::libevdev_new() };
        let evdev = NonNull::new(evdev)
            .ok_or_else(|| Error::new(ErrorKind::Other, "Failed to create device"))?;

        Ok(Self { evdev, file: None })
    }

    pub async fn open(path: &Path) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .await?
            .into_std()
            .await;

        let file = AsyncFd::new(file)?;

        let mut evdev = MaybeUninit::uninit();

        let ret = unsafe { glue::libevdev_new_from_fd(file.as_raw_fd(), evdev.as_mut_ptr()) };
        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret).into());
        }

        let evdev = unsafe { evdev.assume_init() };
        let evdev = unsafe { NonNull::new_unchecked(evdev) };

        Ok(Self {
            evdev,
            file: Some(file),
        })
    }

    pub fn file(&self) -> Option<&AsyncFd<File>> {
        self.file.as_ref()
    }

    pub fn as_ptr(&self) -> *mut libevdev {
        self.evdev.as_ptr()
    }
}

impl Drop for Evdev {
    fn drop(&mut self) {
        unsafe {
            glue::libevdev_free(self.evdev.as_ptr());
        }
    }
}

unsafe impl Send for Evdev {}

unsafe impl Sync for Evdev {}
