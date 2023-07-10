#[macro_use]
extern crate log;
extern crate core_affinity;

use berserker::worker::new_worker;
use berserker::WorkloadConfig;
use config::Config;
use fork::{fork, Fork};
use itertools::iproduct;
use nix::sys::wait::waitpid;
use nix::unistd::Pid;
use rand::prelude::*;
use rand_distr::Uniform;
use rand_distr::Zipf;

use berserker::{Distribution, Workload};

fn main() {
    // Retrieve the IDs of all active CPU cores.
    let core_ids = core_affinity::get_core_ids().unwrap();
    let config = Config::builder()
        // Add in `./Settings.toml`
        .add_source(config::File::with_name("/etc/berserker/workload.toml").required(false))
        .add_source(config::File::with_name("workload.toml").required(false))
        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `WORKLOAD_DEBUG=1 ./target/app` would set the `debug` key
        .add_source(config::Environment::with_prefix("WORKLOAD"))
        .build()
        .unwrap()
        .try_deserialize::<WorkloadConfig>()
        .unwrap();

    let mut lower = 1024;
    let mut upper = 1024;

    env_logger::init();

    // Create processes for each active CPU core.
    let handles: Vec<_> = iproduct!(core_ids.into_iter(), 0..9)
        .map(|(cpu, process)| {
            if let Workload::Endpoints { distribution } = config.workload {
                match distribution {
                    Distribution::Zipfian { n_ports, exponent } => {
                        let n_ports: f64 =
                            thread_rng().sample(Zipf::new(n_ports, exponent).unwrap());

                        lower = upper;
                        upper += n_ports as usize;
                    }
                    Distribution::Uniform {
                        lower: config_lower,
                        upper: config_upper,
                    } => {
                        // TODO: Double check this branch
                        let n_ports = thread_rng().sample(Uniform::new(config_lower, config_upper));

                        lower = upper;
                        upper += n_ports as usize;
                    }
                }
            }

            match fork() {
                Ok(Fork::Parent(child)) => {
                    info!("Child {}", child);
                    Some(child)
                }
                Ok(Fork::Child) => {
                    if core_affinity::set_for_current(cpu) {
                        let worker = new_worker(config, cpu, process, lower, upper);

                        loop {
                            worker.run_payload().unwrap();
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
