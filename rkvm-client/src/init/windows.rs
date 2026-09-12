use rkvm_input::windows::writer_simple::WriterWindowsSimple;
use rkvm_input::windows::writer::WriterWindows;

pub use crate::connection::init_stream as stream;

pub fn writers() -> WriterWindows {
    WriterWindows::new(WriterWindowsSimple::new())
}
