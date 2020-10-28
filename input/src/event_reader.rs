use crate::glue::{self, input_event, libevdev};
use mio::unix::EventedFd;
use std::fs::{File, OpenOptions};
use std::future::Future;
use std::io::Error;
use std::mem::MaybeUninit;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::Registration;

pub(crate) struct EventReader {
    file: File,
    registration: Registration,
    evdev: *mut libevdev,
}

impl EventReader {
    pub async fn new(path: &Path) -> Result<Self, Error> {
        let path = path.to_owned();
        tokio::task::spawn_blocking(move || Self::new_sync(&path)).await?
    }

    fn new_sync(path: &Path) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)?;
        let registration = Registration::new(&EventedFd(&file.as_raw_fd()))?;

        let mut evdev = MaybeUninit::uninit();
        let ret = unsafe { glue::libevdev_new_from_fd(file.as_raw_fd(), evdev.as_mut_ptr()) };
        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret));
        }

        let evdev = unsafe { evdev.assume_init() };
        let ret = unsafe { glue::libevdev_grab(evdev, glue::libevdev_grab_mode_LIBEVDEV_GRAB) };
        if ret < 0 {
            unsafe {
                glue::libevdev_free(evdev);
            }

            return Err(Error::from_raw_os_error(-ret));
        }

        Ok(Self {
            file,
            registration,
            evdev,
        })
    }

    pub async fn read(&mut self) -> Result<input_event, Error> {
        Read {
            reader: self,
            polling: false,
        }
        .await
    }
}

impl Drop for EventReader {
    fn drop(&mut self) {
        unsafe {
            glue::libevdev_free(self.evdev);
        }
    }
}

unsafe impl Send for EventReader {}

struct Read<'a> {
    reader: &'a mut EventReader,
    polling: bool,
}

impl Future for Read<'_> {
    type Output = Result<input_event, Error>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<Self::Output> {
        if self.polling {
            if let Poll::Pending = self.reader.registration.poll_read_ready(context)? {
                return Poll::Pending;
            }
        }

        let mut event = MaybeUninit::uninit();
        let ret = unsafe {
            glue::libevdev_next_event(
                self.reader.evdev,
                glue::libevdev_read_flag_LIBEVDEV_READ_FLAG_NORMAL,
                event.as_mut_ptr(),
            )
        };

        if !self.polling && ret == -libc::EAGAIN {
            self.polling = true;
            return self.poll(context);
        }

        self.polling = false;

        if ret < 0 {
            return Poll::Ready(Err(Error::from_raw_os_error(-ret)));
        }

        let event = unsafe { event.assume_init() };
        Poll::Ready(Ok(event))
    }
}
