use crate::convert::Convert;
use crate::glue;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Property {
    Pointer,
    Direct,
    ButtonPad,
    SemiMt,
    TopButtonPad,
    PointingStick,
    Accelerometer,
    PressurePad,
}

impl Convert for Property {
    type Raw = u16;

    fn from_raw(raw: Self::Raw) -> Option<Self> {
        let property = match raw as _ {
            glue::INPUT_PROP_POINTER => Self::Pointer,
            glue::INPUT_PROP_DIRECT => Self::Direct,
            glue::INPUT_PROP_BUTTONPAD => Self::ButtonPad,
            glue::INPUT_PROP_SEMI_MT => Self::SemiMt,
            glue::INPUT_PROP_TOPBUTTONPAD => Self::TopButtonPad,
            glue::INPUT_PROP_POINTING_STICK => Self::PointingStick,
            glue::INPUT_PROP_ACCELEROMETER => Self::Accelerometer,
            glue::INPUT_PROP_PRESSUREPAD => Self::PressurePad,
            _ => return None,
        };

        Some(property)
    }

    fn to_raw(&self) -> Option<Self::Raw> {
        let raw = match self {
            Self::Pointer => glue::INPUT_PROP_POINTER,
            Self::Direct => glue::INPUT_PROP_DIRECT,
            Self::ButtonPad => glue::INPUT_PROP_BUTTONPAD,
            Self::SemiMt => glue::INPUT_PROP_SEMI_MT,
            Self::TopButtonPad => glue::INPUT_PROP_TOPBUTTONPAD,
            Self::PointingStick => glue::INPUT_PROP_POINTING_STICK,
            Self::Accelerometer => glue::INPUT_PROP_ACCELEROMETER,
            Self::PressurePad => glue::INPUT_PROP_PRESSUREPAD,
        };

        Some(raw as _)
    }
}
