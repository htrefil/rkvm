use crate::event::Button;
use winapi::um::winuser;

impl Button {
    pub(crate) fn to_raw(&self) -> Option<u16> {
        use Button::*;

        let code = match *self {
            Left => winuser::VK_LBUTTON,
            Right => winuser::VK_RBUTTON,
            Middle => winuser::VK_MBUTTON,
            X => winuser::VK_XBUTTON1,
            Y => winuser::VK_XBUTTON2, // TODO: Check for correctness.
            _ => return None,
        };

        Some(code as _)
    }
}
