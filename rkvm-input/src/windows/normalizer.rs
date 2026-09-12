
pub struct AxisNormalizer {
    min: i32,
    max: i32,
}

impl AxisNormalizer {
    pub fn new(min: i32, max: i32) -> Self {
        Self { min, max }
    }
    pub fn normalize(&self, value: i32) -> i32 {
        if self.max == self.min {
            return 0;
        }
        return (value * 65535 - self.min) / (self.max - self.min)
    }
}