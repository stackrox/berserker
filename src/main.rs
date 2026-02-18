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
use fork::{Fork, fork};
use itertools::iproduct;
use itertools::{Either, Itertools};
use nix::errno::Errno;
use nix::sys::signal::{Signal, kill};
use nix::sys::wait::waitpid;
use nix::unistd::Pid;
use serde::Deserialize;
use std::time::SystemTime;
use std::{thread, time};

use berserker::{
    WorkloadConfig,
    worker::{new_script_worker, new_worker},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use berserker::script::{
    ast::Node, parser::parse_instructions, rules::apply_rules,
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

fn run_script(script_path: String) -> Vec<(i32, u64)> {
    let mut handles = vec![];
    info!("Loading script: {:?}", script_path);

    let ast: Vec<Node> =
        parse_instructions(&std::fs::read_to_string(script_path).unwrap())
            .unwrap();

    let (machine, works): (Vec<_>, Vec<_>) =
        ast.iter().partition_map(|node| match node {
            Node::Work { .. } => Either::Right(node),
            Node::Machine { .. } => Either::Left(node),
        });

    let works = apply_rules(works);

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

        let duration: u64 = args
            .get("duration")
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
                        Some((child, duration))
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

    handles.iter().filter_map(|i| *i).collect()
}

fn run_workload(config: WorkloadConfig) -> Vec<(i32, u64)> {
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
            let worker = new_worker(
                config.clone(),
                cpu,
                process,
                &mut lower,
                &mut upper,
            );

            match fork() {
                Ok(Fork::Parent(child)) => {
                    info!("Child {}", child);
                    Some((child, config.duration))
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
    handles.iter().filter_map(|i| *i).collect()
}

fn main() {
    env_logger::init();

    let terminating = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(
        signal_hook::consts::SIGTERM,
        Arc::clone(&terminating),
    )
    .unwrap();

    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    debug!("ARGS {:?}", args);

    let duration_timer = SystemTime::now();
    let script_path = args.flag_f;

    let handles = match script_path {
        Some(path) => run_script(path),
        None => {
            let default_config = String::from("workload.toml");
            let config_path = args.flag_c.unwrap_or(default_config);

            let config = Config::builder()
                // Add in `./Settings.toml`
                .add_source(
                    config::File::with_name("/etc/berserker/workload.toml")
                        .required(false),
                )
                .add_source(
                    config::File::with_name(config_path.as_str())
                        .required(false),
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
            run_workload(config)
        }
    };

    let processes = &handles.clone();

    thread::scope(|s| {
        // Spin a watcher thread
        s.spawn(move || {
            loop {
                thread::sleep(time::Duration::from_secs(1));
                let elapsed = duration_timer.elapsed().unwrap().as_secs();

                // Ignore processes without specified duration -- we don't want
                // neither terminate them, nor count against processes to compare.
                let watched_processes = processes
                    .iter()
                    .filter(|(_, duration)| *duration > 0)
                    .collect::<Vec<_>>();

                // Find all processes with expired duration. If we've received
                // SIGTERM, get all processes.
                let expired = watched_processes
                    .iter()
                    .filter(|(_, duration)| {
                        *duration < elapsed
                            || terminating.load(Ordering::Relaxed)
                    })
                    .collect::<Vec<_>>();

                for (handle, _) in &expired {
                    info!("Terminating: {}", *handle);
                    let _ = kill(Pid::from_raw(*handle), Signal::SIGKILL);
                }

                if expired.len() == watched_processes.len() {
                    break;
                }
            }
        });

        s.spawn(move || {
            for (handle, _) in handles {
                info!("waitpid: {}", handle);
                match waitpid(Pid::from_raw(handle), None) {
                    Ok(_) => {
                        info!("{handle:?} stopped")
                    }
                    Err(Errno::ECHILD) => {
                        info!("no process {handle:?} found")
                    }
                    Err(e) => {
                        panic!("cannot wait for {handle:?}: {e:?}")
                    }
                };
            }
        });
    });
}
