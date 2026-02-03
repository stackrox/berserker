mod accept;
mod capset;
mod chmod;
mod chown;
mod connect;
mod dummy;
mod ioctl;
mod listen;
mod mmap;
mod mount;
mod open;
mod openat;
mod prctl;
mod setresuid;
mod setreuid;
mod setuid;
mod socket;
mod unlink;
mod unshare;

use std::time::Instant;
use std::{fmt::Display, thread, time};

use core_affinity::CoreId;
use enum_dispatch::enum_dispatch;
use log::{debug, error, info, trace};
use rand::{Rng, thread_rng};
use rand_distr::Exp;
use syscalls::{Errno, Sysno};

use crate::ArgsMap;
use crate::worker::syscalls::accept::AcceptCall;
use crate::worker::syscalls::capset::CapsetCall;
use crate::worker::syscalls::chmod::ChmodCall;
use crate::worker::syscalls::chown::ChownCall;
use crate::worker::syscalls::connect::ConnectCall;
use crate::worker::syscalls::dummy::DummyCall;
use crate::worker::syscalls::ioctl::IoctlCall;
use crate::worker::syscalls::listen::ListenCall;
use crate::worker::syscalls::mmap::MmapCall;
use crate::worker::syscalls::mount::MountCall;
use crate::worker::syscalls::open::OpenCall;
use crate::worker::syscalls::openat::OpenatCall;
use crate::worker::syscalls::prctl::PrctlCall;
use crate::worker::syscalls::setresuid::SetresuidCall;
use crate::worker::syscalls::setreuid::SetreuidCall;
use crate::worker::syscalls::setuid::SetuidCall;
use crate::worker::syscalls::socket::SocketCall;
use crate::worker::syscalls::unlink::UnlinkCall;
use crate::worker::syscalls::unshare::UnshareCall;
use crate::{BaseConfig, Worker, WorkerError, Workload, WorkloadConfig};

#[derive(Debug, Clone)]
pub struct SyscallsWorker {
    config: BaseConfig,
    workload: WorkloadConfig,
}

impl SyscallsWorker {
    pub fn new(workload: WorkloadConfig, cpu: CoreId, process: usize) -> Self {
        SyscallsWorker {
            config: BaseConfig { cpu, process },
            workload,
        }
    }
}

impl Worker for SyscallsWorker {
    fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        let mut counter = 0;
        let mut start = Instant::now();

        let Workload::Syscalls {
            arrival_rate,
            tight_loop,
            syscall_nr,
            syscall_args,
        } = &self.workload.workload
        else {
            unreachable!()
        };

        let syscall = Sysno::from(*syscall_nr);
        let mut caller = SysCallerEnum::new(syscall, syscall_args);
        if let Err(e) = caller.init() {
            error!("Error initializing syscall: {:?}", e);
            return Err(WorkerError::Internal);
        };

        let exp = Exp::new(*arrival_rate).unwrap();
        let rng = thread_rng();
        let mut rng_iter = rng.sample_iter(exp);

        info!("Running syscall {syscall}");
        loop {
            if start.elapsed().as_secs() > 10 {
                info!(
                    "CPU {}, {}",
                    self.config.cpu.id,
                    counter / start.elapsed().as_secs()
                );
                start = Instant::now();
                counter = 0;
            }

            counter += 1;
            // Do the syscall directly, without spawning a thread (it would
            // introduce too much overhead for a quick syscall).
            match caller.call() {
                Ok(_) => trace!(
                    "{}-{}: Success",
                    self.config.cpu.id, self.config.process
                ),
                Err(e) => debug!(
                    "{}-{}: Error: {:?}",
                    self.config.cpu.id, self.config.process, e
                ),
            }
            // If running in a tight loop, go to the next iteration
            if *tight_loop {
                continue;
            }

            // Otherwise calculate waiting time
            let interval: f64 = rng_iter.next().unwrap();
            trace!(
                "{}-{}: Interval {}, rounded {}",
                self.config.cpu.id,
                self.config.process,
                interval,
                (interval * 1000000.0).round() as u64
            );
            thread::sleep(time::Duration::from_nanos(
                (interval * 1000000.0).round() as u64,
            ));
            trace!("{}-{}: Continue", self.config.cpu.id, self.config.process);
        }
    }
}

impl Display for SyscallsWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}

#[allow(clippy::enum_variant_names)]
#[enum_dispatch]
#[derive(Debug)]
enum SysCallerEnum {
    DummyCall,
    OpenCall,
    OpenatCall,
    SocketCall,
    ConnectCall,
    ListenCall,
    AcceptCall,
    SetuidCall,
    SetreuidCall,
    SetresuidCall,
    MmapCall,
    MountCall,
    UnlinkCall,
    UnshareCall,
    ChownCall,
    ChmodCall,
    PrctlCall,
    IoctlCall,
    CapsetCall,
}

#[enum_dispatch(SysCallerEnum)]
trait SysCaller {
    fn init(&mut self) -> Result<usize, Errno> {
        Ok(0)
    }
    fn call(&self) -> Result<usize, Errno>;
}

impl SysCallerEnum {
    fn new(syscall: Sysno, syscall_args: &ArgsMap) -> Self {
        match syscall {
            Sysno::open => Self::OpenCall(OpenCall::new(syscall_args)),
            Sysno::openat => Self::OpenatCall(OpenatCall::new(syscall_args)),
            Sysno::socket => Self::SocketCall(SocketCall::new(syscall_args)),
            Sysno::connect => Self::ConnectCall(ConnectCall::new(syscall_args)),
            Sysno::listen => Self::ListenCall(ListenCall::new(syscall_args)),
            Sysno::accept => {
                Self::AcceptCall(AcceptCall::new(syscall_args, Sysno::accept))
            }
            Sysno::accept4 => {
                // For accept4, we need to base it on accept
                Self::AcceptCall(AcceptCall::new(syscall_args, Sysno::accept4))
            }
            Sysno::setuid => Self::SetuidCall(SetuidCall::new(syscall_args)),
            Sysno::setreuid => {
                Self::SetreuidCall(SetreuidCall::new(syscall_args))
            }
            Sysno::setresuid => {
                Self::SetresuidCall(SetresuidCall::new(syscall_args))
            }
            Sysno::mmap => Self::MmapCall(MmapCall::new(syscall_args)),
            Sysno::mount => Self::MountCall(MountCall::new(syscall_args)),
            Sysno::unlink => Self::UnlinkCall(UnlinkCall::new(syscall_args)),
            Sysno::unshare => Self::UnshareCall(UnshareCall::new(syscall_args)),
            Sysno::chown => Self::ChownCall(ChownCall::new(syscall_args)),
            Sysno::chmod => Self::ChmodCall(ChmodCall::new(syscall_args)),
            Sysno::prctl => Self::PrctlCall(PrctlCall::new(syscall_args)),
            Sysno::ioctl => Self::IoctlCall(IoctlCall::new(syscall_args)),
            Sysno::capset => Self::CapsetCall(CapsetCall::new(syscall_args)),
            _ => Self::DummyCall(DummyCall::new(syscall_args, syscall)),
        }
    }
}
