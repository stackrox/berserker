//! Berserker workload generator.
//!
//! The implementation is covering two part:
//! * workload independent logic
//! * workload specific details
//!
//! Those have to be isolated as much as possible, and working together via
//! configuration data structures and worker interface.
//!
//! The execution contains following steps:
//! * Consume provided configuration
//! * For each available CPU core spawn specified number of worker processes
//! * Invoke a workload-specific logic via run_payload
//! * Wait for all the workers to finish

#[macro_use]
extern crate log;
extern crate core_affinity;

use config::Config;
use core_affinity::CoreId;
use fork::{fork, Fork};
use itertools::iproduct;
use nix::sys::signal::{kill, Signal};
use nix::sys::wait::waitpid;
use nix::unistd::Pid;
use std::time::SystemTime;
use std::{env, thread, time};

use berserker::{worker::new_worker, WorkloadConfig};

fn main() {
    let args: Vec<String> = env::args().collect();
    let default_config = String::from("workload.toml");
    let config_path = &args.get(1).unwrap_or(&default_config);
    let duration_timer = SystemTime::now();

    let config = Config::builder()
        // Add in `./Settings.toml`
        .add_source(
            config::File::with_name("/etc/berserker/workload.toml")
                .required(false),
        )
        .add_source(config::File::with_name(config_path).required(false))
        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `BERSERKER__WORKLOAD__ARRIVAL_RATE=1` would set the `arrival_rate` key
        .add_source(
            config::Environment::with_prefix("BERSERKER")
                .try_parsing(true)
                .separator("__"),
        )
        .build()
        .unwrap()
        .try_deserialize::<WorkloadConfig>()
        .unwrap();

    let mut lower = 1024;
    let mut upper = 1024;

    env_logger::init();

    info!("Config: {:?}", config);

    let core_ids: Vec<CoreId> = if config.per_core {
        // Retrieve the IDs of all active CPU cores.
        core_affinity::get_core_ids().unwrap()
    } else {
        vec![CoreId { id: 0 }]
    };

    let handles: Vec<_> = iproduct!(core_ids.into_iter(), 0..config.workers)
        .map(|(cpu, process)| {
            let worker =
                new_worker(config, cpu, process, &mut lower, &mut upper);

            match fork() {
                Ok(Fork::Parent(child)) => {
                    info!("Child {}", child);
                    Some(child)
                }
                Ok(Fork::Child) => {
                    if config.per_core {
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

    info!("In total: {}", upper);

    let processes = &handles.clone();

    thread::scope(|s| {
        if config.duration != 0 {
            // Spin a watcher thread
            s.spawn(move || loop {
                thread::sleep(time::Duration::from_secs(1));
                let elapsed = duration_timer.elapsed().unwrap().as_secs();

                if elapsed > config.duration {
                    for handle in processes.iter().flatten() {
                        info!("Terminating: {}", *handle);
                        let _ = kill(Pid::from_raw(*handle), Signal::SIGTERM);
                    }

                    break;
                }
            });
        }

        s.spawn(move || {
            for handle in processes.iter().flatten() {
                info!("waitpid: {}", *handle);
                waitpid(Pid::from_raw(*handle), None).unwrap();
            }
        });
    });
}
