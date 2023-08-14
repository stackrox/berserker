#[macro_use]
extern crate log;
extern crate core_affinity;

use config::Config;
use fork::{fork, Fork};
use itertools::iproduct;
use nix::sys::wait::waitpid;
use nix::unistd::Pid;

use berserker::{worker::new_worker, WorkloadConfig};

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
            let worker = new_worker(config, cpu, process, &mut lower, &mut upper);

            match fork() {
                Ok(Fork::Parent(child)) => {
                    info!("Child {}", child);
                    Some(child)
                }
                Ok(Fork::Child) => {
                    if core_affinity::set_for_current(cpu) {
                        loop {
                            worker.run_payload().unwrap();
                        }
                    }

                    None
                }
                Err(e) => {
                    warn!("Failed: {e:?}");
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
