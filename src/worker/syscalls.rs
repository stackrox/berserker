use std::{fmt::Display, thread, time};

use log::{info, warn};
use rand::{thread_rng, Rng};
use rand_distr::Exp;
use syscalls::{syscall, Sysno};

use crate::{workload, BaseConfig, WorkerError};

#[derive(Debug, Copy, Clone)]
pub struct SyscallsWorker {
    config: BaseConfig,
    arrival_rate: f64,
}

impl SyscallsWorker {
    pub fn new(workload: workload::Syscalls, config: BaseConfig) -> Self {
        SyscallsWorker {
            config,
            arrival_rate: workload.arrival_rate,
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

    pub fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        loop {
            let worker = *self;
            thread::spawn(move || {
                worker.do_syscall().unwrap();
            });

            let interval: f64 =
                thread_rng().sample(Exp::new(self.arrival_rate).unwrap());
            info!(
                "{}-{}: Interval {}, rounded {}",
                self.config.cpu.id,
                self.config.process,
                interval,
                (interval * 1000.0).round() as u64
            );
            thread::sleep(time::Duration::from_millis(
                (interval * 1000.0).round() as u64,
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
