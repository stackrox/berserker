use std::{fmt::Display, process::Command, thread, time};

use core_affinity::CoreId;
use fork::{fork, Fork};
use log::{info, warn};
use nix::{sys::wait::waitpid, unistd::Pid};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rand_distr::Exp;

use crate::{workload, BaseConfig, WorkerError};

#[derive(Debug, Clone)]
pub struct ProcessesWorker {
    config: BaseConfig,
    workload: workload::Processes,
}

impl ProcessesWorker {
    pub fn new(
        workload: workload::Processes,
        cpu: CoreId,
        process: usize,
    ) -> Self {
        ProcessesWorker {
            config: BaseConfig { cpu, process },
            workload,
        }
    }

    fn spawn_process(&self, lifetime: u64) -> Result<(), WorkerError> {
        let BaseConfig { cpu, process } = self.config;

        if self.workload.random_process {
            let uniq_arg: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(7)
                .map(char::from)
                .collect();
            let _res = Command::new("stub").arg(uniq_arg).output().unwrap();
            Ok(())
        } else {
            match fork() {
                Ok(Fork::Parent(child)) => {
                    info!("Parent: child {}", child);
                    waitpid(Pid::from_raw(child), None).unwrap();
                    Ok(())
                }
                Ok(Fork::Child) => {
                    info!("{}-{}: Child start, {}", cpu.id, process, lifetime);
                    thread::sleep(time::Duration::from_millis(lifetime));
                    info!("{}-{}: Child stop", cpu.id, process);
                    Ok(())
                }
                Err(_) => {
                    warn!("Failed");
                    Ok(())
                }
            }
        }
    }

    pub fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        let workload::Processes {
            arrival_rate,
            departure_rate,
            random_process: _,
        } = self.workload;

        loop {
            let lifetime: f64 =
                thread_rng().sample(Exp::new(departure_rate).unwrap());

            let worker = self.clone();
            thread::spawn(move || {
                worker.spawn_process((lifetime * 1000.0).round() as u64)
            });

            let interval: f64 =
                thread_rng().sample(Exp::new(arrival_rate).unwrap());
            info!(
                "{}-{}: Interval {}, rounded {}, lifetime {}, rounded {}",
                self.config.cpu.id,
                self.config.process,
                interval,
                (interval * 1000.0).round() as u64,
                lifetime,
                (lifetime * 1000.0).round() as u64
            );
            thread::sleep(time::Duration::from_millis(
                (interval * 1000.0).round() as u64,
            ));
            info!("{}-{}: Continue", self.config.cpu.id, self.config.process);
        }
    }
}

impl Display for ProcessesWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}
