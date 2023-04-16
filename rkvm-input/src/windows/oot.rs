// Used to prevent frequent allocation in cases where one key corresponds to multiple INPUT events.
#[derive(Eq, PartialEq, Hash)]
pub enum Oot<T> {
    V1([T; 1]),
    V2([T; 2]),
}

impl<T> Oot<T> {
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        match self {
            Oot::V1(values) => &mut *values,
            Oot::V2(values) => &mut *values,
        }
    }

    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> Oot<U> {
        match self {
            Oot::V1([a]) => Oot::V1([f(a)]),
            Oot::V2([a, b]) => Oot::V2([f(a), f(b)]),
        }
    }
}
