use crate::abs::{AbsAxis, AbsInfo};
use crate::convert::Convert;
use crate::linux::glue;
use crate::linux::interceptor::InterceptorLinux;
use crate::key::Key;
use crate::rel::RelAxis;

pub struct RelCaps<'a> {
    current: u16,
    interceptor: &'a InterceptorLinux,
}

impl<'a> RelCaps<'a> {
    pub(super) fn new(interceptor: &'a InterceptorLinux) -> Self {
        let has =
            unsafe { glue::libevdev_has_event_type(interceptor.evdev.as_ptr(), glue::EV_REL) == 1 };

        Self {
            current: if has { 0 } else { glue::REL_MAX as _ },
            interceptor,
        }
    }
}

impl Iterator for RelCaps<'_> {
    type Item = RelAxis;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < glue::REL_MAX as _ {
            let has = unsafe {
                glue::libevdev_has_event_code(
                    self.interceptor.evdev.as_ptr(),
                    glue::EV_REL,
                    self.current as _,
                ) == 1
            };

            self.current += 1;

            if !has {
                continue;
            }

            if let Some(axis) = RelAxis::from_raw(self.current - 1) {
                return Some(axis);
            }
        }

        None
    }
}

pub struct AbsCaps<'a> {
    current: u16,
    interceptor: &'a InterceptorLinux,
}

impl<'a> AbsCaps<'a> {
    pub(super) fn new(interceptor: &'a InterceptorLinux) -> Self {
        let has =
            unsafe { glue::libevdev_has_event_type(interceptor.evdev.as_ptr(), glue::EV_ABS) == 1 };

        Self {
            current: if has { 0 } else { glue::ABS_MAX as _ },
            interceptor,
        }
    }
}

impl Iterator for AbsCaps<'_> {
    type Item = (AbsAxis, AbsInfo);

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < glue::ABS_MAX as _ {
            let has = unsafe {
                glue::libevdev_has_event_code(
                    self.interceptor.evdev.as_ptr(),
                    glue::EV_ABS,
                    self.current as _,
                ) == 1
            };

            self.current += 1;

            if !has {
                continue;
            }

            if let Some(axis) = AbsAxis::from_raw(self.current - 1) {
                let info = unsafe {
                    glue::libevdev_get_abs_info(
                        self.interceptor.evdev.as_ptr(),
                        (self.current - 1) as _,
                    )
                };

                let info = unsafe { info.as_ref().unwrap() };
                let info = AbsInfo {
                    min: info.minimum,
                    max: info.maximum,
                    fuzz: info.fuzz,
                    flat: info.flat,
                    resolution: info.resolution,
                };

                return Some((axis, info));
            }
        }

        None
    }
}

pub struct KeyCaps<'a> {
    current: u16,
    interceptor: &'a InterceptorLinux,
}

impl<'a> KeyCaps<'a> {
    pub(super) fn new(interceptor: &'a InterceptorLinux) -> Self {
        let has =
            unsafe { glue::libevdev_has_event_type(interceptor.evdev.as_ptr(), glue::EV_KEY) == 1 };

        Self {
            current: if has { 0 } else { glue::KEY_MAX as _ },
            interceptor,
        }
    }
}

impl Iterator for KeyCaps<'_> {
    type Item = Key;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < glue::KEY_MAX as _ {
            let has = unsafe {
                glue::libevdev_has_event_code(
                    self.interceptor.evdev.as_ptr(),
                    glue::EV_KEY,
                    self.current as _,
                ) == 1
            };

            self.current += 1;

            if !has {
                continue;
            }

            if let Some(stroke) = Key::from_raw(self.current - 1) {
                return Some(stroke);
            }
        }

        None
    }
}
