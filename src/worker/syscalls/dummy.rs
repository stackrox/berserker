use syscalls::{Errno, Sysno, syscall};

use super::SysCaller;
use crate::ArgsMap;

#[derive(Debug)]
pub struct DummyCall {
    pub syscall: Sysno,
}

impl DummyCall {
    pub fn new(_args: &ArgsMap, syscall: Sysno) -> Self {
        Self { syscall }
    }
}

impl SysCaller for DummyCall {
    fn call(&self) -> Result<usize, Errno> {
        unsafe { syscall!(self.syscall) }
    }
}
