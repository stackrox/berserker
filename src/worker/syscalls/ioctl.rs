use super::SysCaller;
use crate::ArgsMap;
use std::fs::File;
use std::os::fd::IntoRawFd;
use syscalls::syscall;
use syscalls::{Errno, Sysno};

#[derive(Debug, Default)]
pub struct IoctlCall {
    pub fd: usize,
    pub op: usize,
    pub argp: usize,
}

impl IoctlCall {
    pub fn new(_: &ArgsMap) -> Self {
        // Zero initialize all fields, fd will be initialized in `Syscaller::init`.
        // All other fields can be overridden as needed
        Default::default()
    }
}

impl Drop for IoctlCall {
    fn drop(&mut self) {
        unsafe {
            let _ = syscall!(Sysno::close, self.fd);
        }
    }
}

impl SysCaller for IoctlCall {
    fn init(&mut self) -> Result<usize, Errno> {
        self.fd = match File::open("/dev/null") {
            Ok(f) => f.into_raw_fd() as usize,
            Err(e) => return Err(Errno::new(e.raw_os_error().unwrap())),
        };
        Ok(self.fd)
    }
    fn call(&self) -> Result<usize, Errno> {
        unsafe { syscall!(Sysno::ioctl, self.fd, self.op, self.argp) }
    }
}
