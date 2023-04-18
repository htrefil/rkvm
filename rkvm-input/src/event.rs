mod button;
mod key;

pub use button::Button;
pub use key::Key;

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

pub type EventPack = SmallVec<[Event; 2]>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Event {
    MouseScroll { axis: Axis, delta: i32 },
    MouseMove { axis: Axis, delta: i32 },
    Key { direction: Direction, kind: KeyKind },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Axis {
    X,
    Y,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum Direction {
    Up,   // The key is released.
    Down, // The key is pressed.
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum KeyKind {
    Key(Key),
    Button(Button),
}
