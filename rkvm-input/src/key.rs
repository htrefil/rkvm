mod button;
mod keyboard;

pub use button::Button;
pub use keyboard::Keyboard;

use crate::convert::Convert;

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

impl Convert for Key {
    type Raw = u16;

    fn from_raw(code: Self::Raw) -> Option<Self> {
        if let Some(key) = Keyboard::from_raw(code) {
            return Some(Self::Key(key));
        }

        if let Some(button) = Button::from_raw(code) {
            return Some(Self::Button(button));
        }

        None
    }

    fn to_raw(&self) -> Option<u16> {
        match self {
            Self::Key(key) => key.to_raw(),
            Self::Button(button) => button.to_raw(),
        }
    }
}
