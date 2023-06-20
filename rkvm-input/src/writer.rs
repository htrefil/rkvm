use crate::abs::{AbsAxis, AbsEvent, AbsInfo};
use crate::event::Event;
use crate::glue::{self, input_absinfo, libevdev, libevdev_uinput};
use crate::key::{Key, KeyEvent};
use crate::rel::{RelAxis, RelEvent};

use std::ffi::{CStr, OsStr};
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind};
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::prelude::OpenOptionsExt;
use std::path::Path;
use std::ptr;
use std::ptr::NonNull;
use tokio::io::unix::AsyncFd;

pub struct Writer {
    file: AsyncFd<File>,
    uinput: NonNull<libevdev_uinput>,
}

impl Writer {
    pub fn builder() -> Result<WriterBuilder, Error> {
        WriterBuilder::new()
    }

    pub async fn write(&mut self, event: &Event) -> Result<(), Error> {
        let (r#type, code, value) = match event {
            Event::Rel(RelEvent { axis, value }) => (glue::EV_REL, axis.to_raw(), *value),
            Event::Abs(event) => match event {
                AbsEvent::Axis { axis, value } => (glue::EV_ABS, axis.to_raw(), *value),
                AbsEvent::MtToolType { value } => {
                    (glue::EV_ABS, glue::ABS_MT_TOOL_TYPE as _, value.to_raw())
                }
                AbsEvent::MtBlobId { value } => (glue::EV_ABS, glue::ABS_MT_BLOB_ID as _, *value),
            },
            Event::Key(KeyEvent { down, key }) => (glue::EV_KEY, key.to_raw(), *down as _),
            Event::Sync(event) => (glue::EV_SYN, event.to_raw(), 0),
        };

        self.write_raw(r#type as _, code, value).await?;
        Ok(())
    }

    pub fn path(&self) -> Option<&Path> {
        let path = unsafe { glue::libevdev_uinput_get_devnode(self.uinput.as_ptr()) };
        if path.is_null() {
            return None;
        }

        let path = unsafe { CStr::from_ptr(path) };
        let path = OsStr::from_bytes(path.to_bytes());
        let path = Path::new(path);

        Some(path)
    }

    pub(crate) unsafe fn from_evdev(evdev: *const libevdev) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uinput")
            .and_then(AsyncFd::new)?;

        let mut uinput = MaybeUninit::uninit();

        let ret = unsafe {
            glue::libevdev_uinput_create_from_device(evdev, file.as_raw_fd(), uinput.as_mut_ptr())
        };

        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret));
        }

        let uinput = unsafe { uinput.assume_init() };
        let uinput = NonNull::new(uinput).unwrap();

        Ok(Self { file, uinput })
    }

    pub(crate) async fn write_raw(
        &mut self,
        r#type: u16,
        code: u16,
        value: i32,
    ) -> Result<(), Error> {
        loop {
            let result = self.file.writable().await?.try_io(|_| {
                let ret = unsafe {
                    glue::libevdev_uinput_write_event(
                        self.uinput.as_ptr(),
                        r#type as _,
                        code as _,
                        value,
                    )
                };

                if ret < 0 {
                    return Err(Error::from_raw_os_error(-ret).into());
                }

                Ok(())
            });

            match result {
                Ok(result) => return result,
                Err(_) => continue, // This means it would block.
            }
        }
    }
}

pub struct WriterBuilder {
    evdev: NonNull<libevdev>,
}

impl WriterBuilder {
    pub fn new() -> Result<Self, Error> {
        let evdev = unsafe { glue::libevdev_new() };
        let evdev = NonNull::new(evdev)
            .ok_or_else(|| Error::new(ErrorKind::Other, "Failed to create device"))?;

        unsafe {
            glue::libevdev_set_id_bustype(evdev.as_ptr(), glue::BUS_VIRTUAL as _);
        }

        Ok(Self { evdev })
    }

    pub fn name(&mut self, name: &CStr) -> &mut Self {
        unsafe {
            glue::libevdev_set_name(self.evdev.as_ptr(), name.as_ptr());
        }

        self
    }

    pub fn vendor(&mut self, value: u16) -> &mut Self {
        unsafe {
            glue::libevdev_set_id_vendor(self.evdev.as_ptr(), value as _);
        }

        self
    }

    pub fn product(&mut self, value: u16) -> &mut Self {
        unsafe {
            glue::libevdev_set_id_product(self.evdev.as_ptr(), value as _);
        }

        self
    }

    pub fn version(&mut self, value: u16) -> &mut Self {
        unsafe {
            glue::libevdev_set_id_version(self.evdev.as_ptr(), value as _);
        }

        self
    }

    pub fn rel<T: IntoIterator<Item = RelAxis>>(&mut self, items: T) -> Result<&mut Self, Error> {
        for axis in items {
            let ret = unsafe {
                glue::libevdev_enable_event_code(
                    self.evdev.as_ptr(),
                    glue::EV_REL,
                    axis.to_raw() as _,
                    ptr::null(),
                )
            };

            if ret < 0 {
                return Err(Error::from_raw_os_error(-ret));
            }
        }

        Ok(self)
    }

    pub fn abs<T: IntoIterator<Item = (AbsAxis, AbsInfo)>>(
        &mut self,
        items: T,
    ) -> Result<&mut Self, Error> {
        let ret = unsafe {
            glue::libevdev_enable_event_code(
                self.evdev.as_ptr(),
                glue::EV_SYN,
                glue::SYN_MT_REPORT,
                ptr::null(),
            )
        };

        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret));
        }

        for (axis, info) in items {
            let info = input_absinfo {
                value: info.min,
                minimum: info.min,
                maximum: info.max,
                fuzz: info.fuzz,
                flat: info.flat,
                resolution: info.resolution,
            };

            let ret = unsafe {
                glue::libevdev_enable_event_code(
                    self.evdev.as_ptr(),
                    glue::EV_ABS,
                    axis.to_raw() as _,
                    &info as *const _ as *const _,
                )
            };

            if ret < 0 {
                return Err(Error::from_raw_os_error(-ret));
            }
        }

        Ok(self)
    }

    pub fn key<T: IntoIterator<Item = Key>>(&mut self, items: T) -> Result<&mut Self, Error> {
        for key in items {
            let ret = unsafe {
                glue::libevdev_enable_event_code(
                    self.evdev.as_ptr(),
                    glue::EV_KEY,
                    key.to_raw() as _,
                    ptr::null(),
                )
            };

            if ret < 0 {
                return Err(Error::from_raw_os_error(-ret));
            }
        }

        Ok(self)
    }

    pub fn build(&self) -> Result<Writer, Error> {
        unsafe { Writer::from_evdev(self.evdev.as_ref()) }
    }
}

unsafe impl Send for Writer {}

impl Drop for WriterBuilder {
    fn drop(&mut self) {
        unsafe {
            glue::libevdev_free(self.evdev.as_ptr());
        }
    }
}
