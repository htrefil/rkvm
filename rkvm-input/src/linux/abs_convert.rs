use crate::convert::Convert;
use crate::linux::glue;

use crate::abs::{AbsAxis, ToolType};

impl Convert for AbsAxis {
    type Raw = u16;

    fn from_raw(raw: Self::Raw) -> Option<Self> {
        let axis = match raw as _ {
            glue::ABS_X => Self::X,
            glue::ABS_Y => Self::Y,
            glue::ABS_Z => Self::Z,
            glue::ABS_RX => Self::Rx,
            glue::ABS_RY => Self::Ry,
            glue::ABS_RZ => Self::Rz,
            glue::ABS_THROTTLE => Self::Throttle,
            glue::ABS_RUDDER => Self::Rudder,
            glue::ABS_WHEEL => Self::Wheel,
            glue::ABS_GAS => Self::Gas,
            glue::ABS_BRAKE => Self::Brake,
            glue::ABS_HAT0X => Self::Hat0X,
            glue::ABS_HAT0Y => Self::Hat0Y,
            glue::ABS_HAT1X => Self::Hat1X,
            glue::ABS_HAT1Y => Self::Hat1Y,
            glue::ABS_HAT2X => Self::Hat2X,
            glue::ABS_HAT2Y => Self::Hat2Y,
            glue::ABS_HAT3X => Self::Hat3X,
            glue::ABS_HAT3Y => Self::Hat3Y,
            glue::ABS_PRESSURE => Self::Pressure,
            glue::ABS_DISTANCE => Self::Distance,
            glue::ABS_TILT_X => Self::TiltX,
            glue::ABS_TILT_Y => Self::TiltY,
            glue::ABS_TOOL_WIDTH => Self::ToolWidth,
            glue::ABS_VOLUME => Self::Volume,
            #[cfg(feature = "abs_profile")]
            glue::ABS_PROFILE => Self::Profile,
            glue::ABS_MISC => Self::Misc,
            glue::ABS_MT_SLOT => Self::MtSlot,
            glue::ABS_MT_TOUCH_MAJOR => Self::MtTouchMajor,
            glue::ABS_MT_TOUCH_MINOR => Self::MtTouchMinor,
            glue::ABS_MT_WIDTH_MAJOR => Self::MtWidthMajor,
            glue::ABS_MT_WIDTH_MINOR => Self::MtWidthMinor,
            glue::ABS_MT_ORIENTATION => Self::MtOrientation,
            glue::ABS_MT_POSITION_X => Self::MtPositionX,
            glue::ABS_MT_POSITION_Y => Self::MtPositionY,
            glue::ABS_MT_BLOB_ID => Self::MtBlobId,
            glue::ABS_MT_TRACKING_ID => Self::MtTrackingId,
            glue::ABS_MT_PRESSURE => Self::MtPressure,
            glue::ABS_MT_DISTANCE => Self::MtDistance,
            glue::ABS_MT_TOOL_X => Self::MtToolX,
            glue::ABS_MT_TOOL_Y => Self::MtToolY,
            _ => return None,
        };

        Some(axis)
    }

    fn to_raw(&self) -> Option<Self::Raw> {
        let code = match self {
            Self::X => glue::ABS_X,
            Self::Y => glue::ABS_Y,
            Self::Z => glue::ABS_Z,
            Self::Rx => glue::ABS_RX,
            Self::Ry => glue::ABS_RY,
            Self::Rz => glue::ABS_RZ,
            Self::Throttle => glue::ABS_THROTTLE,
            Self::Rudder => glue::ABS_RUDDER,
            Self::Wheel => glue::ABS_WHEEL,
            Self::Gas => glue::ABS_GAS,
            Self::Brake => glue::ABS_BRAKE,
            Self::Hat0X => glue::ABS_HAT0X,
            Self::Hat0Y => glue::ABS_HAT0Y,
            Self::Hat1X => glue::ABS_HAT1X,
            Self::Hat1Y => glue::ABS_HAT1Y,
            Self::Hat2X => glue::ABS_HAT2X,
            Self::Hat2Y => glue::ABS_HAT2Y,
            Self::Hat3X => glue::ABS_HAT3X,
            Self::Hat3Y => glue::ABS_HAT3Y,
            Self::Pressure => glue::ABS_PRESSURE,
            Self::Distance => glue::ABS_DISTANCE,
            Self::TiltX => glue::ABS_TILT_X,
            Self::TiltY => glue::ABS_TILT_Y,
            Self::ToolWidth => glue::ABS_TOOL_WIDTH,
            Self::Volume => glue::ABS_VOLUME,
            #[cfg(feature = "abs_profile")]
            Self::Profile => glue::ABS_PROFILE,
            #[cfg(not(feature = "abs_profile"))]
            Self::Profile => return None,
            Self::Misc => glue::ABS_MISC,
            Self::MtSlot => glue::ABS_MT_SLOT,
            Self::MtTouchMajor => glue::ABS_MT_TOUCH_MAJOR,
            Self::MtTouchMinor => glue::ABS_MT_TOUCH_MINOR,
            Self::MtWidthMajor => glue::ABS_MT_WIDTH_MAJOR,
            Self::MtWidthMinor => glue::ABS_MT_WIDTH_MINOR,
            Self::MtOrientation => glue::ABS_MT_ORIENTATION,
            Self::MtPositionX => glue::ABS_MT_POSITION_X,
            Self::MtPositionY => glue::ABS_MT_POSITION_Y,
            Self::MtBlobId => glue::ABS_MT_BLOB_ID,
            Self::MtTrackingId => glue::ABS_MT_TRACKING_ID,
            Self::MtPressure => glue::ABS_MT_PRESSURE,
            Self::MtDistance => glue::ABS_MT_DISTANCE,
            Self::MtToolX => glue::ABS_MT_TOOL_X,
            Self::MtToolY => glue::ABS_MT_TOOL_Y,
        };

        Some(code as _)
    }
}

impl Convert for ToolType {
    type Raw = i32;

    fn from_raw(raw: Self::Raw) -> Option<Self> {
        let r#type = match raw as _ {
            glue::MT_TOOL_FINGER => Self::Finger,
            glue::MT_TOOL_PEN => Self::Pen,
            glue::MT_TOOL_PALM => Self::Palm,
            glue::MT_TOOL_DIAL => Self::Dial,
            _ => return None,
        };

        Some(r#type)
    }

    fn to_raw(&self) -> Option<Self::Raw> {
        let value = match self {
            Self::Finger => glue::MT_TOOL_FINGER,
            Self::Pen => glue::MT_TOOL_PEN,
            Self::Palm => glue::MT_TOOL_PALM,
            Self::Dial => glue::MT_TOOL_DIAL,
        };

        Some(value as _)
    }
}
