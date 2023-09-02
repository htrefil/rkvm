use crate::evdev::Evdev;
use crate::glue::{self, libevdev_uinput};

use std::fs::File;
use std::io::Error;
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::ptr::NonNull;
use tokio::fs::OpenOptions;
use tokio::io::unix::AsyncFd;

pub struct Uinput {
    file: AsyncFd<File>,
    uinput: NonNull<libevdev_uinput>,
}

impl Uinput {
    pub async fn from_evdev(evdev: &Evdev) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uinput")
            .await?
            .into_std()
            .await;

        let file = AsyncFd::new(file)?;

        let mut uinput = MaybeUninit::uninit();

        let ret = unsafe {
            glue::libevdev_uinput_create_from_device(
                evdev.as_ptr(),
                file.as_raw_fd(),
                uinput.as_mut_ptr(),
            )
        };

        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret));
        }

        let uinput = unsafe { uinput.assume_init() };
        let uinput = unsafe { NonNull::new_unchecked(uinput) };

        Ok(Self { file, uinput })
    }

    pub fn file(&self) -> &AsyncFd<File> {
        &self.file
    }

    pub fn as_ptr(&self) -> *mut libevdev_uinput {
        self.uinput.as_ptr()
    }
}

impl Drop for Uinput {
    fn drop(&mut self) {
        unsafe {
            glue::libevdev_uinput_destroy(self.uinput.as_ptr());
        }
    }
}

unsafe impl Send for Uinput {}

unsafe impl Sync for Uinput {}
