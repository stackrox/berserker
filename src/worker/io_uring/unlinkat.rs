use std::ffi::CString;

use io_uring::opcode::UnlinkAt;
use io_uring::squeue::Entry;
use io_uring::{IoUring, types};
use syscalls::Errno;

use crate::worker::io_uring::ArgsMap;
use crate::worker::io_uring::IOUringCaller;

#[derive(Debug)]
#[allow(dead_code)]
pub(super) struct UnlinkatIOUringCall {
    unlinkat: Entry,

    pathname: CString, // used a raw pointer from string
}

impl UnlinkatIOUringCall {
    pub fn new(args: &ArgsMap) -> Self {
        let pathname =
            args.get("pathname", CString::new("/not_existing_file").unwrap());
        let flags = args.get("flags", 0);

        let unlinkat = UnlinkAt::new(types::Fd(-1), pathname.as_ptr())
            .flags(flags)
            .build();
        Self { unlinkat, pathname }
    }
}

impl IOUringCaller for UnlinkatIOUringCall {
    fn submit(&self, ring: &mut IoUring) -> Result<usize, Errno> {
        unsafe {
            if ring.submission().push(&self.unlinkat).is_err() {
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
