use crate::abs::AbsEvent;
use crate::key::KeyEvent;
use crate::rel::RelEvent;
use crate::sync::SyncEvent;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Event {
    Rel(RelEvent),
    Abs(AbsEvent),
    Key(KeyEvent),
    Sync(SyncEvent),
}
