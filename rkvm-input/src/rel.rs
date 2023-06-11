use crate::glue;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RelEvent {
    pub axis: RelAxis,
    pub value: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum RelAxis {
    X,
    Y,
    Z,
    Rx,
    Ry,
    Rz,
    HWheel,
    Dial,
    Wheel,
    Misc,
    WheelHiRes,
    HWheelHiRes,
}

impl RelAxis {
    pub(crate) fn from_raw(code: u16) -> Option<Self> {
        let axis = match code as _ {
            glue::REL_X => Self::X,
            glue::REL_Y => Self::Y,
            glue::REL_Z => Self::Z,
            glue::REL_RX => Self::Rx,
            glue::REL_RY => Self::Ry,
            glue::REL_RZ => Self::Rz,
            glue::REL_HWHEEL => Self::HWheel,
            glue::REL_DIAL => Self::Dial,
            glue::REL_WHEEL => Self::Wheel,
            glue::REL_MISC => Self::Misc,
            glue::REL_WHEEL_HI_RES => Self::WheelHiRes,
            glue::REL_HWHEEL_HI_RES => Self::HWheelHiRes,
            _ => return None,
        };

        Some(axis)
    }

    pub(crate) fn to_raw(&self) -> u16 {
        let code = match self {
            Self::X => glue::REL_X,
            Self::Y => glue::REL_Y,
            Self::Z => glue::REL_Z,
            Self::Rx => glue::REL_RX,
            Self::Ry => glue::REL_RY,
            Self::Rz => glue::REL_RZ,
            Self::HWheel => glue::REL_HWHEEL,
            Self::Dial => glue::REL_DIAL,
            Self::Wheel => glue::REL_WHEEL,
            Self::Misc => glue::REL_MISC,
            Self::WheelHiRes => glue::REL_WHEEL_HI_RES,
            Self::HWheelHiRes => glue::REL_HWHEEL_HI_RES,
        };

        code as _
    }
}
