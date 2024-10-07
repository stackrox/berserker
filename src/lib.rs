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
use std::{fmt::Display, sync::Arc, thread};
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
pub struct BaseConfig {
    cpu: CoreId,
    process: usize,
}

impl Display for BaseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Process {} from {}", self.process, self.cpu.id)
    }
}

pub fn run(workload: WorkloadConfig) {
    let duration_timer = std::time::SystemTime::now();
    let mut start_port = 1024;
    let mut total_ports = 0;

    let core_ids: Vec<CoreId> = if workload.per_core {
        // Retrieve the IDs of all active CPU cores.
        core_affinity::get_core_ids().unwrap()
    } else {
        vec![CoreId { id: 0 }]
    };

    let handles: Vec<_> = iproduct!(core_ids.into_iter(), 0..workload.workers)
        .map(|(cpu, process)| {
            let config = BaseConfig { cpu, process };
            let worker = Worker::new(workload.clone(), config, start_port);

            if let Worker::Endpoint(w) = &worker {
                start_port += w.size();
                total_ports += w.size();
            }

            match fork() {
                Ok(Fork::Parent(child)) => {
                    info!("Child {}", child);
                    Some(child)
                }
                Ok(Fork::Child) => {
                    if workload.per_core {
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

    if total_ports != 0 {
        info!("In total: {total_ports}");
    }

    let handles = Arc::new(handles);

    thread::scope(|s| {
        if workload.duration != 0 {
            // Cloning the Arc so we can hand it over to the watcher thread
            let handles = handles.clone();

            // Spin a watcher thread
            s.spawn(move || loop {
                thread::sleep(std::time::Duration::from_secs(1));
                let elapsed = duration_timer.elapsed().unwrap().as_secs();

                if elapsed > workload.duration {
                    for handle in handles.iter().flatten() {
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
            for handle in handles.iter().flatten() {
                info!("waitpid: {}", *handle);
                waitpid(Pid::from_raw(*handle), None).unwrap();
            }
        });
    });
}
