use crate::glue::{self, input_event, libevdev};
use mio::event::Evented;
use mio::unix::EventedFd;
use mio::{PollOpt, Ready, Token};
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
    file: EventedFile,
    registration: Registration,
    evdev: *mut libevdev,
}

impl EventReader {
    pub async fn new(path: &Path) -> Result<Self, Error> {
        let path = path.to_owned();

        tokio::task::spawn_blocking(move || Self::open_sync(&path)).await?
    }

    pub async fn read(&mut self) -> Result<input_event, Error> {
        Read { reader: self }.await
    }

    fn open_sync(path: &Path) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .map(|file| EventedFile { file })?;
        let registration = Registration::new(&file)?;

        let mut evdev = std::ptr::null_mut();

        let ret =
            unsafe { glue::libevdev_new_from_fd(file.file.as_raw_fd(), &mut evdev as *mut _) };
        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret));
        }

        let ret = unsafe { glue::libevdev_grab(evdev, glue::libevdev_grab_mode_LIBEVDEV_GRAB) };
        if ret < 0 {
            unsafe { glue::libevdev_free(evdev) };
            return Err(Error::from_raw_os_error(-ret));
        }

        Ok(Self {
            file,
            registration,
            evdev,
        })
    }
}

unsafe impl Send for EventReader {}

impl Drop for EventReader {
    fn drop(&mut self) {
        unsafe { glue::libevdev_free(self.evdev) };
    }
}

struct EventedFile {
    file: File,
}

impl Evented for EventedFile {
    fn register(
        &self,
        poll: &mio::Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<(), Error> {
        EventedFd(&self.file.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &mio::Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<(), Error> {
        EventedFd(&self.file.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> Result<(), Error> {
        EventedFd(&self.file.as_raw_fd()).deregister(poll)
    }
}

struct Read<'a> {
    reader: &'a mut EventReader,
}

impl Future for Read<'_> {
    type Output = Result<input_event, Error>;

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<Self::Output> {
        if let Poll::Pending = self.reader.registration.poll_read_ready(context)? {
            return Poll::Pending;
        }

        let mut event = MaybeUninit::uninit();
        let ret = unsafe {
            glue::libevdev_next_event(
                self.reader.evdev,
                glue::libevdev_read_flag_LIBEVDEV_READ_FLAG_NORMAL,
                event.as_mut_ptr(),
            )
        };
        if ret < 0 {
            return Poll::Ready(Err(Error::from_raw_os_error(-ret)));
        }

        Poll::Ready(Ok(unsafe { event.assume_init() }))
    }
}
