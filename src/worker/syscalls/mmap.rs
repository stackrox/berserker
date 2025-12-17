use libc::{MAP_ANONYMOUS, MAP_PRIVATE, PROT_EXEC, PROT_READ, PROT_WRITE};

use super::{ArgsMap, SysCaller};
use syscalls::{Errno, Sysno, syscall};

#[derive(Debug)]
pub struct MmapCall {
    pub address: usize,
    pub length: usize,
    pub prot: usize,
    pub flags: usize,
    pub fd: usize,
    pub offset: usize,
}

impl MmapCall {
    pub fn new(args: &ArgsMap) -> Self {
        let address = 0;
        let length = args.get("length", 8);
        let prot =
            args.get("prot", (PROT_READ | PROT_WRITE | PROT_EXEC) as usize);
        let flags = args.get("flags", (MAP_PRIVATE | MAP_ANONYMOUS) as usize);
        let fd = args.get("fd", usize::MAX); // -1
        let offset = args.get("offset", 0);

        Self {
            address,
            length,
            prot,
            flags,
            fd,
            offset,
        }
    }
}

impl Drop for MmapCall {
    fn drop(&mut self) {
        unsafe {
            let _ = syscall!(Sysno::close, self.fd);
        }
    }
}

impl SysCaller for MmapCall {
    fn call(&self) -> Result<usize, Errno> {
        let res = unsafe {
            syscall!(
                Sysno::mmap,
                self.address,
                self.length,
                self.prot,
                self.flags,
                self.fd,
                self.offset
            )
        };

        if let Ok(addr) = res {
            // Unmap memory
            unsafe { syscall!(Sysno::munmap, addr, self.length)? };
        }

        res
    }
}
