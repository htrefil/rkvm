use crate::event::Event;
use crate::glue::{self, input_event, libevdev, libevdev_uinput};
use mio::unix::EventedFd;
use std::fs::OpenOptions;
use std::future::Future;
use std::io::{Error, ErrorKind};
use std::mem::MaybeUninit;
use std::ops::RangeInclusive;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::Registration;

pub struct EventWriter {
    evdev: *mut libevdev,
    uinput: *mut libevdev_uinput,
    registration: Registration,
}

impl EventWriter {
    pub async fn new() -> Result<Self, Error> {
        tokio::task::spawn_blocking(|| Self::new_sync()).await?
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
            unsafe { glue::libevdev_free(evdev) };
            return Err(Error::from_raw_os_error(-ret));
        }

        let uinput = unsafe { uinput.assume_init() };
        let fd = unsafe { glue::libevdev_uinput_get_fd(uinput) };
        let registration = match Registration::new(&EventedFd(&fd)) {
            Ok(registration) => registration,
            Err(err) => {
                unsafe {
                    glue::libevdev_uinput_destroy(uinput);
                    glue::libevdev_free(evdev);
                };

                return Err(err);
            }
        };

        Ok(Self {
            evdev,
            uinput,
            registration,
        })
    }

    pub async fn write(&mut self, event: Event) -> Result<(), Error> {
        self.write_raw(event.to_raw()).await
    }

    pub(crate) async fn write_raw(&mut self, event: input_event) -> Result<(), Error> {
        WriteRaw {
            writer: self,
            event,
            polling: false,
        }
        .await
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

struct WriteRaw<'a> {
    writer: &'a mut EventWriter,
    event: input_event,
    polling: bool,
}

impl Future for WriteRaw<'_> {
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<Self::Output> {
        if self.polling {
            if let Poll::Pending = self.writer.registration.poll_write_ready(context)? {
                return Poll::Pending;
            }
        }

        let ret = unsafe {
            glue::libevdev_uinput_write_event(
                self.writer.uinput as *const _,
                self.event.type_ as _,
                self.event.code as _,
                self.event.value,
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

        Poll::Ready(Ok(()))
    }
}

const TYPES: &[(u32, &[RangeInclusive<u32>])] = &[
    (glue::EV_SYN, &[glue::SYN_REPORT..=glue::SYN_REPORT]),
    (glue::EV_REL, &[0..=glue::REL_MAX]),
    (glue::EV_KEY, &[0..=glue::KEY_MAX]),
];

unsafe fn setup_evdev(evdev: *mut libevdev) -> Result<(), Error> {
    glue::libevdev_set_name(evdev, b"rkvm\0".as_ptr() as *const _);
    glue::libevdev_set_id_product(evdev, 1);
    glue::libevdev_set_id_version(evdev, 1);
    glue::libevdev_set_id_vendor(evdev, i32::from_be_bytes(*b"rkvm"));
    glue::libevdev_set_id_bustype(evdev, glue::BUS_USB as _);

    for (r#type, codes) in TYPES.iter().copied() {
        let ret = glue::libevdev_enable_event_type(evdev, r#type);
        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret));
        }

        for code in codes.iter().cloned().flat_map(|code| code) {
            let ret = glue::libevdev_enable_event_code(evdev, r#type, code, std::ptr::null_mut());
            if ret < 0 {
                return Err(Error::from_raw_os_error(-ret));
            }
        }
    }

    Ok(())
}
