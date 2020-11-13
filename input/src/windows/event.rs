mod button;
mod key;

use crate::event::{Axis, Direction, Event};
use winapi::um::winuser::{self, INPUT_u, INPUT, KEYBDINPUT, MOUSEINPUT};

impl Event {
    pub(crate) fn to_raw(&self) -> Option<INPUT> {
        let event = match *self {
            Event::MouseScroll { delta } => INPUT {
                type_: winuser::INPUT_MOUSE,
                u: {
                    let mi = MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: if delta == winuser::WHEEL_DELTA as _ {
                            delta - 1
                        } else {
                            delta
                        } as _,
                        dwFlags: winuser::MOUSEEVENTF_WHEEL,
                        time: 0,
                        dwExtraInfo: 0,
                    };

                    unsafe {
                        let mut u: INPUT_u = std::mem::zeroed();
                        *u.mi_mut() = mi;

                        u
                    }
                },
            },
            Event::MouseMove {
                axis: Axis::X,
                delta,
            } => INPUT {
                type_: winuser::INPUT_MOUSE,
                u: {
                    let mi = MOUSEINPUT {
                        dx: delta,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: winuser::MOUSEEVENTF_MOVE,
                        time: 0,
                        dwExtraInfo: 0,
                    };

                    unsafe {
                        let mut u: INPUT_u = std::mem::zeroed();
                        *u.mi_mut() = mi;

                        u
                    }
                },
            },
            Event::MouseMove {
                axis: Axis::Y,
                delta,
            } => INPUT {
                type_: winuser::INPUT_MOUSE,
                u: {
                    let mi = MOUSEINPUT {
                        dx: 0,
                        dy: delta,
                        mouseData: 0,
                        dwFlags: winuser::MOUSEEVENTF_MOVE,
                        time: 0,
                        dwExtraInfo: 0,
                    };

                    unsafe {
                        let mut u: INPUT_u = std::mem::zeroed();
                        *u.mi_mut() = mi;

                        u
                    }
                },
            },
            Event::Key { direction, kind } => INPUT {
                type_: winuser::INPUT_KEYBOARD,
                u: {
                    let ki = KEYBDINPUT {
                        wVk: kind.to_raw()?,
                        wScan: 0,
                        dwFlags: if direction == Direction::Up {
                            winuser::KEYEVENTF_KEYUP
                        } else {
                            0
                        },
                        time: 0,
                        dwExtraInfo: 0,
                    };

                    unsafe {
                        let mut u: INPUT_u = std::mem::zeroed();
                        *u.ki_mut() = ki;

                        u
                    }
                },
            },
        };

        Some(event)
    }
}
