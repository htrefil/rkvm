use crate::event::Event;
use crate::glue::{self, libevdev, libevdev_uinput};
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind};
use std::mem::MaybeUninit;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use tokio::io::unix::AsyncFd;

pub(crate) struct EventReader {
    file: AsyncFd<File>,
    evdev: *mut libevdev,
    uinput: *mut libevdev_uinput,
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
            .open(path)
            .and_then(AsyncFd::new)?;

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

        let mut uinput = MaybeUninit::uninit();
        let ret = unsafe {
            glue::libevdev_uinput_create_from_device(
                evdev,
                glue::libevdev_uinput_open_mode_LIBEVDEV_UINPUT_OPEN_MANAGED,
                uinput.as_mut_ptr(),
            )
        };

        if ret < 0 {
            unsafe { glue::libevdev_free(evdev) };
            return Err(Error::from_raw_os_error(-ret));
        }

        let uinput = unsafe { uinput.assume_init() };
        Ok(Self {
            file,
            evdev,
            uinput,
        })
    }

    pub async fn read(&mut self) -> Result<Event, Error> {
        loop {
            let result = self.file.readable().await?.with_io(|| {
                let mut event = MaybeUninit::uninit();
                let ret = unsafe {
                    glue::libevdev_next_event(
                        self.evdev,
                        glue::libevdev_read_flag_LIBEVDEV_READ_FLAG_NORMAL,
                        event.as_mut_ptr(),
                    )
                };

                if ret < 0 {
                    return Err(Error::from_raw_os_error(-ret));
                }

                let event = unsafe { event.assume_init() };
                Ok(event)
            });

            let event = match result {
                Ok(event) => event,
                Err(ref err) if err.kind() == ErrorKind::WouldBlock => continue,
                Err(err) => return Err(err),
            };

            if let Some(event) = Event::from_raw(event) {
                return Ok(event);
            }

            // Not understood, write it back.
            let ret = unsafe {
                glue::libevdev_uinput_write_event(
                    self.uinput as *const _,
                    event.type_ as _,
                    event.code as _,
                    event.value,
                )
            };

            if ret < 0 {
                return Err(Error::from_raw_os_error(-ret));
            }
        }
    }
}

impl Drop for EventReader {
    fn drop(&mut self) {
        unsafe {
            glue::libevdev_uinput_destroy(self.uinput);
            glue::libevdev_free(self.evdev);
        }
    }
}

unsafe impl Send for EventReader {}
