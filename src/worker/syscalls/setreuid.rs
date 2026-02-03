use super::{ArgsMap, SysCaller};
use syscalls::{Errno, Sysno, syscall};

#[derive(Debug)]
pub struct SetreuidCall {
    pub ruid: usize,
    pub euid: usize,
}

impl SetreuidCall {
    pub fn new(args: &ArgsMap) -> Self {
        let ruid = args.get("ruid", 0);
        let euid = args.get("euid", 0);

        Self { ruid, euid }
    }
}

impl SysCaller for SetreuidCall {
    fn call(&self) -> Result<usize, Errno> {
        unsafe { syscall!(Sysno::setreuid, self.ruid, self.euid) }
    }
}
