use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AbsEvent {
    Axis { axis: AbsAxis, value: i32 },
    MtToolType { value: ToolType },
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
    MtSlot,
    MtTouchMajor,
    MtTouchMinor,
    MtWidthMajor,
    MtWidthMinor,
    MtOrientation,
    MtPositionX,
    MtPositionY,
    MtBlobId,
    MtTrackingId,
    MtPressure,
    MtDistance,
    MtToolX,
    MtToolY,
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

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum ToolType {
    Finger,
    Pen,
    Palm,
    Dial,
}
