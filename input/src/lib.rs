mod device_id;
mod event;
mod event_manager;
mod platforms;
mod glue;

pub use event::{Axis, Button, Direction, Event, Key, KeyKind};
pub use event_manager::EventManager;

#[cfg(target_os="linux")]
pub use platforms::linux::event_writer::EventWriter;
pub use platforms::linux::event_reader::EventReader;