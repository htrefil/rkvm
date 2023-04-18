use crate::event::{Axis, Direction, Event, EventPack};
use crate::linux::device_id;
use crate::linux::glue::{self, libevdev, libevdev_uinput};
use crate::linux::glue::{input_event, timeval};
use crate::KeyKind;

use std::fs::{File, OpenOptions};
use std::io::Error;
use std::mem::MaybeUninit;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use tokio::io::unix::AsyncFd;
use tokio::task;

pub(crate) struct EventReader {
    file: AsyncFd<File>,
    evdev: *mut libevdev,
    uinput: *mut libevdev_uinput,
}

impl EventReader {
    pub async fn open(path: &Path) -> Result<Self, OpenError> {
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
        let (vendor, product, version) = unsafe {
            (
                glue::libevdev_get_id_vendor(evdev),
                glue::libevdev_get_id_product(evdev),
                glue::libevdev_get_id_version(evdev),
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
        let has_sw = unsafe { glue::libevdev_has_event_type(evdev, glue::EV_SW) == 1 };

        if vendor || has_sw {
            unsafe {
                glue::libevdev_free(evdev);
            }

            return Err(OpenError::NotAppliable);
        }

        unsafe {
            glue::libevdev_set_id_vendor(evdev, device_id::VENDOR as _);
            glue::libevdev_set_id_product(evdev, device_id::PRODUCT as _);
            glue::libevdev_set_id_version(evdev, device_id::VERSION as _);
        }

        let ret = unsafe { glue::libevdev_grab(evdev, glue::libevdev_grab_mode_LIBEVDEV_GRAB) };
        if ret < 0 {
            unsafe {
                glue::libevdev_free(evdev);
            }

            return Err(Error::from_raw_os_error(-ret).into());
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
            unsafe {
                glue::libevdev_free(evdev);
            }

            return Err(Error::from_raw_os_error(-ret).into());
        }

        Ok(Self {
            file,
            evdev,
            uinput: unsafe { uinput.assume_init() },
        })
    }

    pub async fn read(&mut self) -> Result<EventPack, Error> {
        let mut events = EventPack::new();

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
                (glue::EV_KEY, code, 0) => KeyKind::from_raw(code as _).map(|kind| Event::Key {
                    direction: Direction::Up,
                    kind,
                }),
                (glue::EV_KEY, code, 1) => KeyKind::from_raw(code as _).map(|kind| Event::Key {
                    direction: Direction::Down,
                    kind,
                }),
                (glue::EV_SYN, glue::SYN_REPORT, _) => break,
                _ => None,
            };

            if let Some(event) = event {
                events.push(event);
                continue;
            }

            self.write_raw(&raw).await?;
        }

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

        Ok(events)
    }

    async fn read_raw(&mut self) -> Result<input_event, Error> {
        loop {
            let result = self.file.readable().await?.try_io(|_| {
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

            match result {
                Ok(Ok(event)) => return Ok(event),
                Ok(Err(err)) => return Err(err),
                Err(_) => continue, // This means it would block.
            }
        }
    }

    async fn write_raw(&mut self, event: &input_event) -> Result<(), Error> {
        // TODO: This can block.
        let ret = unsafe {
            glue::libevdev_uinput_write_event(
                self.uinput,
                event.type_ as _,
                event.code as _,
                event.value,
            )
        };

        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret).into());
        }

        Ok(())
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

pub enum OpenError {
    NotAppliable,
    Io(Error),
}

impl From<Error> for OpenError {
    fn from(err: Error) -> Self {
        OpenError::Io(err)
    }
}
