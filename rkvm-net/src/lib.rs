pub mod auth;
pub mod message;
pub mod socket;
pub mod version;

use rkvm_input::abs::{AbsAxis, AbsInfo};
use rkvm_input::event::Event;
use rkvm_input::key::Key;
use rkvm_input::rel::RelAxis;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ffi::CString;

#[derive(Deserialize, Serialize, Debug)]
pub enum Update {
    CreateDevice {
        id: usize,
        name: CString,
        vendor: u16,
        product: u16,
        version: u16,
        rel: HashSet<RelAxis>,
        abs: HashMap<AbsAxis, AbsInfo>,
        keys: HashSet<Key>,
    },
    DestroyDevice {
        id: usize,
    },
    Event {
        id: usize,
        event: Event,
    },
}
