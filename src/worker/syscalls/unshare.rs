use super::{ArgsMap, SysCaller};
use syscalls::{Errno, Sysno};

#[derive(Debug)]
pub struct UnshareCall {
    pub flags: usize,
}

impl UnshareCall {
    pub fn new(args: &ArgsMap) -> Self {
        let flags = args.get("flags", 0);

        Self { flags }
    }
}

impl SysCaller for UnshareCall {
    fn call(&self) -> Result<usize, Errno> {
        unsafe { syscalls::syscall!(Sysno::unshare, self.flags) }
    }
}
