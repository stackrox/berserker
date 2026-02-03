use libc::{AF_INET, SOCK_STREAM};

use super::{ArgsMap, SysCaller};
use syscalls::{Errno, Sysno, syscall};

#[derive(Debug)]
pub struct SocketCall {
    pub domain: usize,
    pub stype: usize,
    pub protocol: usize,
}

impl SocketCall {
    pub fn new(args: &ArgsMap) -> Self {
        let domain = args.get("domain", AF_INET as usize);
        let stype = args.get("type", SOCK_STREAM as usize);
        let protocol = args.get("protocol", 0);

        Self {
            domain,
            stype,
            protocol,
        }
    }
}

impl SysCaller for SocketCall {
    fn call(&self) -> Result<usize, Errno> {
        let res = unsafe {
            syscall!(
                Sysno::socket,
                self.domain,
                self.stype | libc::SOCK_NONBLOCK as usize,
                self.protocol
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
