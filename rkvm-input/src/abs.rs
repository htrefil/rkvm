use crate::glue;

use serde::{Deserialize, Serialize};

// #define ABS_MT_SLOT		0x2f	/* MT slot being modified */
// #define ABS_MT_TOUCH_MAJOR	0x30	/* Major axis of touching ellipse */
// #define ABS_MT_TOUCH_MINOR	0x31	/* Minor axis (omit if circular) */
// #define ABS_MT_WIDTH_MAJOR	0x32	/* Major axis of approaching ellipse */
// #define ABS_MT_WIDTH_MINOR	0x33	/* Minor axis (omit if circular) */
// #define ABS_MT_ORIENTATION	0x34	/* Ellipse orientation */
// #define ABS_MT_POSITION_X	0x35	/* Center X touch position */
// #define ABS_MT_POSITION_Y	0x36	/* Center Y touch position */
// #define ABS_MT_TOOL_TYPE	0x37	/* Type of touching device */
// #define ABS_MT_BLOB_ID		0x38	/* Group a set of packets as a blob */
// #define ABS_MT_TRACKING_ID	0x39	/* Unique ID of initiated contact */
// #define ABS_MT_PRESSURE		0x3a	/* Pressure on contact area */
// #define ABS_MT_DISTANCE		0x3b	/* Contact hover distance */
// #define ABS_MT_TOOL_X		0x3c	/* Center X tool position */
// #define ABS_MT_TOOL_Y		0x3d	/* Center Y tool position */
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AbsEvent {
    pub axis: AbsAxis,
    pub value: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum AbsAxis {
    X,
    Y,
    Z,
    Rx,
    Ry,
    Rz,
    Throttle,
    Rudder,
    Wheel,
    Gas,
    Brake,
    Hat0X,
    Hat0Y,
    Hat1X,
    Hat1Y,
    Hat2X,
    Hat2Y,
    Hat3X,
    Hat3Y,
    Pressure,
    Distance,
    TiltX,
    TiltY,
    ToolWidth,
    Volume,
    Profile,
    Misc,
}

impl AbsAxis {
    pub(crate) fn from_raw(code: u16) -> Option<Self> {
        let axis = match code as _ {
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
            glue::ABS_PROFILE => Self::Profile,
            glue::ABS_MISC => Self::Misc,
            _ => return None,
        };

        Some(axis)
    }

    pub(crate) fn to_raw(&self) -> u16 {
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
            Self::Profile => glue::ABS_PROFILE,
            Self::Misc => glue::ABS_MISC,
        };

        code as _
    }
}

// See struct input_absinfo.
#[derive(Clone, Copy, Deserialize, Serialize, Debug)]
pub struct AbsInfo {
    pub min: i32,
    pub max: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ToolType {
    Finger,
    Pen,
    Palm,
    Dial,
}
