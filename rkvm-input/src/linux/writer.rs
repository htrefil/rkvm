use libc::c_int;

use crate::abs::{AbsAxis, AbsEvent, AbsInfo};
use crate::convert::Convert;
use crate::event::Event;
use crate::linux::glue::{self, input_absinfo};
use crate::key::{Key, KeyEvent};
use crate::rel::{RelAxis, RelEvent};
use crate::linux::uinput::Uinput;
use crate::linux::evdev::Evdev;
use crate::writer::{DeviceWriter, EventWriter};

use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::ffi::{CString, CStr, OsStr};
use std::io::{Error, ErrorKind};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

pub struct WriterLinux {
    dev: HashMap<usize,DeviceWriterLinux>,
}

pub struct DeviceWriterLinux {
    uinput: Uinput,
}

impl WriterLinux {
    pub fn new() -> WriterLinux {
        WriterLinux { dev: HashMap::new()}
    }
}

impl DeviceWriterLinux {
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

    pub async fn from_evdev(evdev: &Evdev) -> Result<Self, Error> {
        Ok(Self {
            uinput: Uinput::from_evdev(evdev).await?,
        })
    }

    pub async fn write(&mut self, event: &Event) -> Result<(), Error> {
        let (r#type, code, value) = match event {
            Event::Rel(RelEvent { axis, value }) => (glue::EV_REL, axis.to_raw(), Some(*value)),
            Event::Abs(event) => match event {
                AbsEvent::Axis { axis, value } => (glue::EV_ABS, axis.to_raw(), Some(*value)),
                AbsEvent::MtToolType { value } => (glue::EV_ABS, Some(glue::ABS_MT_TOOL_TYPE as _), value.to_raw()),
            },
            Event::Key(KeyEvent { down, key }) => (glue::EV_KEY, key.to_raw(), Some(*down as _)),
            Event::Sync(event) => (glue::EV_SYN, event.to_raw(), Some(0)),
        };

        if let (Some(code), Some(value)) = (code, value) {
            self.write_raw(r#type as _, code, value).await?;
        }
        Ok(())
    }

    pub async fn write_raw(
        &mut self,
        r#type: u16,
        code: u16,
        value: i32,
    ) -> Result<(), Error> {
        loop {
            let result = self.uinput.file().writable().await?.try_io(|_| {
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

#[async_trait]
impl DeviceWriter for WriterLinux {
    async fn create_device(&mut self, id: usize, name: &CString, vendor: u16, product: u16, version: u16, rel: HashSet<RelAxis>, abs: HashMap<AbsAxis, AbsInfo>, keys: HashSet<Key>, delay: Option<i32>, period: Option<i32>) -> Result<(), Error> {
        let entry = self.dev.entry(id);
        if let Entry::Occupied(_) = entry {
            return Err(Error::new(ErrorKind::InvalidData, "Server created the same device twice"));
        }

        let evdev = Evdev::new()?;
        unsafe {
            glue::libevdev_set_id_bustype(evdev.as_ptr(), glue::BUS_VIRTUAL as _);
            glue::libevdev_set_name(evdev.as_ptr(), name.as_ptr());
            glue::libevdev_set_id_vendor(evdev.as_ptr(), vendor as _);
            glue::libevdev_set_id_product(evdev.as_ptr(), product as _);
            glue::libevdev_set_id_version(evdev.as_ptr(), version as _);
        }
        init_rel(&evdev, rel)?;
        init_abs(&evdev, abs)?;
        init_keys(&evdev, keys)?;
        if let Some(value) = delay {
            init_delay(&evdev, &value)?;
        }
        if let Some(value) = period {
            init_period(&evdev, &value)?;
        }

        entry.or_insert(DeviceWriterLinux::from_evdev(&evdev).await?);
        Ok(())
    }
    async fn destroy_device(&mut self, id: usize) -> Result<(), Error> {
        if self.dev.remove(&id).is_none() {
            return Err(Error::new(ErrorKind::InvalidData, "Server destroyed a nonexistent device"));
        }

        tracing::info!(id = %id, "Destroyed device");
        Ok(())
    }
}

#[async_trait]
impl EventWriter for WriterLinux {
    async fn event(&mut self, id: usize, event: Event) -> Result<(), Error> {
        let dev = self.dev.get_mut(&id).ok_or_else(|| { Error::new(ErrorKind::InvalidData,"Server sent an event to a nonexistent device",)})?;
        dev.write(&event).await
    }
}

fn init_rel(evdev: &Evdev, rel: HashSet<RelAxis>) -> Result<(),Error> {
    for axis in rel {
        let axis = match axis.to_raw() {
            Some(axis) => axis,
            None => continue,
        };

        let ret = unsafe {
            glue::libevdev_enable_event_code(evdev.as_ptr(), glue::EV_REL, axis as _, ptr::null(),)
        };

        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret));
        }
    }
    Ok(())
}
fn init_abs(evdev: &Evdev, abs: HashMap<AbsAxis, AbsInfo>) -> Result<(),Error> {
    let ret = unsafe {
        glue::libevdev_enable_event_code(evdev.as_ptr(), glue::EV_SYN, glue::SYN_MT_REPORT, ptr::null(),)
    };

    if ret < 0 {
        return Err(Error::from_raw_os_error(-ret));
    }

    for (axis, info) in abs {
        let code = match axis.to_raw() {
            Some(code) => code,
            None => continue,
        };

        let info = input_absinfo {
            value: info.min,
            minimum: info.min,
            maximum: info.max,
            fuzz: info.fuzz,
            flat: info.flat,
            resolution: info.resolution,
        };

        let ret = unsafe {
            glue::libevdev_enable_event_code(evdev.as_ptr(), glue::EV_ABS, code as _, &info as *const _ as *const _,)
        };

        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret));
        }
    }
    Ok(())
}
fn init_keys(evdev: &Evdev, keys: HashSet<Key>) -> Result<(), Error> {
    for key in keys {
        let key = match key.to_raw() {
            Some(key) => key,
            None => continue,
        };

        let ret = unsafe {
            glue::libevdev_enable_event_code(evdev.as_ptr(), glue::EV_KEY, key as _, ptr::null(),)
        };

        if ret < 0 {
            return Err(Error::from_raw_os_error(-ret));
        }
    }

    Ok(())
}
fn init_delay(evdev: &Evdev, value: &c_int) -> Result<(),Error> {
    let ret = unsafe {
        glue::libevdev_enable_event_code(evdev.as_ptr(), glue::EV_REP, glue::REP_DELAY, value as *const _ as *const _,)
    };

    if ret < 0 {
        return Err(Error::from_raw_os_error(-ret));
    }
    Ok(())
}
fn init_period(evdev: &Evdev, value: &c_int) -> Result<(), Error> {
    let ret = unsafe {
        glue::libevdev_enable_event_code(evdev.as_ptr(), glue::EV_REP, glue::REP_PERIOD, value as *const _ as *const _,)
    };

    if ret < 0 {
        return Err(Error::from_raw_os_error(-ret));
    }
    Ok(())
}

