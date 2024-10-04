use core_affinity::CoreId;
use rand::{thread_rng, Rng};
use rand_distr::{Uniform, Zipf};

use crate::{Distribution, WorkerError, Workload, WorkloadConfig};

use self::{
    endpoints::EndpointWorker, network::NetworkWorker,
    processes::ProcessesWorker, syscalls::SyscallsWorker,
};

pub mod endpoints;
pub mod network;
pub mod processes;
pub mod syscalls;

pub enum Worker {
    Endpoint(EndpointWorker),
    Process(ProcessesWorker),
    Syscalls(SyscallsWorker),
    Network(NetworkWorker),
}

impl Worker {
    pub fn run_payload(&self) -> Result<(), WorkerError> {
        match self {
            Worker::Endpoint(e) => e.run_payload(),
            Worker::Process(p) => p.run_payload(),
            Worker::Syscalls(s) => s.run_payload(),
            Worker::Network(n) => n.run_payload(),
        }
    }

    pub fn new(
        workload: WorkloadConfig,
        cpu: CoreId,
        process: usize,
        lower_bound: &mut usize,
        upper_bound: &mut usize,
    ) -> Worker {
        match workload.workload {
            Workload::Processes { .. } => {
                Worker::Process(ProcessesWorker::new(workload, cpu, process))
            }
            Workload::Endpoints { distribution } => {
                let n_ports: usize = match distribution {
                    Distribution::Zipfian { n_ports, exponent } => thread_rng()
                        .sample(Zipf::new(n_ports, exponent).unwrap())
                        as usize,
                    Distribution::Uniform { lower, upper } => {
                        thread_rng().sample(Uniform::new(lower, upper)) as usize
                    }
                };

                *lower_bound = *upper_bound;
                *upper_bound += n_ports as usize;
                Worker::Endpoint(EndpointWorker::new(
                    workload,
                    cpu,
                    process,
                    *lower_bound,
                    *upper_bound,
                ))
            }
            Workload::Syscalls { .. } => {
                Worker::Syscalls(SyscallsWorker::new(workload, cpu, process))
            }
            Workload::Network { .. } => {
                Worker::Network(NetworkWorker::new(workload, cpu, process))
            }
        }
    }
}
