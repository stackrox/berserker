mod openat;
mod openat2;
mod statx;
mod unlinkat;

use std::{fmt::Display, time::Instant};

use core_affinity::CoreId;
use enum_dispatch::enum_dispatch;
use log::{debug, info, trace};
use rand::{Rng, thread_rng};
use rand_distr::Exp;
use syscalls::Errno;

use crate::{
    ArgsMap, BaseConfig, Worker, WorkerError, Workload, WorkloadConfig,
    worker::io_uring::{
        openat::OpenatIOUringCall, openat2::Openat2IOUringCall,
        statx::StatxIOUringCall, unlinkat::UnlinkatIOUringCall,
    },
};

#[derive(Debug, Clone)]
pub struct IOUringWorker {
    config: BaseConfig,
    workload: WorkloadConfig,
}

impl IOUringWorker {
    pub fn new(workload: WorkloadConfig, cpu: CoreId, process: usize) -> Self {
        IOUringWorker {
            config: BaseConfig { cpu, process },
            workload,
        }
    }
}

impl Worker for IOUringWorker {
    fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        let mut counter = 0;
        let mut start = Instant::now();

        let Workload::IOUring {
            arrival_rate,
            tight_loop,
            iouring_nr,
            iouring_args,
        } = &self.workload.workload
        else {
            unreachable!()
        };

        let mut caller = new_iouring_generator(*iouring_nr, iouring_args)?;
        if let Err(e) = caller.init() {
            return Err(WorkerError::InternalWithMessage(format!(
                "Error initializing iouring: {:?}",
                e
            )));
        };
        let mut ring = io_uring::IoUring::new(1).unwrap();

        let exp = Exp::new(*arrival_rate).unwrap();
        let rng = thread_rng();
        let mut rng_iter = rng.sample_iter(exp);

        info!("Running iouring {iouring_nr}");

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
            // Do the iouring directly, without spawning a thread (it would
            // introduce too much overhead for a quick iouring).
            match caller.submit(&mut ring) {
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
            std::thread::sleep(std::time::Duration::from_nanos(
                (interval * 1000000.0).round() as u64,
            ));
            trace!("{}-{}: Continue", self.config.cpu.id, self.config.process);
        }
    }
}

impl Display for IOUringWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}

#[allow(clippy::enum_variant_names)]
#[enum_dispatch]
#[derive(Debug)]
enum IOUringCallerEnum {
    OpenatIOUringCall,
    Openat2IOUringCall,
    StatxIOUringCall,
    UnlinkatIOUringCall,
}

#[enum_dispatch(IOUringCallerEnum)]
trait IOUringCaller {
    fn init(&mut self) -> std::io::Result<usize> {
        Ok(0)
    }
    fn submit(&self, ring: &mut io_uring::IoUring) -> Result<usize, Errno>;
}

fn new_iouring_generator(
    iouring_nr: u8,
    iouring_args: &ArgsMap,
) -> Result<IOUringCallerEnum, WorkerError> {
    use io_uring::opcode::*;
    match iouring_nr {
        OpenAt::CODE => Ok(IOUringCallerEnum::OpenatIOUringCall(
            OpenatIOUringCall::new(iouring_args),
        )),
        OpenAt2::CODE => Ok(IOUringCallerEnum::Openat2IOUringCall(
            Openat2IOUringCall::new(iouring_args),
        )),
        Statx::CODE => Ok(IOUringCallerEnum::StatxIOUringCall(
            StatxIOUringCall::new(iouring_args),
        )),
        UnlinkAt::CODE => Ok(IOUringCallerEnum::UnlinkatIOUringCall(
            UnlinkatIOUringCall::new(iouring_args),
        )),
        _ => Err(WorkerError::InternalWithMessage(
            "Unsupported iouring number".to_string(),
        )),
    }
}
