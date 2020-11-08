mod device_id;
mod event;
mod event_manager;
mod glue;

pub(crate) mod platforms;

pub use event::{Axis, Button, Direction, Event, Key, KeyKind};
pub use event_manager::EventManager;

#[cfg(target_os = "linux")]
pub use platforms::{event_reader, event_writer};
