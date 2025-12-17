use std::ffi::CString;

use super::{ArgsMap, SysCaller};
use syscalls::{Errno, Sysno, syscall};

#[derive(Debug)]
pub struct UnlinkCall {
    pub pathname: CString,
}

impl UnlinkCall {
    pub fn new(args: &ArgsMap) -> Self {
        let pathname =
            args.get("pathname", CString::new("/privileged_dir/file").unwrap());

        Self { pathname }
    }
}

impl SysCaller for UnlinkCall {
    fn call(&self) -> Result<usize, Errno> {
        unsafe { syscall!(Sysno::unlink, self.pathname.as_ptr()) }
    }
}
