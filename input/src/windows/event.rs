mod button;
mod key;

use crate::event::{Axis, Button, Direction, Event, KeyKind};
use winapi::um::winuser::{self, INPUT_u, INPUT, KEYBDINPUT, MOUSEINPUT};

impl Event {
    pub(crate) fn to_raw(&self) -> Option<INPUT> {
        let mut u: INPUT_u = unsafe { std::mem::zeroed() };
        let type_ = match *self {
            Event::MouseScroll { delta } => {
                let delta = delta * 100;
                unsafe {
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
                }

                winuser::INPUT_MOUSE
            }
            Event::MouseMove {
                axis: Axis::X,
                delta,
            } => {
                unsafe {
                    *u.mi_mut() = MOUSEINPUT {
                        dx: delta,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: winuser::MOUSEEVENTF_MOVE,
                        time: 0,
                        dwExtraInfo: 0,
                    };
                }

                winuser::INPUT_MOUSE
            }
            Event::MouseMove {
                axis: Axis::Y,
                delta,
            } => {
                unsafe {
                    *u.mi_mut() = MOUSEINPUT {
                        dx: 0,
                        dy: delta,
                        mouseData: 0,
                        dwFlags: winuser::MOUSEEVENTF_MOVE,
                        time: 0,
                        dwExtraInfo: 0,
                    };
                }

                winuser::INPUT_MOUSE
            }
            Event::Key { direction, kind } => match kind {
                KeyKind::Button(Button::Left) => {
                    unsafe {
                        *u.mi_mut() = MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: 0,
                            dwFlags: match direction {
                                Direction::Up => winuser::MOUSEEVENTF_LEFTUP,
                                Direction::Down => winuser::MOUSEEVENTF_LEFTDOWN,
                            },
                            time: 0,
                            dwExtraInfo: 0,
                        };
                    }

                    winuser::INPUT_MOUSE
                }
                KeyKind::Button(Button::Right) => {
                    unsafe {
                        *u.mi_mut() = MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: 0,
                            dwFlags: match direction {
                                Direction::Up => winuser::MOUSEEVENTF_RIGHTUP,
                                Direction::Down => winuser::MOUSEEVENTF_RIGHTDOWN,
                            },
                            time: 0,
                            dwExtraInfo: 0,
                        };
                    }

                    winuser::INPUT_MOUSE
                }
                KeyKind::Button(Button::Middle) => {
                    unsafe {
                        *u.mi_mut() = MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: 0,
                            dwFlags: match direction {
                                Direction::Up => winuser::MOUSEEVENTF_MIDDLEUP,
                                Direction::Down => winuser::MOUSEEVENTF_MIDDLEDOWN,
                            },
                            time: 0,
                            dwExtraInfo: 0,
                        };
                    }

                    winuser::INPUT_MOUSE
                }
                KeyKind::Key(key) => {
                    let (code, extended) = key.to_raw()?;
                    unsafe {
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
                    }

                    winuser::INPUT_KEYBOARD
                }
                KeyKind::Button(_) => return None,
            },
        };

        Some(INPUT { type_, u })
    }
}
