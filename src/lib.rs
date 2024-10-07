use core_affinity::CoreId;
use fork::{fork, Fork};
use itertools::iproduct;
use log::{info, warn};
use nix::{
    sys::{
        signal::{kill, Signal},
        wait::waitpid,
    },
    unistd::Pid,
};
use std::{fmt::Display, thread};
use workload::WorkloadConfig;

use crate::worker::Worker;

pub mod worker;
pub mod workload;

#[derive(Debug)]
pub enum WorkerError {
    Internal,
}

impl Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "worker error found")
    }
}

/// General information for each worker, on which CPU is it running
/// and what is the process number.
#[derive(Debug, Copy, Clone)]
struct BaseConfig {
    cpu: CoreId,
    process: usize,
}

impl Display for BaseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Process {} from {}", self.process, self.cpu.id)
    }
}

pub fn run(config: WorkloadConfig) {
    let duration_timer = std::time::SystemTime::now();
    let mut lower = 1024;
    let mut upper = 1024;

    let core_ids: Vec<CoreId> = if config.per_core {
        // Retrieve the IDs of all active CPU cores.
        core_affinity::get_core_ids().unwrap()
    } else {
        vec![CoreId { id: 0 }]
    };

    let handles: Vec<_> = iproduct!(core_ids.into_iter(), 0..config.workers)
        .map(|(cpu, process)| {
            let worker = Worker::new(
                config.clone(),
                cpu,
                process,
                &mut lower,
                &mut upper,
            );

            match fork() {
                Ok(Fork::Parent(child)) => {
                    info!("Child {}", child);
                    Some(child)
                }
                Ok(Fork::Child) => {
                    if config.per_core {
                        core_affinity::set_for_current(cpu);
                    }

                    loop {
                        worker.run_payload().unwrap();
                    }
                }
                Err(e) => {
                    warn!("Failed: {e:?}");
                    None
                }
            }
        })
        .collect();

    info!("In total: {}", upper);

    let processes = &handles.clone();

    thread::scope(|s| {
        if config.duration != 0 {
            // Spin a watcher thread
            s.spawn(move || loop {
                thread::sleep(std::time::Duration::from_secs(1));
                let elapsed = duration_timer.elapsed().unwrap().as_secs();

                if elapsed > config.duration {
                    for handle in processes.iter().flatten() {
                        info!("Terminating: {}", *handle);
                        match kill(Pid::from_raw(*handle), Signal::SIGTERM) {
                            Ok(()) => {
                                continue;
                            }
                            Err(_) => {
                                continue;
                            }
                        }
                    }

                    break;
                }
            });
        }

        s.spawn(move || {
            for handle in processes.iter().flatten() {
                info!("waitpid: {}", *handle);
                waitpid(Pid::from_raw(*handle), None).unwrap();
            }
        });
    });
}
