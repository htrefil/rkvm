mod caps;

pub use caps::{AbsCaps, KeyCaps, RelCaps};

use crate::abs::{AbsAxis, AbsEvent};
use crate::event::{Event, Packet};
use crate::glue::{self, libevdev};
use crate::key::{Key, KeyEvent};
use crate::rel::{RelAxis, RelEvent};
use crate::writer::Writer;

use std::ffi::CStr;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind};
use std::mem;
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::os::unix::prelude::OpenOptionsExt;
use std::path::Path;
use std::ptr::NonNull;
use thiserror::Error;
use tokio::io::unix::AsyncFd;
use tokio::task;

pub struct Interceptor {
    file: AsyncFd<File>,
    evdev: NonNull<libevdev>,
    writer: Writer,
    // The state of `read` is stored here to make it cancel safe.
    events: Packet,
    wrote: bool,
    dropped: bool,
    writing: Option<Writing>,
}

impl Interceptor {
    pub async fn read(&mut self) -> Result<Packet, Error> {
        if let Some(writing) = self.writing {
            let (r#type, code, value) = match writing {
                Writing::Event {
                    r#type,
                    code,
                    value,
                } => (r#type, code, value),
                Writing::Sync => (glue::EV_SYN as _, glue::SYN_REPORT as _, 0),
            };

            self.writer.write_raw(r#type, code, value).await?;
            self.writing = None;
        }

        loop {
            loop {
                let (r#type, code, value) = self.read_raw().await?;
                let event = match r#type as _ {
                    glue::EV_REL if !self.dropped => {
                        RelAxis::from_raw(code).map(|axis| Event::Rel(RelEvent { axis, value }))
                    }
                    glue::EV_ABS if !self.dropped => {
                        AbsAxis::from_raw(code).map(|axis| Event::Abs(AbsEvent { axis, value }))
                    }
                    glue::EV_KEY if !self.dropped && (value == 0 || value == 1) => {
                        Key::from_raw(code).map(|key| {
                            Event::Key(KeyEvent {
                                key,
                                down: value == 1,
                            })
                        })
                    }
                    glue::EV_SYN => match code as _ {
                        glue::SYN_REPORT => {
                            if self.dropped {
                                self.dropped = false;
                                continue;
                            }

                            break;
                        }
                        glue::SYN_DROPPED => {
                            log::warn!(
                                "Dropped {} event{}",
                                self.events.len(),
                                if self.events.len() == 1 { "" } else { "s" }
                            );

                            self.events.clear();
                            self.dropped = true;
                            continue;
                        }
                        _ => continue,
                    },
                    _ => None,
                };

                if let Some(event) = event {
                    self.events.push(event);
                    continue;
                }

                log::trace!(
                    "Writing back unknown event (type {}, code {}, value {})",
                    r#type,
                    code,
                    value
                );

                self.writing = Some(Writing::Event {
                    r#type,
                    code,
                    value,
                });
                self.writer.write_raw(r#type, code, value).await?;
                self.writing = None;
                self.wrote = true;
            }

            // Write an EV_SYN only if we actually wrote something back.
            if self.wrote {
                self.writing = Some(Writing::Sync);
                self.writer
                    .write_raw(glue::EV_SYN as _, glue::SYN_REPORT as _, 0)
                    .await?;
                self.writing = None;
                self.wrote = false;
            }

            if !self.events.is_empty() {
                return Ok(mem::take(&mut self.events));
            }

            // At this point, we received an EV_SYN, but no actual events useful to us, so try again.
        }
    }

    pub async fn write(&mut self, events: &[Event]) -> Result<(), Error> {
        self.writer.write(events).await
    }

    pub fn name(&self) -> &CStr {
        let name = unsafe { glue::libevdev_get_name(self.evdev.as_ptr()) };
        let name = unsafe { CStr::from_ptr(name) };

        name
    }

    pub fn vendor(&self) -> u16 {
        unsafe { glue::libevdev_get_id_vendor(self.evdev.as_ptr()) as _ }
    }

    pub fn product(&self) -> u16 {
        unsafe { glue::libevdev_get_id_product(self.evdev.as_ptr()) as _ }
    }

    pub fn version(&self) -> u16 {
        unsafe { glue::libevdev_get_id_version(self.evdev.as_ptr()) as _ }
    }

    pub fn rel(&self) -> RelCaps {
        RelCaps::new(self)
    }

    pub fn abs(&self) -> AbsCaps {
        AbsCaps::new(self)
    }

    pub fn key(&self) -> KeyCaps {
        KeyCaps::new(self)
    }

    async fn read_raw(&mut self) -> Result<(u16, u16, i32), Error> {
        loop {
            let result = self.file.readable().await?.try_io(|_| {
                let mut event = MaybeUninit::uninit();
                let ret = unsafe {
                    glue::libevdev_next_event(
                        self.evdev.as_ptr(),
                        glue::libevdev_read_flag_LIBEVDEV_READ_FLAG_NORMAL,
                        event.as_mut_ptr(),
                    )
                };

                if ret < 0 {
                    // ENODEV means that the device got disconnected.
                    // However, ErrorKind doesn't have support for it yet,
                    // so translate to BrokenPipe here to not introduce
                    // platform specific code to rkvm-server.
                    let err = if ret == -libc::ENODEV {
                        Error::new(ErrorKind::BrokenPipe, "Device disconnected")
                    } else {
                        Error::from_raw_os_error(-ret)
                    };

                    return Err(err);
                }

                let event = unsafe { event.assume_init() };
                Ok((event.type_, event.code, event.value))
            });

            match result {
                Ok(result) => return result,
                Err(_) => continue, // This means it would block.
            }
        }
    }

    pub(crate) async fn open(path: &Path) -> Result<Self, OpenError> {
        let path = path.to_owned();
        task::spawn_blocking(move || Self::open_sync(&path))
            .await
            .map_err(|err| OpenError::Io(err.into()))?
    }

    fn open_sync(path: &Path) -> Result<Self, OpenError> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .and_then(AsyncFd::new)?;

        let mut evdev = MaybeUninit::uninit();

        let ret = unsafe { glue::libevdev_new_from_fd(file.as_raw_fd(), evdev.as_mut_ptr()) };
        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret).into());
        }

        let evdev = unsafe { evdev.assume_init() };
        let evdev = NonNull::new(evdev).unwrap();

        // "Upon binding to a device or resuming from suspend, a driver must report
        // the current switch state. This ensures that the device, kernel, and userspace
        // state is in sync."
        // We have no way of knowing that.
        let sw = unsafe { glue::libevdev_has_event_type(evdev.as_ptr(), glue::EV_SW) };

        // Check if we're not opening our own virtual device.
        let bus_type = unsafe { glue::libevdev_get_id_bustype(evdev.as_ptr()) };

        if bus_type == glue::BUS_VIRTUAL as _ || sw == 1 {
            unsafe {
                glue::libevdev_free(evdev.as_ptr());
            }

            return Err(OpenError::NotAppliable);
        }

        unsafe {
            glue::libevdev_set_id_bustype(evdev.as_ptr(), glue::BUS_VIRTUAL as _);
        }

        let ret =
            unsafe { glue::libevdev_grab(evdev.as_ptr(), glue::libevdev_grab_mode_LIBEVDEV_GRAB) };

        if ret < 0 {
            unsafe {
                glue::libevdev_free(evdev.as_ptr());
            }

            return Err(Error::from_raw_os_error(-ret).into());
        }

        let writer = unsafe { Writer::from_evdev(evdev.as_ptr()) };
        let writer = match writer {
            Ok(writer) => writer,
            Err(err) => {
                unsafe {
                    glue::libevdev_free(evdev.as_ptr());
                }

                return Err(err.into());
            }
        };

        Ok(Self {
            file,
            evdev,
            writer,

            events: Packet::new(),
            wrote: false,
            dropped: false,
            writing: None,
        })
    }
}

impl Drop for Interceptor {
    fn drop(&mut self) {
        unsafe {
            glue::libevdev_free(self.evdev.as_ptr());
        }
    }
}

unsafe impl Send for Interceptor {}

#[derive(Clone, Copy)]
enum Writing {
    Event { r#type: u16, code: u16, value: i32 },
    Sync,
}

#[derive(Error, Debug)]
pub(crate) enum OpenError {
    #[error("Not appliable")]
    NotAppliable,
    #[error(transparent)]
    Io(#[from] Error),
}
