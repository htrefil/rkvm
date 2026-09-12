use rkvm_input::linux::writer::WriterLinux;

pub use crate::connection::init_stream as stream;

pub fn writers() -> WriterLinux {
    WriterLinux::new()
}
