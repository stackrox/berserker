use syscalls::{Errno, Sysno, syscall};

use super::SysCaller;
use crate::ArgsMap;
use crate::worker::syscalls::socket::SocketCall;

#[derive(Debug)]
pub struct ListenCall {
    pub socket_call: SocketCall,
    pub sockfd: usize,
}

impl ListenCall {
    pub fn new(args: &ArgsMap) -> Self {
        let socket_call = SocketCall::new(args);
        let sockfd = 0;

        Self {
            socket_call,
            sockfd,
        }
    }
}

impl Drop for ListenCall {
    fn drop(&mut self) {
        unsafe {
            let _ = syscall!(Sysno::close, self.sockfd);
        }
    }
}

impl SysCaller for ListenCall {
    fn init(&mut self) -> Result<usize, Errno> {
        // Create socket directly instead of calling socket_call.call()
        // since that would immediately close the fd.
        self.sockfd = unsafe {
            syscall!(
                Sysno::socket,
                self.socket_call.domain,
                self.socket_call.stype | libc::SOCK_NONBLOCK as usize,
                self.socket_call.protocol
            )?
        };
        Ok(self.sockfd)
    }
    fn call(&self) -> Result<usize, Errno> {
        unsafe { syscall!(Sysno::listen, self.sockfd, 10) }
    }
}
