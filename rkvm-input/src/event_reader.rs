use crate::device_id;
use crate::event::{Axis, Direction, Event, EventPack};
use crate::glue::{self, libevdev, libevdev_uinput};
use crate::glue::{input_event, timeval};
use crate::KeyKind;

use std::fs::{File, OpenOptions};
use std::io::Error;
use std::mem::MaybeUninit;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::ptr::NonNull;
use tokio::io::unix::AsyncFd;
use tokio::task;

pub(crate) struct EventReader {
    evdev_file: AsyncFd<File>,
    evdev_handle: NonNull<libevdev>,
    uinput_file: AsyncFd<File>,
    uinput_handle: NonNull<libevdev_uinput>,
}

impl EventReader {
    pub async fn open(path: &Path) -> Result<Self, OpenError> {
        let path = path.to_owned();
        task::spawn_blocking(move || Self::open_sync(&path))
            .await
            .map_err(|err| OpenError::Io(err.into()))?
    }

    fn open_sync(path: &Path) -> Result<Self, OpenError> {
        let evdev_file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .and_then(AsyncFd::new)?;

        let mut evdev_handle = MaybeUninit::uninit();

        let ret = unsafe {
            glue::libevdev_new_from_fd(evdev_file.as_raw_fd(), evdev_handle.as_mut_ptr())
        };

        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret).into());
        }

        let evdev_handle = unsafe { evdev_handle.assume_init() };
        let evdev_handle = NonNull::new(evdev_handle).unwrap();

        let (vendor, product, version) = unsafe {
            (
                glue::libevdev_get_id_vendor(evdev_handle.as_ptr()),
                glue::libevdev_get_id_product(evdev_handle.as_ptr()),
                glue::libevdev_get_id_version(evdev_handle.as_ptr()),
            )
        };

        // Check if we're not opening our own virtual device.
        let vendor = vendor == device_id::VENDOR as _
            && product == device_id::PRODUCT as _
            && version == device_id::VERSION as _;

        // "Upon binding to a device or resuming from suspend, a driver must report
        // the current switch state. This ensures that the device, kernel, and userspace
        // state is in sync."
        // We have no way of knowing that.
        let has_sw =
            unsafe { glue::libevdev_has_event_type(evdev_handle.as_ptr(), glue::EV_SW) == 1 };

        if vendor || has_sw {
            unsafe {
                glue::libevdev_free(evdev_handle.as_ptr());
            }

            return Err(OpenError::NotAppliable);
        }

        unsafe {
            glue::libevdev_set_id_vendor(evdev_handle.as_ptr(), device_id::VENDOR as _);
            glue::libevdev_set_id_product(evdev_handle.as_ptr(), device_id::PRODUCT as _);
            glue::libevdev_set_id_version(evdev_handle.as_ptr(), device_id::VERSION as _);
        }

        let ret = unsafe {
            glue::libevdev_grab(
                evdev_handle.as_ptr(),
                glue::libevdev_grab_mode_LIBEVDEV_GRAB,
            )
        };

        if ret < 0 {
            unsafe {
                glue::libevdev_free(evdev_handle.as_ptr());
            }

            return Err(Error::from_raw_os_error(-ret).into());
        }

        // libevdev opens /dev/uinput with O_RDWR.
        let uinput_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/uinput")
            .and_then(AsyncFd::new)?;

        let mut uinput_handle = MaybeUninit::uninit();

        let ret = unsafe {
            glue::libevdev_uinput_create_from_device(
                evdev_handle.as_ptr(),
                uinput_file.as_raw_fd(),
                uinput_handle.as_mut_ptr(),
            )
        };

        if ret < 0 {
            unsafe {
                glue::libevdev_free(evdev_handle.as_ptr());
            }

            return Err(Error::from_raw_os_error(-ret).into());
        }

        let uinput_handle = unsafe { uinput_handle.assume_init() };
        let uinput_handle = NonNull::new(uinput_handle).unwrap();

        Ok(Self {
            evdev_file,
            evdev_handle,
            uinput_file,
            uinput_handle,
        })
    }

    pub async fn read(&mut self) -> Result<EventPack, Error> {
        loop {
            let mut events = EventPack::new();
            let mut wrote = false;

            loop {
                let raw = self.read_raw().await?;
                let event = match (raw.type_ as _, raw.code as _, raw.value) {
                    // These should not be propagated, it will result in double scrolling otherwise.
                    (glue::EV_REL, glue::REL_HWHEEL | glue::REL_WHEEL, _) => continue,
                    (glue::EV_REL, glue::REL_HWHEEL_HI_RES, value) => Some(Event::MouseScroll {
                        axis: Axis::X,
                        delta: value,
                    }),
                    (glue::EV_REL, glue::REL_WHEEL_HI_RES, value) => Some(Event::MouseScroll {
                        axis: Axis::Y,
                        delta: value,
                    }),
                    (glue::EV_REL, glue::REL_X, value) => Some(Event::MouseMove {
                        axis: Axis::X,
                        delta: value,
                    }),
                    (glue::EV_REL, glue::REL_Y, value) => Some(Event::MouseMove {
                        axis: Axis::Y,
                        delta: value,
                    }),
                    (glue::EV_KEY, code, 0) => {
                        KeyKind::from_raw(code as _).map(|kind| Event::Key {
                            direction: Direction::Up,
                            kind,
                        })
                    }
                    (glue::EV_KEY, code, 1) => {
                        KeyKind::from_raw(code as _).map(|kind| Event::Key {
                            direction: Direction::Down,
                            kind,
                        })
                    }
                    (glue::EV_SYN, glue::SYN_REPORT, _) => break,
                    _ => None,
                };

                if let Some(event) = event {
                    events.push(event);
                    continue;
                }

                self.write_raw(&raw).await?;
                wrote = true;
            }

            // Send an EV_SYN only if we actually wrote something back.
            if wrote {
                self.write_raw(&input_event {
                    type_: glue::EV_SYN as _,
                    code: glue::SYN_REPORT as _,
                    value: 0,
                    time: timeval {
                        tv_sec: 0,
                        tv_usec: 0,
                    },
                })
                .await?;
            }

            if !events.is_empty() {
                return Ok(events);
            }

            // At this point, we received an EV_SYN, but no actual events useful to us, so try again.
        }
    }

    async fn read_raw(&mut self) -> Result<input_event, Error> {
        loop {
            let result = self.evdev_file.readable().await?.try_io(|_| {
                let mut event = MaybeUninit::uninit();
                let ret = unsafe {
                    glue::libevdev_next_event(
                        self.evdev_handle.as_ptr(),
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

            match result {
                Ok(result) => return result,
                Err(_) => continue, // This means it would block.
            }
        }
    }

    async fn write_raw(&mut self, event: &input_event) -> Result<(), Error> {
        loop {
            let result = self.uinput_file.writable().await?.try_io(|_| {
                let ret = unsafe {
                    glue::libevdev_uinput_write_event(
                        self.uinput_handle.as_ptr(),
                        event.type_ as _,
                        event.code as _,
                        event.value,
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

impl Drop for EventReader {
    fn drop(&mut self) {
        unsafe {
            glue::libevdev_uinput_destroy(self.uinput_handle.as_ptr());
            glue::libevdev_free(self.evdev_handle.as_ptr());
        }
    }
}

unsafe impl Send for EventReader {}

pub enum OpenError {
    NotAppliable,
    Io(Error),
}

impl From<Error> for OpenError {
    fn from(err: Error) -> Self {
        OpenError::Io(err)
    }
}
