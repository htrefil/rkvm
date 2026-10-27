mod gadget;
mod hid;

use rkvm_input::event::Event;
use rkvm_input::key::{Key, KeyEvent};
use rkvm_input::rel::{RelAxis, RelEvent};
use rkvm_input::sync::SyncEvent;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::{io, mem};

use crate::client::{self, Error};
use crate::config;
use crate::output::{self, Output};
use crate::usb::gadget::{Function, Gadget};
use crate::usb::hid::Hid;

const KEYBOARD: &str = "keyboard";
const MOUSE: &str = "mouse";

// Forwards input to the host this machine is connected to over USB, acting as a USB keyboard and mouse.
//
// All devices of the server are merged into the single keyboard and mouse of the gadget.
pub struct Usb {
    keyboard: Hid,
    mouse: Hid,
    devices: HashMap<usize, Device>,
    // Last sent keyboard report.
    keyboard_report: [u8; hid::KEYBOARD_REPORT_LENGTH],
    // Last sent mouse button state.
    buttons: u8,
    // Declared last so that the device files are closed before the gadget is removed.
    _gadget: Gadget,
}

// State of a server device, motion is accumulated until the next sync event.
#[derive(Default)]
struct Device {
    keys: HashSet<Key>,
    x: i32,
    y: i32,
    wheel: i32,
    hwheel: i32,
}

impl Usb {
    pub async fn new(config: &config::Usb) -> Result<Self, io::Error> {
        let functions = [
            Function {
                name: KEYBOARD,
                subclass: 1, // Boot interface.
                protocol: 1, // Keyboard.
                report_length: hid::KEYBOARD_REPORT_LENGTH,
                report_descriptor: hid::KEYBOARD_DESCRIPTOR,
            },
            // The mouse report is not boot protocol compatible, so don't advertise it.
            Function {
                name: MOUSE,
                subclass: 0,
                protocol: 0,
                report_length: hid::MOUSE_REPORT_LENGTH,
                report_descriptor: hid::MOUSE_DESCRIPTOR,
            },
        ];

        let gadget = Gadget::create(config, &functions)?;

        let keyboard = Hid::open(&gadget.device(KEYBOARD)?).await?;
        let mouse = Hid::open(&gadget.device(MOUSE)?).await?;

        tracing::info!(udc = gadget.udc(), "Created USB gadget");

        Ok(Self {
            keyboard,
            mouse,
            devices: HashMap::new(),
            keyboard_report: [0; hid::KEYBOARD_REPORT_LENGTH],
            buttons: 0,
            _gadget: gadget,
        })
    }

    async fn flush_keyboard(&mut self) -> Result<(), Error> {
        let report = self.make_keyboard_report();
        if report == self.keyboard_report {
            return Ok(());
        }

        self.keyboard.write(&report).await.map_err(Error::Input)?;
        self.keyboard_report = report;

        Ok(())
    }

    async fn flush_mouse(
        &mut self,
        mut x: i32,
        mut y: i32,
        mut wheel: i32,
        mut hwheel: i32,
    ) -> Result<(), Error> {
        let buttons = self.make_buttons();

        // Motion that doesn't fit into a single report is split into several.
        while buttons != self.buttons || x != 0 || y != 0 || wheel != 0 || hwheel != 0 {
            let report_x = x.clamp(-hid::MOUSE_MOTION_MAX, hid::MOUSE_MOTION_MAX);
            let report_y = y.clamp(-hid::MOUSE_MOTION_MAX, hid::MOUSE_MOTION_MAX);
            let report_wheel = wheel.clamp(-hid::MOUSE_WHEEL_MAX, hid::MOUSE_WHEEL_MAX);
            let report_hwheel = hwheel.clamp(-hid::MOUSE_WHEEL_MAX, hid::MOUSE_WHEEL_MAX);

            x -= report_x;
            y -= report_y;
            wheel -= report_wheel;
            hwheel -= report_hwheel;

            let report_x = (report_x as i16).to_le_bytes();
            let report_y = (report_y as i16).to_le_bytes();
            let report = [
                buttons,
                report_x[0],
                report_x[1],
                report_y[0],
                report_y[1],
                report_wheel as i8 as u8,
                report_hwheel as i8 as u8,
            ];

            self.mouse.write(&report).await.map_err(Error::Input)?;
            self.buttons = buttons;
        }

        Ok(())
    }

    fn make_keyboard_report(&self) -> [u8; hid::KEYBOARD_REPORT_LENGTH] {
        let mut modifiers = 0;
        let mut usages = Vec::new();

        for key in self.devices.values().flat_map(|device| &device.keys) {
            let Key::Key(key) = key else {
                continue;
            };

            if let Some(modifier) = hid::modifier(*key) {
                modifiers |= modifier;
            } else if let Some(usage) = hid::usage(*key) {
                usages.push(usage);
            }
        }

        // The same key might be held on multiple devices.
        usages.sort_unstable();
        usages.dedup();

        let mut report = [0; hid::KEYBOARD_REPORT_LENGTH];
        report[0] = modifiers;

        if usages.len() > hid::KEYBOARD_ROLLOVER {
            report[2..].fill(hid::KEYBOARD_ERROR_ROLLOVER);
        } else {
            report[2..2 + usages.len()].copy_from_slice(&usages);
        }

        report
    }

    fn make_buttons(&self) -> u8 {
        self.devices
            .values()
            .flat_map(|device| &device.keys)
            .filter_map(|key| match key {
                Key::Button(button) => hid::button(*button),
                Key::Key(_) => None,
            })
            .fold(0, |buttons, button| buttons | button)
    }
}

impl Output for Usb {
    async fn create_device(&mut self, id: usize, device: output::Device) -> Result<(), Error> {
        let Entry::Vacant(entry) = self.devices.entry(id) else {
            return Err(client::protocol_error(
                "Server created the same device twice",
            ));
        };

        if !device.abs.is_empty() {
            tracing::warn!(
                %id,
                "Device has absolute axes, these are not supported in USB mode and will be ignored"
            );
        }

        entry.insert(Device::default());

        Ok(())
    }

    async fn destroy_device(&mut self, id: usize) -> Result<(), Error> {
        if self.devices.remove(&id).is_none() {
            return Err(client::protocol_error(
                "Server destroyed a nonexistent device",
            ));
        }

        // Release whatever the device was holding.
        self.flush_keyboard().await?;
        self.flush_mouse(0, 0, 0, 0).await?;

        Ok(())
    }

    async fn write(&mut self, id: usize, event: &Event) -> Result<(), Error> {
        let device = self.devices.get_mut(&id).ok_or_else(|| {
            client::protocol_error("Server sent an event to a nonexistent device")
        })?;

        match event {
            Event::Key(KeyEvent { key, down: true }) => {
                device.keys.insert(*key);
            }
            Event::Key(KeyEvent { key, down: false }) => {
                device.keys.remove(key);
            }
            Event::Rel(RelEvent { axis, value }) => {
                let accumulator = match axis {
                    RelAxis::X => &mut device.x,
                    RelAxis::Y => &mut device.y,
                    RelAxis::Wheel => &mut device.wheel,
                    RelAxis::HWheel => &mut device.hwheel,
                    _ => return Ok(()),
                };

                *accumulator = accumulator.saturating_add(*value);
            }
            Event::Sync(SyncEvent::All) => {
                let x = mem::take(&mut device.x);
                let y = mem::take(&mut device.y);
                let wheel = mem::take(&mut device.wheel);
                let hwheel = mem::take(&mut device.hwheel);

                self.flush_keyboard().await?;
                self.flush_mouse(x, y, wheel, hwheel).await?;
            }
            Event::Sync(SyncEvent::Mt) | Event::Abs(_) => {}
        }

        Ok(())
    }
}
