use std::fmt::Display;

use core_affinity::CoreId;

use crate::{Workload, WorkloadConfig};

use self::{endpoints::EndpointWorker, processes::ProcessesWorker, syscalls::SyscallsWorker};

pub mod endpoints;
pub mod processes;
pub mod syscalls;

#[derive(Debug)]
pub enum WorkerError {}

impl Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "worker error found")
    }
}

pub trait Worker {
    fn run_payload(&self) -> Result<(), WorkerError>;
}

#[derive(Debug, Copy, Clone)]
struct BaseConfig {
    cpu: CoreId,
    process: usize,
    lower: usize,
    upper: usize,
}

pub fn new_worker(
    workload: WorkloadConfig,
    cpu: CoreId,
    process: usize,
    lower: usize,
    upper: usize,
) -> Box<dyn Worker> {
    match workload.workload {
        Workload::Processes {
            arrival_rate: _,
            departure_rate: _,
            random_process: _,
        } => Box::new(ProcessesWorker::new(workload, cpu, process, lower, upper)),
        Workload::Endpoints { distribution: _ } => {
            Box::new(EndpointWorker::new(workload, cpu, process, lower, upper))
        }
        Workload::Syscalls { arrival_rate: _ } => {
            Box::new(SyscallsWorker::new(workload, cpu, process, lower, upper))
        }
    }
}
