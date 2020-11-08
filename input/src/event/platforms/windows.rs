use super::Key;
impl Key {
    //This should probably return an Option<u16> now? Not all keys will be representable on all platforms.
    pub(crate) fn to_raw(&self) -> u16 {
        use Key::*;
        match *self {
            Numeric0 => 0x30,
            Numeric1 => 0x31,
            Numeric2 => 0x32,
            Numeric3 => 0x33,
            Numeric4 => 0x34,
            Numeric5 => 0x35,
            Numeric6 => 0x36,
            Numeric7 => 0x37,
            Numeric8 => 0x38,
            Numeric9 => 0x39,
            //10+ can't be represented in windows.
            A => 0x41,
            B => 0x42,
            C => 0x43,
            D => 0x44,
            E => 0x45,
            F => 0x46,
            G => 0x47,
            H => 0x48,
            I => 0x49,
            J => 0x4A,
            K => 0x4B,
            L => 0x4C,
            M => 0x4D,
            N => 0x4E,
            O => 0x4F,
            P => 0x50,
            Q => 0x51,
            R => 0x52,
            S => 0x53,
            T => 0x54,
            U => 0x55,
            V => 0x56,
            W => 0x57,
            X => 0x58,
            Y => 0x59,
            Z => 0x5A,
            _ => 0x00,
        }
    }

    pub(crate) fn from_raw(code: u16) -> Option<Self> {
        use Key::*;

        match code {
            0x30=> Numeric0 ,
            0x31=> Numeric1 ,
            0x32=> Numeric2 ,
            0x33=> Numeric3 ,
            0x34=> Numeric4 ,
            0x35=> Numeric5 ,
            0x36=> ric6 ,
            0x37=> ric7 ,
            0x38=> ric8 ,
            0x39=> ric9 ,
            //10+ can't be represented in windows.
            0x41=> A ,
            0x42=>  B ,
            0x43=> C ,
            0x44=> D ,
            0x45=> E ,
            0x46=> F ,
            0x47=> G ,
            0x48=> H ,
            0x49=> I ,
            0x4A=> J ,
            0x4B=> K ,
            0x4C=> L ,
            0x4D=> M ,
            0x4E=> N ,
            0x4F=> O ,
            0x50=> P ,
            0x51=> Q ,
            0x52=> R ,
            0x53=> S ,
            0x54=> T ,
            0x55=> U ,
            0x56=> V ,
            0x57=> W ,
            0x58=> X ,
            0x59=> Y ,
            0x5A=> Z ,
            0x00=> _ ,
        }
    }
}