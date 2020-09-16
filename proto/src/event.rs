#[derive(Clone, Copy, Debug)]
pub enum Event {
    MouseScroll { delta: i8 },
    MouseMove { x_delta: i8, y_delta: i8 },
    Key { up: bool, code: u16 },
}
