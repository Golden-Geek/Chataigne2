use super::{ProcessSnapshot, ProcessSpec};
use std::io;
pub(super) struct Process;
fn unsupported<T>() -> io::Result<T> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "owned process trees require Windows 10 or newer",
    ))
}
impl Process {
    pub fn create(_: &ProcessSpec) -> io::Result<Self> {
        unsupported()
    }
    pub fn id(&self) -> u32 {
        unreachable!("unsupported platforms cannot construct an owned process")
    }
    pub fn snapshot(&self) -> io::Result<ProcessSnapshot> {
        unsupported()
    }
    pub fn resume(&mut self) -> io::Result<()> {
        unsupported()
    }
    pub fn request_close(&self) -> io::Result<u32> {
        unsupported()
    }
    pub fn terminate(&self, _: u32) -> io::Result<()> {
        unsupported()
    }
}
