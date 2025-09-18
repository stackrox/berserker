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
use docopt::Docopt;
use fork::{fork, Fork};
use itertools::iproduct;
use itertools::{Either, Itertools};
use nix::errno::Errno;
use nix::sys::signal::{kill, Signal};
use nix::sys::wait::waitpid;
use nix::unistd::Pid;
use serde::Deserialize;
use std::time::SystemTime;
use std::{thread, time};

use berserker::machine::apply;
use berserker::script::{ast::Node, parser::parse_instructions};
use berserker::{
    worker::new_script_worker, worker::new_worker, WorkloadConfig,
};

const USAGE: &str = "
Usage: berserker [-c CONFIG] [-f SCRIPT]

Options:
    -f, --file SCRIPT       File with instructions to execute.
                            Takes presedence over the config file.
    -c, --config CONFIG     File containing global and workload specific
                            configuration.
";

#[derive(Debug, Deserialize)]
struct Args {
    flag_c: Option<String>,
    flag_f: Option<String>,
}

fn run_script(script_path: String) -> Vec<Option<i32>> {
    let mut handles = vec![];

    let ast: Vec<Node> =
        parse_instructions(&std::fs::read_to_string(script_path).unwrap())
            .unwrap();

    let (machine, works): (Vec<_>, Vec<_>) =
        ast.iter().partition_map(|node| match node {
            Node::Work { .. } => Either::Right(node),
            Node::Machine { .. } => Either::Left(node),
        });

    if let Some(m) = machine.into_iter().next() {
        let Node::Machine { m_instructions } = m.clone() else {
            unreachable!()
        };

        for instr in m_instructions {
            debug!("INSTR {:?}", instr);
            thread::spawn(move || apply(instr.clone()));
        }
    };

    works.into_iter().for_each(|node| {
        debug!("AST NODE: {:?}", node);

        let Node::Work {
            name: _,
            args,
            instructions: _,
            dist: _,
        } = node
        else {
            unreachable!()
        };

        let workers: u32 = args
            .get("workers")
            .cloned()
            .unwrap_or(String::from("0"))
            .parse()
            .unwrap();
        let h: Vec<_> = (0..workers)
            .map(|_| {
                let worker = new_script_worker(node.clone());

                match fork() {
                    Ok(Fork::Parent(child)) => {
                        info!("Child {}", child);
                        Some(child)
                    }
                    Ok(Fork::Child) => {
                        worker.run_payload().unwrap();
                        None
                    }
                    Err(e) => {
                        warn!("Failed: {e:?}");
                        None
                    }
                }
            })
            .collect();

        handles.extend(h);
    });

    handles
}

fn run_workload(config: WorkloadConfig) -> Vec<Option<i32>> {
    let mut lower = 1024;
    let mut upper = 1024;

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
    handles
}

fn main() {
    env_logger::init();

    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    debug!("ARGS {:?}", args);

    let default_config = String::from("workload.toml");
    let duration_timer = SystemTime::now();
    let script_path = args.flag_f;
    let config_path = args.flag_c.unwrap_or(default_config);

    let config = Config::builder()
        // Add in `./Settings.toml`
        .add_source(
            config::File::with_name("/etc/berserker/workload.toml")
                .required(false),
        )
        .add_source(
            config::File::with_name(config_path.as_str()).required(false),
        )
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

    info!("Config: {:?}", config);

    let handles = match script_path {
        Some(path) => run_script(path),
        None => run_workload(config),
    };

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
                match waitpid(Pid::from_raw(*handle), None) {
                    Ok(_) => {
                        info!("{:?} stopped", *handle)
                    }
                    Err(Errno::ECHILD) => {
                        info! {"no process {:?} found", *handle}
                    }
                    Err(e) => {
                        panic! {"cannot wait for {:?}: {:?} ", *handle, e}
                    }
                };
            }
        });
    });
}
