use super::{ArgsMap, SysCaller};
use syscalls::{Errno, Sysno, syscall};

#[derive(Debug)]
pub struct SetuidCall {
    pub uid: usize,
}

impl SetuidCall {
    pub fn new(args: &ArgsMap) -> Self {
        let uid = args.get("uid", 0);

        Self { uid }
    }
}

impl SysCaller for SetuidCall {
    fn call(&self) -> Result<usize, Errno> {
        unsafe { syscall!(Sysno::setuid, self.uid) }
    }
}
