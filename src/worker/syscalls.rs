use std::{fmt::Display, thread, time};

use core_affinity::CoreId;
use log::{info, warn};
use rand::{thread_rng, Rng};
use rand_distr::Exp;
use syscalls::{syscall, Sysno};

use crate::{Workload, WorkloadConfig};

use super::{BaseConfig, Worker};

#[derive(Debug, Copy, Clone)]
pub struct SyscallsWorker {
    config: BaseConfig,
    workload: WorkloadConfig,
}

impl SyscallsWorker {
    pub fn new(
        workload: WorkloadConfig,
        cpu: CoreId,
        process: usize,
        lower: usize,
        upper: usize,
    ) -> Self {
        SyscallsWorker {
            config: BaseConfig {
                cpu,
                process,
                lower,
                upper,
            },
            workload,
        }
    }

    fn do_syscall(&self) -> std::io::Result<()> {
        match unsafe { syscall!(Sysno::getpid) } {
            Ok(_) => Ok(()),
            Err(err) => {
                warn!("Syscall failed: {}", err);
                Ok(())
            }
        }
    }
}

impl Worker for SyscallsWorker {
    fn run_payload(&self) -> Result<(), super::WorkerError> {
        info!("{self}");

        let Workload::Syscalls { arrival_rate } = self.workload.workload else {unreachable!()};

        loop {
            let worker = *self;
            thread::spawn(move || {
                worker.do_syscall().unwrap();
            });

            let interval: f64 = thread_rng().sample(Exp::new(arrival_rate).unwrap());
            info!(
                "{}-{}: Interval {}, rounded {}",
                self.config.cpu.id,
                self.config.process,
                interval,
                (interval * 1000.0).round() as u64
            );
            thread::sleep(time::Duration::from_millis(
                (interval * 1000.0).round() as u64
            ));
            info!("{}-{}: Continue", self.config.cpu.id, self.config.process);
        }
    }
}

impl Display for SyscallsWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}
