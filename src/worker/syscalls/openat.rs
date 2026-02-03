use std::ffi::CString;

use super::SysCaller;
use crate::ArgsMap;
use syscalls::{Errno, Sysno, syscall};

#[derive(Debug)]
pub struct OpenatCall {
    pub dirfd: usize,
    pub pathname: CString,
    pub flags: usize,
    pub mode: usize,
}

impl OpenatCall {
    pub fn new(args: &ArgsMap) -> Self {
        let dirfd = 0; // Default value, can be overridden if needed
        let pathname = args.get("pathname", CString::new("/tmp").unwrap());
        let flags = args.get("flags", 0);
        let mode = args.get("mode", 0);

        Self {
            dirfd,
            pathname,
            flags,
            mode,
        }
    }
}

impl SysCaller for OpenatCall {
    fn call(&self) -> Result<usize, Errno> {
        let res = unsafe {
            syscall!(
                Sysno::openat,
                self.dirfd,
                self.pathname.as_ptr(),
                self.flags,
                self.mode
            )
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
