mod button;
mod keyboard;

pub use button::Button;
pub use keyboard::Keyboard;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct KeyEvent {
    pub key: Key,
    pub down: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Key {
    Key(Keyboard),
    Button(Button),
}

