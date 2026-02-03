use std::ffi::CString;

use io_uring::opcode::OpenAt2;
use io_uring::squeue::Entry;
use io_uring::{IoUring, types};
use syscalls::{Errno, Sysno, syscall};

use crate::worker::io_uring::ArgsMap;
use crate::worker::io_uring::IOUringCaller;

#[derive(Debug)]
#[allow(dead_code)]
pub(super) struct Openat2IOUringCall {
    openat: Entry,

    pathname: CString, // used a raw pointer from string
    openhow: Box<types::OpenHow>,
}

impl Openat2IOUringCall {
    pub fn new(args: &ArgsMap) -> Self {
        let pathname = args.get("pathname", CString::new("/tmp").unwrap());
        let flags = args.get("flags", 0);
        let mode = args.get("mode", 0);
        let resolve = args.get("resolve", 0);
        let openhow = Box::new(
            types::OpenHow::new()
                .flags(flags)
                .mode(mode)
                .resolve(resolve),
        );

        let openat =
            OpenAt2::new(types::Fd(-1), pathname.as_ptr(), openhow.as_ref())
                .build();
        Self {
            openat,
            pathname,
            openhow,
        }
    }
}

impl IOUringCaller for Openat2IOUringCall {
    fn submit(&self, ring: &mut IoUring) -> Result<usize, Errno> {
        unsafe {
            if ring.submission().push(&self.openat).is_err() {
                return Err(Errno::ENOSPC);
            }
        }

        match ring.submit_and_wait(1) {
            Ok(_) => {}
            Err(e) => {
                return Err(Errno::new(
                    e.raw_os_error().unwrap_or(Errno::ENOSPC.into_raw()),
                ));
            }
        }

        let cqe = match ring.completion().next() {
            Some(cqe) => cqe,
            None => return Err(Errno::ENOSPC),
        };
        if cqe.result() > -1 {
            // Close file descriptor
            unsafe {
                let _ = syscall!(Sysno::close, cqe.result());
            }
            return Ok(cqe.result() as usize);
        }
        Err(Errno::new(-cqe.result()))
    }
}
