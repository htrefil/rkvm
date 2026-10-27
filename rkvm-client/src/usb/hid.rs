use rkvm_input::key::{Button, Keyboard};
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::Duration;
use tokio::io::unix::AsyncFd;
use tokio::io::Interest;
use tokio::time;

// How long to wait for the host to pick up a report before dropping it.
// This happens if the host is suspended, we don't want to stall the whole client.
const REPORT_TIMEOUT: Duration = Duration::from_millis(100);

// Boot protocol compatible keyboard, so that it works in BIOS/UEFI too.
// Report: modifiers, reserved, 6 key usages.
pub const KEYBOARD_REPORT_LENGTH: usize = 8;
pub const KEYBOARD_DESCRIPTOR: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x06, // Usage (Keyboard)
    0xa1, 0x01, // Collection (Application)
    0x05, 0x07, //   Usage Page (Keyboard/Keypad)
    0x19, 0xe0, //   Usage Minimum (Left Control)
    0x29, 0xe7, //   Usage Maximum (Right GUI)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x01, //   Logical Maximum (1)
    0x75, 0x01, //   Report Size (1)
    0x95, 0x08, //   Report Count (8)
    0x81, 0x02, //   Input (Data, Variable, Absolute)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x01, //   Report Count (1)
    0x81, 0x01, //   Input (Constant)
    0x19, 0x00, //   Usage Minimum (0)
    0x2a, 0xff, 0x00, //   Usage Maximum (255)
    0x15, 0x00, //   Logical Minimum (0)
    0x26, 0xff, 0x00, //   Logical Maximum (255)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x06, //   Report Count (6)
    0x81, 0x00, //   Input (Data, Array, Absolute)
    0xc0, // End Collection
];

pub const KEYBOARD_ROLLOVER: usize = 6;
pub const KEYBOARD_ERROR_ROLLOVER: u8 = 0x01;

// Report: buttons, X (16 bit), Y (16 bit), wheel, horizontal wheel.
pub const MOUSE_REPORT_LENGTH: usize = 7;
pub const MOUSE_DESCRIPTOR: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x02, // Usage (Mouse)
    0xa1, 0x01, // Collection (Application)
    0x09, 0x01, //   Usage (Pointer)
    0xa1, 0x00, //   Collection (Physical)
    0x05, 0x09, //     Usage Page (Button)
    0x19, 0x01, //     Usage Minimum (1)
    0x29, 0x05, //     Usage Maximum (5)
    0x15, 0x00, //     Logical Minimum (0)
    0x25, 0x01, //     Logical Maximum (1)
    0x75, 0x01, //     Report Size (1)
    0x95, 0x05, //     Report Count (5)
    0x81, 0x02, //     Input (Data, Variable, Absolute)
    0x75, 0x03, //     Report Size (3)
    0x95, 0x01, //     Report Count (1)
    0x81, 0x01, //     Input (Constant)
    0x05, 0x01, //     Usage Page (Generic Desktop)
    0x09, 0x30, //     Usage (X)
    0x09, 0x31, //     Usage (Y)
    0x16, 0x01, 0x80, //     Logical Minimum (-32767)
    0x26, 0xff, 0x7f, //     Logical Maximum (32767)
    0x75, 0x10, //     Report Size (16)
    0x95, 0x02, //     Report Count (2)
    0x81, 0x06, //     Input (Data, Variable, Relative)
    0x09, 0x38, //     Usage (Wheel)
    0x15, 0x81, //     Logical Minimum (-127)
    0x25, 0x7f, //     Logical Maximum (127)
    0x75, 0x08, //     Report Size (8)
    0x95, 0x01, //     Report Count (1)
    0x81, 0x06, //     Input (Data, Variable, Relative)
    0x05, 0x0c, //     Usage Page (Consumer)
    0x0a, 0x38, 0x02, //     Usage (AC Pan)
    0x95, 0x01, //     Report Count (1)
    0x81, 0x06, //     Input (Data, Variable, Relative)
    0xc0, //   End Collection
    0xc0, // End Collection
];

pub const MOUSE_MOTION_MAX: i32 = 32767;
pub const MOUSE_WHEEL_MAX: i32 = 127;

// A /dev/hidgN device.
pub struct Hid {
    file: AsyncFd<File>,
}

impl Hid {
    pub async fn open(path: &Path) -> Result<Self, Error> {
        // The device node might take a moment to show up after the gadget is bound.
        let mut attempts = 10;

        let file = loop {
            let result = OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(path);

            match result {
                Ok(file) => break file,
                Err(err) if err.kind() == ErrorKind::NotFound && attempts > 0 => {
                    attempts -= 1;
                    time::sleep(Duration::from_millis(100)).await;
                }
                Err(err) => return Err(err),
            }
        };

        Ok(Self {
            file: AsyncFd::with_interest(file, Interest::WRITABLE)?,
        })
    }

    // Reports that the host doesn't pick up are dropped, the next one always carries the full state anyway.
    pub async fn write(&self, report: &[u8]) -> Result<(), Error> {
        let result = time::timeout(REPORT_TIMEOUT, async {
            loop {
                let mut guard = self.file.writable().await?;
                match guard.try_io(|file| file.get_ref().write(report)) {
                    Ok(Ok(written)) if written != report.len() => {
                        return Err(Error::new(ErrorKind::WriteZero, "Partial report write"))
                    }
                    Ok(result) => return result.map(|_| ()),
                    Err(_) => continue, // This means it would block.
                }
            }
        })
        .await;

        match result {
            Ok(Ok(())) => Ok(()),
            // The gadget is not connected to a host (or the host didn't configure it yet).
            Ok(Err(err)) if err.raw_os_error() == Some(libc::ESHUTDOWN) => {
                tracing::debug!("USB host not connected, dropping report");
                Ok(())
            }
            Ok(Err(err)) => Err(err),
            Err(_) => {
                tracing::debug!("USB host not accepting reports, dropping report");
                Ok(())
            }
        }
    }
}

// Returns the modifier bit of a key, if it is a modifier.
pub fn modifier(key: Keyboard) -> Option<u8> {
    let bit = match key {
        Keyboard::LeftCtrl => 0,
        Keyboard::LeftShift => 1,
        Keyboard::LeftAlt => 2,
        Keyboard::LeftMeta => 3,
        Keyboard::RightCtrl => 4,
        Keyboard::RightShift => 5,
        Keyboard::RightAlt => 6,
        Keyboard::RightMeta => 7,
        _ => return None,
    };

    Some(1 << bit)
}

// Returns the Keyboard/Keypad page usage of a key.
// This mirrors the mapping done by the Linux HID driver, so that the keys round trip when the host runs Linux.
pub fn usage(key: Keyboard) -> Option<u8> {
    let usage = match key {
        Keyboard::A => 0x04,
        Keyboard::B => 0x05,
        Keyboard::C => 0x06,
        Keyboard::D => 0x07,
        Keyboard::E => 0x08,
        Keyboard::F => 0x09,
        Keyboard::G => 0x0a,
        Keyboard::H => 0x0b,
        Keyboard::I => 0x0c,
        Keyboard::J => 0x0d,
        Keyboard::K => 0x0e,
        Keyboard::L => 0x0f,
        Keyboard::M => 0x10,
        Keyboard::N => 0x11,
        Keyboard::O => 0x12,
        Keyboard::P => 0x13,
        Keyboard::Q => 0x14,
        Keyboard::R => 0x15,
        Keyboard::S => 0x16,
        Keyboard::T => 0x17,
        Keyboard::U => 0x18,
        Keyboard::V => 0x19,
        Keyboard::W => 0x1a,
        Keyboard::X => 0x1b,
        Keyboard::Y => 0x1c,
        Keyboard::Z => 0x1d,
        Keyboard::N1 => 0x1e,
        Keyboard::N2 => 0x1f,
        Keyboard::N3 => 0x20,
        Keyboard::N4 => 0x21,
        Keyboard::N5 => 0x22,
        Keyboard::N6 => 0x23,
        Keyboard::N7 => 0x24,
        Keyboard::N8 => 0x25,
        Keyboard::N9 => 0x26,
        Keyboard::N0 => 0x27,
        Keyboard::Enter => 0x28,
        Keyboard::Esc => 0x29,
        Keyboard::Backspace => 0x2a,
        Keyboard::Tab => 0x2b,
        Keyboard::Space => 0x2c,
        Keyboard::Minus => 0x2d,
        Keyboard::Equal => 0x2e,
        Keyboard::LeftBrace => 0x2f,
        Keyboard::RightBrace => 0x30,
        Keyboard::Backslash => 0x31,
        Keyboard::Semicolon => 0x33,
        Keyboard::Apostrophe => 0x34,
        Keyboard::Grave => 0x35,
        Keyboard::Comma => 0x36,
        Keyboard::Dot => 0x37,
        Keyboard::Slash => 0x38,
        Keyboard::CapsLock => 0x39,
        Keyboard::F1 => 0x3a,
        Keyboard::F2 => 0x3b,
        Keyboard::F3 => 0x3c,
        Keyboard::F4 => 0x3d,
        Keyboard::F5 => 0x3e,
        Keyboard::F6 => 0x3f,
        Keyboard::F7 => 0x40,
        Keyboard::F8 => 0x41,
        Keyboard::F9 => 0x42,
        Keyboard::F10 => 0x43,
        Keyboard::F11 => 0x44,
        Keyboard::F12 => 0x45,
        Keyboard::SysRq => 0x46,
        Keyboard::ScrollLock => 0x47,
        Keyboard::Pause => 0x48,
        Keyboard::Insert => 0x49,
        Keyboard::Home => 0x4a,
        Keyboard::PageUp => 0x4b,
        Keyboard::Delete => 0x4c,
        Keyboard::End => 0x4d,
        Keyboard::PageDown => 0x4e,
        Keyboard::Right => 0x4f,
        Keyboard::Left => 0x50,
        Keyboard::Down => 0x51,
        Keyboard::Up => 0x52,
        Keyboard::NumLock => 0x53,
        Keyboard::KpSlash => 0x54,
        Keyboard::KpAsterisk => 0x55,
        Keyboard::KpMinus => 0x56,
        Keyboard::KpPlus => 0x57,
        Keyboard::KpEnter => 0x58,
        Keyboard::Kp1 => 0x59,
        Keyboard::Kp2 => 0x5a,
        Keyboard::Kp3 => 0x5b,
        Keyboard::Kp4 => 0x5c,
        Keyboard::Kp5 => 0x5d,
        Keyboard::Kp6 => 0x5e,
        Keyboard::Kp7 => 0x5f,
        Keyboard::Kp8 => 0x60,
        Keyboard::Kp9 => 0x61,
        Keyboard::Kp0 => 0x62,
        Keyboard::KpDot => 0x63,
        Keyboard::N102nd => 0x64,
        Keyboard::Compose => 0x65,
        Keyboard::Power => 0x66,
        Keyboard::KpEqual => 0x67,
        Keyboard::F13 => 0x68,
        Keyboard::F14 => 0x69,
        Keyboard::F15 => 0x6a,
        Keyboard::F16 => 0x6b,
        Keyboard::F17 => 0x6c,
        Keyboard::F18 => 0x6d,
        Keyboard::F19 => 0x6e,
        Keyboard::F20 => 0x6f,
        Keyboard::F21 => 0x70,
        Keyboard::F22 => 0x71,
        Keyboard::F23 => 0x72,
        Keyboard::F24 => 0x73,
        Keyboard::Open => 0x74,
        Keyboard::Help => 0x75,
        Keyboard::Props => 0x76,
        Keyboard::Front => 0x77,
        Keyboard::Stop => 0x78,
        Keyboard::Again => 0x79,
        Keyboard::Undo => 0x7a,
        Keyboard::Cut => 0x7b,
        Keyboard::Copy => 0x7c,
        Keyboard::Paste => 0x7d,
        Keyboard::Find => 0x7e,
        Keyboard::Mute => 0x7f,
        Keyboard::VolumeUp => 0x80,
        Keyboard::VolumeDown => 0x81,
        Keyboard::KpComma => 0x85,
        Keyboard::Ro => 0x87,
        Keyboard::KatakanaHiragana => 0x88,
        Keyboard::Yen => 0x89,
        Keyboard::Henkan => 0x8a,
        Keyboard::Muhenkan => 0x8b,
        Keyboard::KpJpComma => 0x8c,
        Keyboard::Hangeul => 0x90,
        Keyboard::Hanja => 0x91,
        Keyboard::Katakana => 0x92,
        Keyboard::Hiragana => 0x93,
        Keyboard::ZenkakuHankaku => 0x94,
        Keyboard::KpLeftParen => 0xb6,
        Keyboard::KpRightParen => 0xb7,
        _ => return None,
    };

    Some(usage)
}

// Returns the bit of a mouse button in the mouse report.
pub fn button(button: Button) -> Option<u8> {
    let bit = match button {
        Button::Left => 0,
        Button::Right => 1,
        Button::Middle => 2,
        Button::Side | Button::Back => 3,
        Button::Extra | Button::Forward => 4,
        _ => return None,
    };

    Some(1 << bit)
}
