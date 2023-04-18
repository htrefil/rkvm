mod button;
mod key;

use crate::event::{Button, Key, KeyKind};

impl KeyKind {
    pub(crate) fn from_raw(code: u32) -> Option<KeyKind> {
        Key::from_raw(code)
            .map(KeyKind::Key)
            .or_else(|| Button::from_raw(code).map(KeyKind::Button))
    }

    pub(crate) fn to_raw(&self) -> u32 {
        match self {
            KeyKind::Key(key) => key.to_raw(),
            KeyKind::Button(button) => button.to_raw(),
        }
    }
}
