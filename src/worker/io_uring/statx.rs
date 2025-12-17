use std::ffi::CString;

use io_uring::opcode::Statx;
use io_uring::squeue::Entry;
use io_uring::{IoUring, types};
use syscalls::Errno;

use crate::worker::io_uring::ArgsMap;
use crate::worker::io_uring::IOUringCaller;

#[derive(Debug)]
#[allow(dead_code)]
pub(super) struct StatxIOUringCall {
    statx: Entry,

    pathname: CString, // used a raw pointer from string
    statx_struct: Box<libc::statx>, // used as a mutable raw pointer
}

impl StatxIOUringCall {
    pub fn new(args: &ArgsMap) -> Self {
        let pathname = args.get("pathname", CString::new("/tmp").unwrap());
        let flags = args.get("flags", 0);
        let mask = args.get("mask", 0);
        let mut statx_struct: Box<libc::statx> =
            Box::new(unsafe { std::mem::zeroed() });

        let statx = Statx::new(
            types::Fd(-1),
            pathname.as_ptr(),
            statx_struct.as_mut() as *mut libc::statx as *mut _,
        )
        .flags(flags)
        .mask(mask)
        .build();
        Self {
            statx,
            pathname,
            statx_struct,
        }
    }
}

impl IOUringCaller for StatxIOUringCall {
    fn submit(&self, ring: &mut IoUring) -> Result<usize, Errno> {
        unsafe {
            if ring.submission().push(&self.statx).is_err() {
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

        if cqe.result() == 0 {
            Ok(0)
        } else {
            Err(Errno::new(-cqe.result()))
        }
    }
}
