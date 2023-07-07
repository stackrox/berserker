#[macro_use]
extern crate log;
extern crate core_affinity;

use std::collections::HashMap;

use berserker::WorkloadConfig;
use config::Config;
use fork::{fork, Fork};
use itertools::iproduct;
use nix::sys::wait::waitpid;
use nix::unistd::Pid;
use rand::prelude::*;
use rand_distr::Uniform;
use rand_distr::Zipf;

use berserker::{worker::WorkerConfig, Distribution, Workload};

fn main() {
    // Retrieve the IDs of all active CPU cores.
    let core_ids = core_affinity::get_core_ids().unwrap();
    let settings = Config::builder()
        // Add in `./Settings.toml`
        .add_source(config::File::with_name("/etc/berserker/workload.toml").required(false))
        .add_source(config::File::with_name("workload.toml").required(false))
        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `WORKLOAD_DEBUG=1 ./target/app` would set the `debug` key
        .add_source(config::Environment::with_prefix("WORKLOAD"))
        .build()
        .unwrap()
        .try_deserialize::<HashMap<String, String>>()
        .unwrap();

    let mut lower = 1024;
    let mut upper = 1024;

    env_logger::init();

    let workload = match settings["workload"].as_str() {
        "endpoints" => Workload::Endpoints,
        "processes" => Workload::Processes,
        "syscalls" => Workload::Syscalls,
        _ => Workload::Endpoints,
    };

    let endpoints_dist = match settings["endpoints_distribution"].as_str() {
        "zipf" => Distribution::Zipfian,
        "uniform" => Distribution::Uniform,
        _ => Distribution::Zipfian,
    };

    let config = WorkloadConfig {
        restart_interval: settings["restart_interval"].parse::<u64>().unwrap(),
        endpoints_dist,
        workload,
        zipf_exponent: settings["zipf_exponent"].parse::<f64>().unwrap(),
        n_ports: settings["n_ports"].parse::<u64>().unwrap(),
        arrival_rate: settings["arrival_rate"].parse::<f64>().unwrap(),
        departure_rate: settings["departure_rate"].parse::<f64>().unwrap(),
        uniform_lower: settings["uniform_lower"].parse::<u64>().unwrap(),
        uniform_upper: settings["uniform_upper"].parse::<u64>().unwrap(),
        random_process: settings["random_process"].parse::<bool>().unwrap(),
    };

    // Create processes for each active CPU core.
    let handles: Vec<_> = iproduct!(core_ids.into_iter(), 0..9)
        .map(|(cpu, process)| {
            match config.endpoints_dist {
                Distribution::Zipfian => {
                    let n_ports: f64 = thread_rng()
                        .sample(Zipf::new(config.n_ports, config.zipf_exponent).unwrap());

                    lower = upper;
                    upper += n_ports as usize;
                }
                Distribution::Uniform => {
                    let n_ports = thread_rng()
                        .sample(Uniform::new(config.uniform_lower, config.uniform_upper));

                    lower = upper;
                    upper += n_ports as usize;
                }
            }

            match fork() {
                Ok(Fork::Parent(child)) => {
                    info!("Child {}", child);
                    Some(child)
                }
                Ok(Fork::Child) => {
                    if core_affinity::set_for_current(cpu) {
                        let worker_config = WorkerConfig::new(config, cpu, process, lower, upper);

                        loop {
                            let _res = match config.workload {
                                Workload::Endpoints => worker_config.listen_payload(),
                                Workload::Processes => worker_config.process_payload(),
                                Workload::Syscalls => worker_config.syscalls_payload(),
                            };
                        }
                    }

                    None
                }
                Err(_) => {
                    warn!("Failed");
                    None
                }
            }
        })
        .collect();

    info!("In total: {}", upper);

    for handle in handles.into_iter().flatten() {
        info!("waitpid: {}", handle);
        waitpid(Pid::from_raw(handle), None).unwrap();
    }
}
