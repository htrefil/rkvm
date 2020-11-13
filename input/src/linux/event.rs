mod button;
mod key;

use crate::event::{Axis, Button, Direction, Event, Key, KeyKind};
use crate::linux::glue::{self, input_event, timeval};

impl Event {
    pub(crate) fn to_raw(&self) -> Option<input_event> {
        let (type_, code, value) = match *self {
            Event::MouseScroll { delta } => (glue::EV_REL as _, glue::REL_WHEEL as _, delta),
            Event::MouseMove {
                axis: Axis::X,
                delta,
            } => (glue::EV_REL as _, glue::REL_X as _, delta),
            Event::MouseMove {
                axis: Axis::Y,
                delta,
            } => (glue::EV_REL as _, glue::REL_Y as _, delta),
            Event::Key {
                direction: Direction::Up,
                kind,
            } => (glue::EV_KEY as _, kind.to_raw()?, 0),
            Event::Key {
                direction: Direction::Down,
                kind,
            } => (glue::EV_KEY as _, kind.to_raw()?, 1),
        };

        Some(input_event {
            type_,
            code,
            value,
            time: timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
        })
    }

    pub(crate) fn from_raw(raw: input_event) -> Option<Self> {
        let event = match (raw.type_ as _, raw.code as _, raw.value) {
            (glue::EV_REL, glue::REL_WHEEL, value) => Event::MouseScroll { delta: value },
            (glue::EV_REL, glue::REL_X, value) => Event::MouseMove {
                axis: Axis::X,
                delta: value,
            },
            (glue::EV_REL, glue::REL_Y, value) => Event::MouseMove {
                axis: Axis::Y,
                delta: value,
            },
            (glue::EV_KEY, code, 0) => Event::Key {
                direction: Direction::Up,
                kind: KeyKind::from_raw(code as _)?,
            },
            (glue::EV_KEY, code, 1) => Event::Key {
                direction: Direction::Down,
                kind: KeyKind::from_raw(code as _)?,
            },
            _ => return None,
        };

        Some(event)
    }
}

impl KeyKind {
    pub(crate) fn from_raw(code: u16) -> Option<KeyKind> {
        Key::from_raw(code)
            .map(KeyKind::Key)
            .or_else(|| Button::from_raw(code).map(KeyKind::Button))
    }
}
