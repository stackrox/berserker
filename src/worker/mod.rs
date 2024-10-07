use core_affinity::CoreId;
use rand::{thread_rng, Rng};
use rand_distr::{Uniform, Zipf};

use crate::{
    workload::{Distribution, Endpoints, Workload},
    WorkerError, WorkloadConfig,
};

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
        start_port: u16,
    ) -> Worker {
        match workload.workload {
            Workload::Processes(processes) => {
                Worker::Process(ProcessesWorker::new(processes, cpu, process))
            }
            Workload::Endpoints(Endpoints { distribution }) => {
                let n_ports: u16 = match distribution {
                    Distribution::Zipfian { n_ports, exponent } => thread_rng()
                        .sample(Zipf::new(n_ports, exponent).unwrap())
                        as u16,
                    Distribution::Uniform { lower, upper } => {
                        thread_rng().sample(Uniform::new(lower, upper)) as u16
                    }
                };

                Worker::Endpoint(EndpointWorker::new(
                    workload, cpu, process, start_port, n_ports,
                ))
            }
            Workload::Syscalls(syscalls) => {
                Worker::Syscalls(SyscallsWorker::new(syscalls, cpu, process))
            }
            Workload::Network(network) => {
                Worker::Network(NetworkWorker::new(network, cpu, process))
            }
        }
    }
}
