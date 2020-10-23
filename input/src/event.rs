use crate::setup::{self, input_event, timeval};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Event {
    MouseScroll { delta: i32 },
    MouseMove { axis: Axis, delta: i32 },
    Key { direction: Direction, code: u16 },
    Sync,
}

impl Event {
    pub(crate) fn to_raw(&self) -> input_event {
        let (type_, code, value) = match *self {
            Event::MouseScroll { delta } => (setup::EV_REL as _, setup::REL_WHEEL as _, delta),
            Event::MouseMove {
                axis: Axis::X,
                delta,
            } => (setup::EV_REL as _, setup::REL_X as _, delta),
            Event::MouseMove {
                axis: Axis::Y,
                delta,
            } => (setup::EV_REL as _, setup::REL_Y as _, delta),
            Event::Key {
                direction: Direction::Up,
                code,
            } => (setup::EV_KEY as _, code, 0),
            Event::Key {
                direction: Direction::Down,
                code,
            } => (setup::EV_KEY as _, code, 1),
            Event::Sync => (setup::EV_SYN as _, 0, 0),
        };

        input_event {
            type_,
            code,
            value,
            time: timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
        }
    }

    pub(crate) fn from_raw(raw: input_event) -> Option<Self> {
        let event = match (raw.type_ as _, raw.code as _, raw.value) {
            (setup::EV_REL, setup::REL_WHEEL, value) => Event::MouseScroll { delta: value },
            (setup::EV_REL, setup::REL_X, value) => Event::MouseMove {
                axis: Axis::X,
                delta: value,
            },
            (setup::EV_REL, setup::REL_Y, value) => Event::MouseMove {
                axis: Axis::Y,
                delta: value,
            },
            (setup::EV_KEY, code, 0) => Event::Key {
                direction: Direction::Up,
                code: code as _,
            },
            (setup::EV_KEY, code, 1) => Event::Key {
                direction: Direction::Down,
                code: code as _,
            },
            (setup::EV_SYN, _, _) => Event::Sync,
            _ => return None,
        };

        Some(event)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Axis {
    X,
    Y,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum Direction {
    Up,   // The key is released.
    Down, // The key is pressed.
}
