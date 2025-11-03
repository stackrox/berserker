use core_affinity::CoreId;
use rand::{thread_rng, Rng};
use rand_distr::{Uniform, Zipf};

use crate::{Distribution, Worker, Workload, WorkloadConfig};

use self::{
    bpf::BpfWorker, endpoints::EndpointWorker, network::NetworkWorker,
    processes::ProcessesWorker, script::ScriptWorker, syscalls::SyscallsWorker,
};

use crate::script::ast::Node;

pub mod bpf;
pub mod endpoints;
pub mod network;
pub mod processes;
pub mod script;
pub mod syscalls;

pub fn new_worker(
    workload: WorkloadConfig,
    cpu: CoreId,
    process: usize,
    lower_bound: &mut usize,
    upper_bound: &mut usize,
) -> Box<dyn Worker> {
    match workload.workload {
        Workload::Processes { .. } => {
            Box::new(ProcessesWorker::new(workload, cpu, process))
        }
        Workload::Endpoints { distribution } => {
            match distribution {
                Distribution::Zipfian { n_ports, exponent } => {
                    let n_ports: f64 = thread_rng()
                        .sample(Zipf::new(n_ports, exponent).unwrap());

                    *lower_bound = *upper_bound;
                    *upper_bound += n_ports as usize;
                }
                Distribution::Uniform { lower, upper } => {
                    // TODO: Double check this branch
                    let n_ports =
                        thread_rng().sample(Uniform::new(lower, upper));

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
        Workload::Syscalls { .. } => {
            Box::new(SyscallsWorker::new(workload, cpu, process))
        }
        Workload::Network { .. } => {
            Box::new(NetworkWorker::new(workload, cpu, process))
        }
        Workload::Bpf { .. } => {
            Box::new(BpfWorker::new(workload, cpu, process))
        }
    }
}

pub fn new_script_worker(node: Node) -> Box<dyn Worker> {
    Box::new(ScriptWorker::new(node))
}
