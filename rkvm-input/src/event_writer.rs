use crate::device_id;
use crate::event::Event;
use crate::glue::{self, input_event, libevdev, libevdev_uinput, timeval};
use crate::{Axis, Direction};

use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind};
use std::mem::MaybeUninit;
use std::ops::RangeInclusive;
use std::os::fd::AsRawFd;
use std::ptr::NonNull;
use tokio::io::unix::AsyncFd;

pub struct EventWriter {
    evdev_handle: NonNull<libevdev>,
    uinput_file: AsyncFd<File>,
    uinput_handle: NonNull<libevdev_uinput>,
}

impl EventWriter {
    pub async fn new() -> Result<Self, Error> {
        tokio::task::spawn_blocking(Self::new_sync).await?
    }

    fn new_sync() -> Result<Self, Error> {
        let evdev_handle = unsafe { glue::libevdev_new() };
        let evdev_handle = NonNull::new(evdev_handle)
            .ok_or_else(|| Error::new(ErrorKind::Other, "Failed to create device"))?;

        if let Err(err) = unsafe { setup_evdev(evdev_handle.as_ptr()) } {
            unsafe {
                glue::libevdev_free(evdev_handle.as_ptr());
            }

            return Err(err);
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

            return Err(Error::from_raw_os_error(-ret));
        }

        let uinput_handle = unsafe { uinput_handle.assume_init() };
        let uinput_handle = NonNull::new(uinput_handle).unwrap();

        Ok(Self {
            evdev_handle,
            uinput_file,
            uinput_handle,
        })
    }

    pub async fn write(&mut self, events: &[Event]) -> Result<(), Error> {
        let events = events
            .iter()
            .map(|event| match event {
                Event::MouseScroll {
                    axis: Axis::X,
                    delta,
                } => (glue::EV_REL, glue::REL_HWHEEL_HI_RES as _, *delta),
                Event::MouseScroll {
                    axis: Axis::Y,
                    delta,
                } => (glue::EV_REL, glue::REL_WHEEL_HI_RES as _, *delta),
                Event::MouseMove {
                    axis: Axis::X,
                    delta,
                } => (glue::EV_REL, glue::REL_X as _, *delta),
                Event::MouseMove {
                    axis: Axis::Y,
                    delta,
                } => (glue::EV_REL, glue::REL_Y as _, *delta),
                Event::Key {
                    direction: Direction::Up,
                    kind,
                } => (glue::EV_KEY, kind.to_raw(), 0),
                Event::Key {
                    direction: Direction::Down,
                    kind,
                } => (glue::EV_KEY, kind.to_raw(), 1),
            })
            .chain(std::iter::once((glue::EV_SYN, glue::SYN_REPORT as _, 0)));

        for (r#type, code, value) in events {
            self.write_raw(&input_event {
                type_: r#type as _,
                code: code as _,
                value,
                time: timeval {
                    tv_sec: 0,
                    tv_usec: 0,
                },
            })
            .await?;
        }

        Ok(())
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

impl Drop for EventWriter {
    fn drop(&mut self) {
        unsafe {
            glue::libevdev_uinput_destroy(self.uinput_handle.as_ptr());
            glue::libevdev_free(self.evdev_handle.as_ptr());
        }
    }
}

unsafe impl Send for EventWriter {}

const TYPES: &[(u32, &[RangeInclusive<u32>])] = &[
    (glue::EV_SYN, &[glue::SYN_REPORT..=glue::SYN_REPORT]),
    (glue::EV_REL, &[0..=glue::REL_MAX]),
    (glue::EV_KEY, &[0..=glue::KEY_MAX]),
];

unsafe fn setup_evdev(evdev: *mut libevdev) -> Result<(), Error> {
    glue::libevdev_set_name(evdev, b"rkvm\0".as_ptr() as *const _);
    glue::libevdev_set_id_vendor(evdev, device_id::VENDOR as _);
    glue::libevdev_set_id_product(evdev, device_id::PRODUCT as _);
    glue::libevdev_set_id_version(evdev, device_id::VERSION as _);
    glue::libevdev_set_id_bustype(evdev, glue::BUS_USB as _);

    for (r#type, codes) in TYPES.iter().copied() {
        let ret = glue::libevdev_enable_event_type(evdev, r#type);
        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret));
        }

        for code in codes.iter().cloned().flatten() {
            let ret = glue::libevdev_enable_event_code(evdev, r#type, code, std::ptr::null_mut());
            if ret < 0 {
                return Err(Error::from_raw_os_error(-ret));
            }
        }
    }

    Ok(())
}
