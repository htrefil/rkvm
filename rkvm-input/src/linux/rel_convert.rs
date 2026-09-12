use crate::convert::Convert;
use crate::linux::glue;

use crate::rel::RelAxis;

impl Convert for RelAxis {
    type Raw = u16;

    fn from_raw(code: Self::Raw) -> Option<Self> {
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

    fn to_raw(&self) -> Option<Self::Raw> {
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

        Some(code as _)
    }
}
