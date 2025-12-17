use std::ffi::CString;

use syscalls::{Errno, Sysno, syscall};

use super::SysCaller;
use crate::ArgsMap;

#[derive(Debug)]
pub struct OpenCall {
    pub pathname: CString,
    pub flags: usize,
    pub mode: usize,
}

impl OpenCall {
    pub fn new(args: &ArgsMap) -> Self {
        let pathname = args.get("pathname", CString::new("/tmp").unwrap());
        let flags = args.get("flags", 0);
        let mode = args.get("mode", 0);

        Self {
            pathname,
            flags,
            mode,
        }
    }
}

impl SysCaller for OpenCall {
    fn call(&self) -> Result<usize, Errno> {
        let res = unsafe {
            syscall!(Sysno::open, self.pathname.as_ptr(), self.flags, self.mode)
        };

        if let Ok(fd) = res {
            // Close file descriptor
            unsafe {
                let _ = syscall!(Sysno::close, fd);
            }
        }

        res
    }
}
