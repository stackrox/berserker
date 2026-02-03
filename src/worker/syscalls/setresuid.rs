use super::{ArgsMap, SysCaller};
use syscalls::{Errno, Sysno, syscall};

#[derive(Debug)]
pub struct SetresuidCall {
    pub ruid: usize,
    pub euid: usize,
    pub suid: usize,
}

impl SetresuidCall {
    pub fn new(args: &ArgsMap) -> Self {
        let ruid = args.get("ruid", 0);
        let euid = args.get("euid", 0);
        let suid = args.get("suid", 0);

        Self { ruid, euid, suid }
    }
}

impl SysCaller for SetresuidCall {
    fn call(&self) -> Result<usize, Errno> {
        unsafe { syscall!(Sysno::setresuid, self.ruid, self.euid, self.suid) }
    }
}
