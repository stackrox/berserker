use libc::MS_PRIVATE;
use std::ffi::CString;

use super::{ArgsMap, SysCaller};
use syscalls::{Errno, Sysno};

#[derive(Debug)]
pub struct MountCall {
    pub source: CString,
    pub target: CString,
    pub filesystemtype: CString,
    pub mountflags: usize,
    pub data: usize,
}

impl MountCall {
    pub fn new(args: &ArgsMap) -> Self {
        let source = args.get("source", CString::new("").unwrap());
        let target = args.get("target", CString::new("/tmp").unwrap());
        let filesystemtype =
            args.get("filesystemtype", CString::new("").unwrap());
        let mountflags = args.get("mountflags", MS_PRIVATE as usize);
        let data = 0;

        Self {
            source,
            target,
            filesystemtype,
            mountflags,
            data,
        }
    }
}

impl SysCaller for MountCall {
    fn call(&self) -> Result<usize, Errno> {
        let res = unsafe {
            syscalls::syscall!(
                Sysno::mount,
                self.source.as_ptr(),
                self.target.as_ptr(),
                self.filesystemtype.as_ptr(),
                self.mountflags,
                self.data
            )
        };

        if let Ok(code) = res
            && code == 0
        {
            // Unmount the file system
            unsafe {
                let _ =
                    syscalls::syscall!(Sysno::umount2, self.target.as_ptr());
            }
        }

        res
    }
}
