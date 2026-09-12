
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