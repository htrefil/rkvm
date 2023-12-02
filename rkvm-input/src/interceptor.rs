mod caps;

pub use caps::{AbsCaps, KeyCaps, RelCaps, Repeat};

use crate::abs::{AbsAxis, AbsEvent, ToolType};
use crate::convert::Convert;
use crate::evdev::Evdev;
use crate::event::Event;
use crate::glue;
use crate::key::{Key, KeyEvent};
use crate::registry::{Entry, Handle, Registry};
use crate::rel::{RelAxis, RelEvent};
use crate::sync::SyncEvent;
use crate::writer::Writer;

use std::collections::VecDeque;
use std::ffi::CStr;
use std::fs;
use std::io::{Error, ErrorKind};
use std::mem::MaybeUninit;
use std::path::Path;
use thiserror::Error;

pub struct Interceptor {
    evdev: Evdev,
    writer: Writer,
    // The state of `read` is stored here to make it cancel safe.
    events: VecDeque<Event>,
    writing: Option<(u16, u16, i32)>,
    dropped: bool,

    _reader_handle: Handle,
    _writer_handle: Handle,
}

impl Interceptor {
    #[tracing::instrument(fields(path = ?self.writer.path()), skip(self))]
    pub async fn read(&mut self) -> Result<Event, Error> {
        if let Some((r#type, code, value)) = self.writing {
            tracing::trace!("Resuming interrupted write");

            self.writer.write_raw(r#type, code, value).await?;
            self.writing = None;
        }

        while !matches!(self.events.back(), Some(Event::Sync(SyncEvent::All))) {
            let (r#type, code, value) = self.read_raw().await?;
            let event = match r#type as _ {
                glue::EV_REL if !self.dropped => {
                    RelAxis::from_raw(code).map(|axis| Event::Rel(RelEvent { axis, value }))
                }
                glue::EV_ABS if !self.dropped => match code as _ {
                    glue::ABS_MT_TOOL_TYPE => {
                        ToolType::from_raw(value).map(|value| AbsEvent::MtToolType { value })
                    }
                    _ => AbsAxis::from_raw(code).map(|axis| AbsEvent::Axis { axis, value }),
                }
                .map(Event::Abs),
                glue::EV_KEY if !self.dropped && (value == 0 || value == 1) => Key::from_raw(code)
                    .map(|key| {
                        Event::Key(KeyEvent {
                            key,
                            down: value == 1,
                        })
                    }),
                glue::EV_SYN => match code as _ {
                    glue::SYN_REPORT => {
                        if self.dropped {
                            self.dropped = false;
                            continue;
                        }

                        Some(Event::Sync(SyncEvent::All))
                    }
                    glue::SYN_DROPPED => {
                        tracing::warn!(
                            "Dropped {} event{}",
                            self.events.len(),
                            if self.events.len() == 1 { "" } else { "s" }
                        );

                        self.events.clear();
                        self.dropped = true;
                        continue;
                    }
                    glue::SYN_MT_REPORT if !self.dropped => Some(Event::Sync(SyncEvent::Mt)),
                    _ => continue,
                },
                _ => None,
            };

            if let Some(event) = event {
                self.events.push_back(event);
                continue;
            }

            self.writing = Some((r#type, code, value));
            self.writer.write_raw(r#type, code, value).await?;
            self.writing = None;
        }

        Ok(self.events.pop_front().unwrap())
    }

    pub async fn write(&mut self, event: &Event) -> Result<(), Error> {
        self.writer.write(event).await
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

    pub fn repeat(&self) -> Repeat {
        Repeat::new(self)
    }

    async fn read_raw(&mut self) -> Result<(u16, u16, i32), Error> {
        let file = self.evdev.file().unwrap();

        loop {
            let result = file.readable().await?.try_io(|_| {
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

    #[tracing::instrument(skip(registry))]
    pub(crate) async fn open(path: &Path, registry: &Registry) -> Result<Self, OpenError> {
        let evdev = Evdev::open(path).await?;
        let metadata = evdev.file().unwrap().get_ref().metadata()?;

        let reader_handle = registry
            .register(Entry::from_metadata(&metadata))
            .ok_or(OpenError::NotAppliable)?;

        // "Upon binding to a device or resuming from suspend, a driver must report
        // the current switch state. This ensures that the device, kernel, and userspace
        // state is in sync."
        // We have no way of knowing that.
        let sw = unsafe { glue::libevdev_has_event_type(evdev.as_ptr(), glue::EV_SW) };
        if sw == 1 {
            return Err(OpenError::NotAppliable);
        }

        // Some buggy kernels can report nonsense abs info, so check for it and disable the axes.
        for i in 0..glue::ABS_CNT {
            let abs_info = unsafe { glue::libevdev_get_abs_info(evdev.as_ptr(), i).as_ref() };
            let abs_info = match abs_info {
                Some(abs_info) => abs_info,
                None => continue,
            };

            // See Linux source at drivers/input/misc/uinput.c#L408 commit 93f5de5f648d2b1ce3540a4ac71756d4a852dc23.

            let min = abs_info.minimum;
            let max = abs_info.maximum;

            if (min != 0 || max != 0) && max < min {
                tracing::warn!(
                    min = %min,
                    max = max,
                    axis = i,
                    "Detected nonsense min and max values for absolute axis, disabling it",
                );

                let ret =
                    unsafe { glue::libevdev_disable_event_code(evdev.as_ptr(), glue::EV_ABS, i) };

                if ret < 0 {
                    return Err(Error::from_raw_os_error(-ret).into());
                }
            }
        }

        unsafe {
            glue::libevdev_set_id_bustype(evdev.as_ptr(), glue::BUS_VIRTUAL as _);
        }

        let ret =
            unsafe { glue::libevdev_grab(evdev.as_ptr(), glue::libevdev_grab_mode_LIBEVDEV_GRAB) };

        if ret < 0 {
            // We do not use ErrorKind::ResourceBusy because it is a nightly-only API.
            let err = if ret == -libc::EBUSY {
                tracing::info!(
                    "Ignored {:?} because it is busy and can not be grabbed",
                    path
                );
                OpenError::NotAppliable
            } else {
                Error::from_raw_os_error(-ret).into()
            };

            return Err(err);
        }

        let writer = Writer::from_evdev(&evdev).await?;
        let path = writer
            .path()
            .ok_or_else(|| Error::new(ErrorKind::Other, "No syspath for writer"))?;

        let metadata = fs::metadata(path)?;
        let writer_handle = registry
            .register(Entry::from_metadata(&metadata))
            .ok_or_else(|| Error::new(ErrorKind::Other, "Writer already registered"))?;

        Ok(Self {
            evdev,
            writer,
            events: VecDeque::new(),
            dropped: false,
            writing: None,

            _reader_handle: reader_handle,
            _writer_handle: writer_handle,
        })
    }
}

unsafe impl Send for Interceptor {}

#[derive(Error, Debug)]
pub(crate) enum OpenError {
    #[error("Not appliable")]
    NotAppliable,
    #[error(transparent)]
    Io(#[from] Error),
}
