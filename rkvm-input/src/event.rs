use crate::abs::AbsEvent;
use crate::key::KeyEvent;
use crate::rel::RelEvent;

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

pub type Packet = SmallVec<[Event; 2]>;

#[derive(Debug, Serialize, Deserialize)]
pub enum Event {
    Rel(RelEvent),
    Abs(AbsEvent),
    Key(KeyEvent),
}
