mod key;

use crate::event::{Axis, Button, Direction, Event, KeyKind};
use crate::windows::oot::Oot;
use winapi::um::winuser::{self, INPUT_u, INPUT, KEYBDINPUT, MOUSEINPUT};

const DELTA_FACTOR: i32 = 100;

impl Event {
    pub(crate) fn to_raw(&self) -> Option<Oot<INPUT>> {
        let inputs = match *self {
            Event::MouseScroll { delta } => {
                let delta = delta * DELTA_FACTOR;
                unsafe {
                    let mut u: INPUT_u = std::mem::zeroed();
                    *u.mi_mut() = MOUSEINPUT {
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

                    Oot::V1([(winuser::INPUT_MOUSE, u)])
                }
            }
            Event::MouseMove {
                axis: Axis::X,
                delta,
            } => unsafe {
                let mut u: INPUT_u = std::mem::zeroed();
                *u.mi_mut() = MOUSEINPUT {
                    dx: delta,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: winuser::MOUSEEVENTF_MOVE,
                    time: 0,
                    dwExtraInfo: 0,
                };

                Oot::V1([(winuser::INPUT_MOUSE, u)])
            },
            Event::MouseMove {
                axis: Axis::Y,
                delta,
            } => unsafe {
                let mut u: INPUT_u = std::mem::zeroed();
                *u.mi_mut() = MOUSEINPUT {
                    dx: 0,
                    dy: delta,
                    mouseData: 0,
                    dwFlags: winuser::MOUSEEVENTF_MOVE,
                    time: 0,
                    dwExtraInfo: 0,
                };

                Oot::V1([(winuser::INPUT_MOUSE, u)])
            },
            Event::Key { direction, kind } => match kind {
                KeyKind::Key(key) => key.to_raw()?.map(|(code, extended)| unsafe {
                    let mut u: INPUT_u = std::mem::zeroed();
                    *u.ki_mut() = KEYBDINPUT {
                        wVk: 0,
                        wScan: code,
                        dwFlags: winuser::KEYEVENTF_SCANCODE
                            | if extended {
                                winuser::KEYEVENTF_EXTENDEDKEY
                            } else {
                                0
                            }
                            | match direction {
                                Direction::Up => winuser::KEYEVENTF_KEYUP,
                                Direction::Down => 0,
                            },
                        time: 0,
                        dwExtraInfo: 0,
                    };

                    (winuser::INPUT_KEYBOARD, u)
                }),
                KeyKind::Button(button) => {
                    let flags = match (button, direction) {
                        (Button::Right, Direction::Up) => winuser::MOUSEEVENTF_RIGHTUP,
                        (Button::Right, Direction::Down) => winuser::MOUSEEVENTF_RIGHTDOWN,
                        (Button::Left, Direction::Up) => winuser::MOUSEEVENTF_LEFTUP,
                        (Button::Left, Direction::Down) => winuser::MOUSEEVENTF_LEFTDOWN,
                        (Button::Middle, Direction::Up) => winuser::MOUSEEVENTF_MIDDLEUP,
                        (Button::Middle, Direction::Down) => winuser::MOUSEEVENTF_MIDDLEDOWN,
                        _ => return None,
                    };

                    Oot::V1([(winuser::INPUT_MOUSE, unsafe {
                        let mut u: INPUT_u = std::mem::zeroed();
                        *u.mi_mut() = MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: 0,
                            dwFlags: flags,
                            time: 0,
                            dwExtraInfo: 0,
                        };

                        u
                    })])
                }
            },
        };

        Some(inputs.map(|(type_, u)| INPUT { type_, u }))
    }
}
