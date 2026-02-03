use std::mem;

use libc::{AF_INET, htons};
use syscalls::{Errno, Sysno, syscall};

use super::SysCaller;
use crate::ArgsMap;
use crate::worker::syscalls::socket::SocketCall;

#[derive(Debug)]
pub struct ConnectCall {
    pub socket_call: SocketCall,
    pub sockfd: usize,
    pub serv_addr: libc::sockaddr_in,
    pub addrlen: usize,
}

impl ConnectCall {
    pub fn new(args: &ArgsMap) -> Self {
        let socket_call = SocketCall::new(args);
        let sockfd = 0;
        let serv_addr = libc::sockaddr_in {
            sin_family: AF_INET as u16,
            sin_port: htons(63333),
            sin_addr: libc::in_addr {
                s_addr: u32::from_le_bytes([127, 0, 0, 1]),
            },
            sin_zero: Default::default(),
        };
        let addrlen = mem::size_of::<libc::sockaddr_in>();

        Self {
            socket_call,
            sockfd,
            serv_addr,
            addrlen,
        }
    }
}

impl Drop for ConnectCall {
    fn drop(&mut self) {
        unsafe {
            let _ = syscall!(Sysno::close, self.sockfd);
        }
    }
}

impl SysCaller for ConnectCall {
    fn init(&mut self) -> Result<usize, Errno> {
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
        unsafe {
            syscall!(
                Sysno::connect,
                self.sockfd,
                &self.serv_addr as *const libc::sockaddr_in as usize,
                self.addrlen
            )
        }
    }
}
