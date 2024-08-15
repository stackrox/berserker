use std::time::{Duration, Instant};
use std::{fmt::Display, thread, time};

use core_affinity::CoreId;
use log::{info, warn};
use rand::{thread_rng, Rng};
use rand_distr::Exp;
use syscalls::{syscall, Sysno};

use crate::{BaseConfig, Worker, WorkerError, Workload, WorkloadConfig};

#[derive(Debug, Copy, Clone)]
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

    fn do_syscall(&self, syscall: Sysno) -> std::io::Result<()> {
        match unsafe { syscall!(syscall) } {
            Ok(_) => Ok(()),
            Err(err) => {
                info!("Syscall failed: {}", err);
                Ok(())
            }
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
        } = self.workload.workload
        else {
            unreachable!()
        };

        let syscall = Sysno::from(syscall_nr);
        info!("Running syscall {syscall}");

        loop {
            let worker = *self;

            if start.elapsed().as_secs() > 10 {
                warn!(
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
            worker.do_syscall(syscall).unwrap();

            // If running in a tight loop, go to the next iteration
            if tight_loop {
                continue;
            }

            // Otherwise calculate waiting time
            let interval: f64 =
                thread_rng().sample(Exp::new(arrival_rate).unwrap());
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
