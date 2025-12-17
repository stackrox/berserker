use std::ffi::CString;

use super::{ArgsMap, SysCaller};
use syscalls::{self, Sysno, syscall};

#[derive(Debug)]
pub struct ChownCall {
    pub pathname: CString,
    pub owner: usize,
    pub group: usize,
}

impl ChownCall {
    pub fn new(args: &ArgsMap) -> Self {
        let pathname = args.get("pathname", CString::new("/tmp").unwrap());
        let owner = args.get("owner", 0);
        let group = args.get("group", 0);

        Self {
            pathname,
            owner,
            group,
        }
    }
}

impl SysCaller for ChownCall {
    fn call(&self) -> Result<usize, syscalls::Errno> {
        unsafe {
            syscall!(
                Sysno::chown,
                self.pathname.as_ptr(),
                self.owner,
                self.group
            )
        }
    }
}
