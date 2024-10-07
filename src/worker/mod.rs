use rand::{thread_rng, Rng};
use rand_distr::{Uniform, Zipf};

use crate::{
    workload::{
        endpoints::{Distribution, Endpoints},
        Workload,
    },
    BaseConfig, WorkerError, WorkloadConfig,
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
        base_config: BaseConfig,
        start_port: u16,
    ) -> Worker {
        match workload.workload {
            Workload::Processes(processes) => {
                Worker::Process(ProcessesWorker::new(processes, base_config))
            }
            Workload::Endpoints(Endpoints {
                restart_interval,
                distribution,
            }) => {
                let n_ports: u16 = match distribution {
                    Distribution::Zipfian { n_ports, exponent } => thread_rng()
                        .sample(Zipf::new(n_ports, exponent).unwrap())
                        as u16,
                    Distribution::Uniform { lower, upper } => {
                        thread_rng().sample(Uniform::new(lower, upper)) as u16
                    }
                };

                Worker::Endpoint(EndpointWorker::new(
                    base_config,
                    restart_interval,
                    start_port,
                    n_ports,
                ))
            }
            Workload::Syscalls(syscalls) => {
                Worker::Syscalls(SyscallsWorker::new(syscalls, base_config))
            }
            Workload::Network(network) => {
                Worker::Network(NetworkWorker::new(network, base_config))
            }
        }
    }
}
