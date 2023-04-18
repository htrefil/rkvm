use crate::device_id;
use crate::event::Event;
use crate::glue::{self, libevdev, libevdev_uinput};
use crate::{Axis, Direction};
use std::io::{Error, ErrorKind};
use std::mem::MaybeUninit;
use std::ops::RangeInclusive;

pub struct EventWriter {
    evdev: *mut libevdev,
    uinput: *mut libevdev_uinput,
}

impl EventWriter {
    pub async fn new() -> Result<Self, Error> {
        tokio::task::spawn_blocking(Self::new_sync).await?
    }

    fn new_sync() -> Result<Self, Error> {
        let evdev = unsafe { glue::libevdev_new() };
        if evdev.is_null() {
            return Err(Error::new(ErrorKind::Other, "Failed to create device"));
        }

        if let Err(err) = unsafe { setup_evdev(evdev) } {
            unsafe {
                glue::libevdev_free(evdev);
            }

            return Err(err);
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

            return Err(Error::from_raw_os_error(-ret));
        }

        Ok(Self {
            evdev,
            uinput: unsafe { uinput.assume_init() },
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
            // TODO: Not exactly async.

            // As far as tokio is concerned, the FD never becomes ready for writing, so just write it normally.
            // If an error happens, it will be propagated to caller and the FD is opened in nonblocking mode anyway,
            // so it shouldn't be an issue.
            let ret = unsafe {
                glue::libevdev_uinput_write_event(self.uinput as *const _, r#type, code, value)
            };

            if ret < 0 {
                return Err(Error::from_raw_os_error(-ret));
            }
        }

        Ok(())
    }
}

impl Drop for EventWriter {
    fn drop(&mut self) {
        unsafe {
            glue::libevdev_uinput_destroy(self.uinput);
            glue::libevdev_free(self.evdev);
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
