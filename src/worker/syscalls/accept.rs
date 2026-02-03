use syscalls::{Errno, Sysno, syscall};

use super::SysCaller;
use crate::ArgsMap;
use crate::worker::syscalls::listen::ListenCall;

#[derive(Debug)]
pub struct AcceptCall {
    pub accept_nr: Sysno,
    pub listen_call: ListenCall,
    pub sockfd: usize,
}

impl AcceptCall {
    pub fn new(args: &ArgsMap, accept_nr: Sysno) -> Self {
        let listen_call = ListenCall::new(args);
        let sockfd = 0;

        Self {
            accept_nr,
            listen_call,
            sockfd,
        }
    }
}

impl SysCaller for AcceptCall {
    fn init(&mut self) -> Result<usize, Errno> {
        self.sockfd = self.listen_call.init()?;
        self.listen_call.call()?;
        Ok(self.sockfd)
    }
    fn call(&self) -> Result<usize, Errno> {
        unsafe { syscall!(self.accept_nr, self.sockfd, 0, 0, 0) }
    }
}
