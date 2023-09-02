pub trait Convert: Sized {
    type Raw;

    fn from_raw(raw: Self::Raw) -> Option<Self>;

    fn to_raw(&self) -> Option<Self::Raw>;
}
