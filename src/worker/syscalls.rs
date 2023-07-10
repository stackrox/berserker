use std::{thread, time};

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
        let BaseConfig {
            cpu,
            process,
            lower,
            upper,
        } = self.config;

        info!("Process {} from {}: {}-{}", process, cpu.id, lower, upper);

        let Workload::Syscalls { arrival_rate } = self.workload.workload else {unreachable!()};

        loop {
            let worker = *self;
            thread::spawn(move || {
                worker.do_syscall().unwrap();
            });

            let interval: f64 = thread_rng().sample(Exp::new(arrival_rate).unwrap());
            info!(
                "{}-{}: Interval {}, rounded {}",
                cpu.id,
                process,
                interval,
                (interval * 1000.0).round() as u64
            );
            thread::sleep(time::Duration::from_millis(
                (interval * 1000.0).round() as u64
            ));
            info!("{}-{}: Continue", cpu.id, process);
        }
    }
}
