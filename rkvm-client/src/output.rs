use rkvm_input::abs::{AbsAxis, AbsInfo};
use rkvm_input::bus::BusType;
use rkvm_input::event::Event;
use rkvm_input::key::Key;
use rkvm_input::property::Property;
use rkvm_input::rel::RelAxis;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;

use crate::client::Error;

// Where the events received from the server end up.
pub trait Output {
    async fn create_device(&mut self, id: usize, device: Device) -> Result<(), Error>;

    async fn destroy_device(&mut self, id: usize) -> Result<(), Error>;

    async fn write(&mut self, id: usize, event: &Event) -> Result<(), Error>;
}

pub struct Device {
    pub name: CString,
    pub bus_type: BusType,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
    pub rel: HashSet<RelAxis>,
    pub abs: HashMap<AbsAxis, AbsInfo>,
    pub keys: HashSet<Key>,
    pub properties: HashSet<Property>,
    pub delay: Option<i32>,
    pub period: Option<i32>,
}
