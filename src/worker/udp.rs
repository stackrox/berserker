use std::{
    ffi::CString, fmt::Display, net::SocketAddr, slice::from_raw_parts,
    str::from_utf8,
};

use libc::{
    addrinfo, c_int, c_void, getaddrinfo, sendto, socket, strerror, strlen,
    AF_INET, AF_INET6, SOCK_DGRAM,
};

use crate::{Worker, WorkerError};

static LOREM_IPSUM: &[u8] = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. \
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. \
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. \
Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.\n";

#[derive(Debug)]
struct Errno {
    code: i32,
    msg: String,
}

impl Errno {
    fn new() -> Self {
        let code = unsafe { *libc::__errno_location() };
        let msg = unsafe {
            let m = strerror(code);
            let len = strlen(m);
            let m: &[u8] = from_raw_parts(m as *mut u8, len);
            from_utf8(m).unwrap()
        }
        .to_string();

        Errno { code, msg }
    }
}

impl Display for Errno {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Errno { code, msg } = self;
        write!(f, "({code}) {msg}")
    }
}

impl From<Errno> for WorkerError {
    fn from(val: Errno) -> Self {
        let msg = format!("{val}");
        WorkerError { msg }
    }
}

pub struct UdpClient {
    fd: c_int,
    target: addrinfo,
}

impl UdpClient {
    pub fn new(addr: &str) -> Self {
        let addr: SocketAddr = addr.parse().unwrap();
        let ai_family = if addr.is_ipv4() { AF_INET } else { AF_INET6 };
        let hints = addrinfo {
            ai_family,
            ai_socktype: SOCK_DGRAM,
            ai_flags: 0,
            ai_protocol: 0,
            ai_addrlen: 0,
            ai_addr: std::ptr::null_mut(),
            ai_canonname: std::ptr::null_mut(),
            ai_next: std::ptr::null_mut(),
        };

        let target = unsafe {
            let mut servinfo: *mut addrinfo = std::ptr::null_mut();
            let address = CString::new(addr.ip().to_string()).unwrap();
            let port = CString::new(addr.port().to_string()).unwrap();
            let ret = getaddrinfo(
                address.as_ptr(),
                port.as_ptr(),
                &hints,
                &mut servinfo,
            );

            if ret != 0 {
                panic!("getaddrinfo failed: {ret}");
            }
            servinfo.read()
        };

        let fd = create_socket(addr.is_ipv4()).unwrap();
        UdpClient { fd, target }
    }

    fn send_msg(&self, msg: &[u8]) -> Result<(), Errno> {
        let ret = unsafe {
            sendto(
                self.fd,
                msg.as_ptr() as *const c_void,
                msg.len(),
                0,
                self.target.ai_addr,
                self.target.ai_addrlen,
            )
        };

        if ret < 0 {
            Err(Errno::new())
        } else {
            Ok(())
        }
    }
}

impl Worker for UdpClient {
    fn run_payload(&self) -> Result<(), crate::WorkerError> {
        self.send_msg(LOREM_IPSUM)?;
        Ok(())
    }
}

fn create_socket(is_ipv4: bool) -> Result<c_int, Errno> {
    let domain = if is_ipv4 { AF_INET } else { AF_INET6 };
    let fd = unsafe { socket(domain, SOCK_DGRAM, 0) };
    if fd < 0 {
        Err(Errno::new())
    } else {
        Ok(fd)
    }
}
