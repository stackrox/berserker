use std::fmt::Display;

use core_affinity::CoreId;
use rand::{thread_rng, Rng};
use rand_distr::{Uniform, Zipf};

use crate::{Distribution, Workload, WorkloadConfig};

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
}

impl Display for BaseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Process {} from {}", self.process, self.cpu.id)
    }
}

pub fn new_worker(
    workload: WorkloadConfig,
    cpu: CoreId,
    process: usize,
    lower_bound: &mut usize,
    upper_bound: &mut usize,
) -> Box<dyn Worker> {
    match workload.workload {
        Workload::Processes { .. } => Box::new(ProcessesWorker::new(workload, cpu, process)),
        Workload::Endpoints { distribution } => {
            match distribution {
                Distribution::Zipfian { n_ports, exponent } => {
                    let n_ports: f64 = thread_rng().sample(Zipf::new(n_ports, exponent).unwrap());

                    *lower_bound = *upper_bound;
                    *upper_bound += n_ports as usize;
                }
                Distribution::Uniform { lower, upper } => {
                    // TODO: Double check this branch
                    let n_ports = thread_rng().sample(Uniform::new(lower, upper));

                    *lower_bound = *upper_bound;
                    *upper_bound += n_ports as usize;
                }
            }
            Box::new(EndpointWorker::new(
                workload,
                cpu,
                process,
                *lower_bound,
                *upper_bound,
            ))
        }
        Workload::Syscalls { .. } => Box::new(SyscallsWorker::new(workload, cpu, process)),
    }
}
