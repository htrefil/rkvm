use crate::writer::{DeviceWriter, EventWriter};
use crate::abs::{AbsAxis, AbsInfo, AbsEvent};
use crate::event::Event;
use crate::key::{Key, KeyEvent,Keyboard, Button};
use crate::rel::{RelAxis, RelEvent};

use crate::windows::injector::send_input;

use async_trait::async_trait;
use std::ffi::CString;
use std::io::Error;
use std::collections::{HashMap, HashSet};

use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub struct WriterWindowsSimple {
}

impl WriterWindowsSimple {
    pub fn new() -> Self {
        WriterWindowsSimple {}
    }
}

#[async_trait]
impl DeviceWriter for WriterWindowsSimple {
    async fn create_device(&mut self, _id: usize, _name: &CString, _vendor: u16, _product: u16, _version: u16, _rel: HashSet<RelAxis>, _abs: HashMap<AbsAxis, AbsInfo>, _keys: HashSet<Key>, _delay: Option<i32>, _period: Option<i32>) -> Result<(), Error> {
        Ok(())
    }
    async fn destroy_device(&mut self, _id: usize) -> Result<(), Error> {
        Ok(())
    }
}

#[async_trait]
impl EventWriter for WriterWindowsSimple {
    async fn event(&mut self, _id: usize, event: Event) -> Result<(), Error> {
        match event {
            Event::Key(KeyEvent { key, down }) => {
                match key {
                    Key::Key(k) => send_key(k, down),
                    Key::Button(b) => button(b, down),
                }
            }
            Event::Rel(RelEvent { axis, value }) => {
                match axis {
                    RelAxis::X => mouse_move(false, value, 0),
                    RelAxis::Y => mouse_move(false, 0, value),
                    RelAxis::Wheel => mouse_data(MOUSEEVENTF_WHEEL, value*120),
                    RelAxis::HWheel => mouse_data(MOUSEEVENTF_HWHEEL, value*120),
                    RelAxis::WheelHiRes => mouse_data(MOUSEEVENTF_WHEEL, value),
                    RelAxis::HWheelHiRes => mouse_data(MOUSEEVENTF_HWHEEL, value),
                    _ => tracing::warn!("Axe not handled: {:?}", axis),
                }
            }
            Event::Abs(event) => {
                match event {
                    AbsEvent::Axis { axis, value } => {
                            match axis {
                                AbsAxis::X => mouse_move(true, value, 0),
                                AbsAxis::Y => mouse_move(true, 0, value),
                                _ => tracing::warn!("Abs Axis not handled: {:?}", axis)
                            }
                    }
                    _ => tracing::warn!("Abs event not handled: {:?}", event),
                }
            }
            _ => {}
        }

        Ok(())
    }
}

pub fn send_key(key: Keyboard, down: bool) {
    if let Some((scan, extended)) = map_key_to_scancode(key) {
        let mut flags = KEYEVENTF_SCANCODE;
        if !down { flags |= KEYEVENTF_KEYUP; }
        if extended { flags |= KEYEVENTF_EXTENDEDKEY; }
        
        send_input(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY::default(),
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }
}

fn button(button: Button, down: bool) {
    if let Some((flags, mousedata)) = map_button(button, down) {
        send_input(INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: mousedata,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        })
    }
}

fn mouse_move(abs: bool, dx: i32, dy: i32) {
    let mut flags = MOUSEEVENTF_MOVE;
    if abs {
        flags |= MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
    }
    send_input(INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    })
}

fn mouse_data(flags: MOUSE_EVENT_FLAGS, delta: i32) {
     send_input(INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: delta as u32,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    })
}

fn map_button(button: Button, down: bool) -> Option<(MOUSE_EVENT_FLAGS,u32)>  {
   match button {
        Button::Left => Some((
            if down { MOUSEEVENTF_LEFTDOWN } else { MOUSEEVENTF_LEFTUP },
            0 as u32,
        )),
        Button::Right => Some((
            if down { MOUSEEVENTF_RIGHTDOWN } else { MOUSEEVENTF_RIGHTUP },
            0 as u32,
        )),
        Button::Middle => Some((
            if down { MOUSEEVENTF_MIDDLEDOWN } else { MOUSEEVENTF_MIDDLEUP },
            0 as u32,
        )),
        Button::Side => Some((
            if down { MOUSEEVENTF_XDOWN } else { MOUSEEVENTF_XUP },
            1 as u32,
        )),
        Button::Extra => Some((
            if down { MOUSEEVENTF_XDOWN } else { MOUSEEVENTF_XUP },
            2 as u32,
        )),
        _ => {
            tracing::warn!("Unsupported mouse button: {:?}", button);
            None
        }
    }
}

fn map_key_to_scancode(key: Keyboard) -> Option<(u16, bool)> {
    match key {
        // Letters
        Keyboard::A => Some((0x1E, false)),
        Keyboard::B => Some((0x30, false)),
        Keyboard::C => Some((0x2E, false)),
        Keyboard::D => Some((0x20, false)),
        Keyboard::E => Some((0x12, false)),
        Keyboard::F => Some((0x21, false)),
        Keyboard::G => Some((0x22, false)),
        Keyboard::H => Some((0x23, false)),
        Keyboard::I => Some((0x17, false)),
        Keyboard::J => Some((0x24, false)),
        Keyboard::K => Some((0x25, false)),
        Keyboard::L => Some((0x26, false)),
        Keyboard::M => Some((0x32, false)),
        Keyboard::N => Some((0x31, false)),
        Keyboard::O => Some((0x18, false)),
        Keyboard::P => Some((0x19, false)),
        Keyboard::Q => Some((0x10, false)),
        Keyboard::R => Some((0x13, false)),
        Keyboard::S => Some((0x1F, false)),
        Keyboard::T => Some((0x14, false)),
        Keyboard::U => Some((0x16, false)),
        Keyboard::V => Some((0x2F, false)),
        Keyboard::W => Some((0x11, false)),
        Keyboard::X => Some((0x2D, false)),
        Keyboard::Y => Some((0x15, false)),
        Keyboard::Z => Some((0x2C, false)),

        // Numbers
        Keyboard::N1 => Some((0x02, false)),
        Keyboard::N2 => Some((0x03, false)),
        Keyboard::N3 => Some((0x04, false)),
        Keyboard::N4 => Some((0x05, false)),
        Keyboard::N5 => Some((0x06, false)),
        Keyboard::N6 => Some((0x07, false)),
        Keyboard::N7 => Some((0x08, false)),
        Keyboard::N8 => Some((0x09, false)),
        Keyboard::N9 => Some((0x0A, false)),
        Keyboard::N0 => Some((0x0B, false)),

        // Arrows
        Keyboard::Up => Some((0x48, true)),
        Keyboard::Down => Some((0x50, true)),
        Keyboard::Left => Some((0x4B, true)),
        Keyboard::Right => Some((0x4D, true)),

        // Functions
        Keyboard::F1 => Some((0x3B, false)),
        Keyboard::F2 => Some((0x3C, false)),
        Keyboard::F3 => Some((0x3D, false)),
        Keyboard::F4 => Some((0x3E, false)),
        Keyboard::F5 => Some((0x3F, false)),
        Keyboard::F6 => Some((0x40, false)),
        Keyboard::F7 => Some((0x41, false)),
        Keyboard::F8 => Some((0x42, false)),
        Keyboard::F9 => Some((0x43, false)),
        Keyboard::F10 => Some((0x44, false)),
        Keyboard::F11 => Some((0x57, false)),
        Keyboard::F12 => Some((0x58, false)),

      

        // Special Keyboards
        Keyboard::Enter => Some((0x1C, false)),
        Keyboard::Minus => Some((0x0C, false)),
        Keyboard::Equal => Some((0x0D, false)),
        Keyboard::LeftBrace => Some((0x1A, false)),
        Keyboard::RightBrace => Some((0x1B, false)),
        Keyboard::Apostrophe => Some((0x28, false)),
        Keyboard::Slash => Some((0x35, false)),
        Keyboard::Dot => Some((0x34, false)),
        Keyboard::Semicolon => Some((0x27, false)),
        Keyboard::Grave => Some((0x29, false)),
        Keyboard::Comma => Some((0x33, false)),
        Keyboard::Backslash => Some((0x2B, false)),
        Keyboard::Esc => Some((0x01, false)),
        Keyboard::Backspace => Some((0x0E, false)),
        Keyboard::Tab => Some((0x0F, false)),
        Keyboard::Space => Some((0x39, false)),
        Keyboard::CapsLock => Some((0x3A, false)),
        Keyboard::LeftShift => Some((0x2A, false)),
        Keyboard::RightShift => Some((0x36, false)),
        Keyboard::LeftCtrl => Some((0x1D, false)),
        Keyboard::RightCtrl => Some((0x1D, true)),
        Keyboard::LeftAlt => Some((0x38, false)),
        Keyboard::RightAlt => Some((0x38, true)),
        Keyboard::LeftMeta => Some((0x5B, true)), // Windows Keyboard
        Keyboard::RightMeta => Some((0x5C, true)),
        Keyboard::SysRq => Some((0x54, false)),
        Keyboard::ScrollLock => Some((0x46, false)),
        Keyboard::Compose => Some((0x5D, true)),
        Keyboard::Pause => Some((0x45, true)),
        Keyboard::N102nd => Some((0x56, false)),

        Keyboard::Insert => Some((0x52, true)),
        Keyboard::Delete => Some((0x53, true)),
        Keyboard::Home => Some((0x47, true)),
        Keyboard::End => Some((0x4F, true)),
        Keyboard::PageUp => Some((0x49, true)),
        Keyboard::PageDown => Some((0x51, true)),

        // keypad
        Keyboard::NumLock => Some((0x45, false)),
        Keyboard::Kp0 => Some((0x52, false)),
        Keyboard::Kp1 => Some((0x4F, false)),
        Keyboard::Kp2 => Some((0x50, false)),
        Keyboard::Kp3 => Some((0x51, false)),
        Keyboard::Kp4 => Some((0x4B, false)),
        Keyboard::Kp5 => Some((0x4C, false)),
        Keyboard::Kp6 => Some((0x4D, false)),
        Keyboard::Kp7 => Some((0x47, false)),
        Keyboard::Kp8 => Some((0x48, false)),
        Keyboard::Kp9 => Some((0x49, false)),
        Keyboard::KpDot => Some((0x53, false)),
        Keyboard::KpAsterisk => Some((0x37, false)),
        Keyboard::KpEnter => Some((0x1C, true)),
        Keyboard::KpMinus => Some((0x4A, false)),
        Keyboard::KpPlus => Some((0x4E, false)),
        Keyboard::KpSlash => Some((0x35, true)),

        _ => {
            tracing::warn!("Unsupported keyboard key : {:?}", key);
            None
        }
    }
}
