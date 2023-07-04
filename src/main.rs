#[macro_use]
extern crate log;
extern crate core_affinity;

use fork::{fork, Fork};
use std::{thread, time};
use std::process::Command;
use std::net::{TcpListener};
use core_affinity::CoreId;
use itertools::iproduct;
use nix::sys::wait::waitpid;
use nix::unistd::Pid;
use config::Config;
use std::collections::HashMap;
use syscalls::{Sysno, syscall};

use rand::prelude::*;
use rand::{distributions::Alphanumeric, Rng};
use rand_distr::Zipf;
use rand_distr::Uniform;
use rand_distr::Exp;

#[derive(Debug, Copy, Clone)]
enum Distribution {
    Zipfian,
    Uniform,
}

#[derive(Debug, Copy, Clone)]
enum Workload {
    Endpoints,
    Processes,
    Syscalls,
}

#[derive(Debug, Copy, Clone)]
struct WorkloadConfig {
    restart_interval: u64,
    endpoints_dist: Distribution,
    workload: Workload,
    zipf_exponent: f64,
    n_ports: u64,
    uniform_lower: u64,
    uniform_upper: u64,
    arrival_rate: f64,
    departure_rate: f64,
    random_process: bool,
}

#[derive(Debug, Copy, Clone)]
struct WorkerConfig {
    workload: WorkloadConfig,
    cpu: CoreId,
    process: usize,
    lower: usize,
    upper: usize,
}

fn listen(port: usize, sleep: u64) -> std::io::Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(addr)?;

    let _res = listener.incoming();

    thread::sleep(time::Duration::from_secs(sleep));
    Ok(())
}

fn spawn_process(config: WorkerConfig, lifetime: u64) -> std::io::Result<()> {
    if config.workload.random_process {
        let uniq_arg: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();
        let _res = Command::new("stub").arg(uniq_arg).output().unwrap();
        //info!("Command output: {}",  String::from_utf8(res.stdout).unwrap());
        Ok(())
    } else {
        match fork() {
           Ok(Fork::Parent(child)) => {
               info!("Parent: child {}", child);
               waitpid(Pid::from_raw(child), None);
               Ok(())
           },
           Ok(Fork::Child) => {
               info!("{}-{}: Child start, {}", config.cpu.id, config.process, lifetime);
               thread::sleep(time::Duration::from_millis(lifetime));
               info!("{}-{}: Child stop", config.cpu.id, config.process);
               Ok(())
           },
           Err(_) => {
               warn!("Failed");
               Ok(())
          },
        }
    }
}

// Spawn processes with a specified rate
fn process_payload(config: WorkerConfig) -> std::io::Result<()> {
    info!("Process {} from {}: {}-{}",
             config.process, config.cpu.id, config.lower, config.upper);

    loop {
        let lifetime: f64 = thread_rng().sample(Exp::new(config.workload.departure_rate).unwrap());

        thread::spawn(move || {
            spawn_process(config, (lifetime * 1000.0).round() as u64)
        });

        let interval: f64 = thread_rng().sample(Exp::new(config.workload.arrival_rate).unwrap());
        info!("{}-{}: Interval {}, rounded {}, lifetime {}, rounded {}",
              config.cpu.id, config.process,
              interval, (interval * 1000.0).round() as u64,
              lifetime, (lifetime * 1000.0).round() as u64);
        thread::sleep(time::Duration::from_millis((interval * 1000.0).round() as u64));
        info!("{}-{}: Continue", config.cpu.id, config.process);
    }
}

fn listen_payload(config: WorkerConfig) -> std::io::Result<()> {
    info!("Process {} from {}: {}-{}",
             config.process, config.cpu.id, config.lower, config.upper);

    let listeners: Vec<_> = (config.lower..config.upper).map(|port| {
        thread::spawn(move || {
            listen(port, config.workload.restart_interval)
        })
    }).collect();

    for listener in listeners {
        let _res = listener.join().unwrap();
    }

    Ok(())
}

fn do_syscall(config: WorkerConfig) -> std::io::Result<()> {
    match unsafe { syscall!(Sysno::getpid) } {
        Ok(_) => {
            Ok(())
        }
        Err(err) => {
            warn!("Syscall failed: {}", err);
            Ok(())
        }
    }
}

fn syscalls_payload(config: WorkerConfig) -> std::io::Result<()> {
    info!("Process {} from {}: {}-{}",
             config.process, config.cpu.id, config.lower, config.upper);

    loop {
        thread::spawn(move || {
            do_syscall(config)
        });

        let interval: f64 = thread_rng().sample(Exp::new(config.workload.arrival_rate).unwrap());
        info!("{}-{}: Interval {}, rounded {}",
              config.cpu.id, config.process,
              interval, (interval * 1000.0).round() as u64);
        thread::sleep(time::Duration::from_millis((interval * 1000.0).round() as u64));
        info!("{}-{}: Continue", config.cpu.id, config.process);
    }
}

fn main() {
    // Retrieve the IDs of all active CPU cores.
    let core_ids = core_affinity::get_core_ids().unwrap();
    let settings = Config::builder()
        // Add in `./Settings.toml`
        .add_source(config::File::with_name("/etc/berserker/workload.toml")
                    .required(false))
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
        _           => Workload::Endpoints,
    };

    let endpoints_dist = match settings["endpoints_distribution"].as_str() {
        "zipf"      => Distribution::Zipfian,
        "uniform"   => Distribution::Uniform,
        _           => Distribution::Zipfian,
    };

    let config: WorkloadConfig = WorkloadConfig{
        restart_interval: settings["restart_interval"].parse::<u64>().unwrap(),
        endpoints_dist: endpoints_dist,
        workload: workload,
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
                let n_ports: f64 = thread_rng().sample(Zipf::new(config.n_ports, config.zipf_exponent).unwrap());

                lower = upper;
                upper += n_ports as usize;
            },
            Distribution::Uniform => {
                let n_ports = thread_rng().sample(Uniform::new(config.uniform_lower, config.uniform_upper));

                lower = upper;
                upper += n_ports as usize;
            }
        }

        match fork() {
           Ok(Fork::Parent(child)) => {info!("Child {}", child); Some(child)},
           Ok(Fork::Child) => {
                if core_affinity::set_for_current(cpu) {
                    let worker_config: WorkerConfig = WorkerConfig{
                        workload: config,
                        cpu: cpu,
                        process: process,
                        lower: lower,
                        upper: upper,
                    };

                    loop {
                        let _res = match config.workload {
                            Workload::Endpoints => listen_payload(worker_config),
                            Workload::Processes => process_payload(worker_config),
                            Workload::Syscalls => syscalls_payload(worker_config),
                        };
                    }
                }

                None
           },
           Err(_) => {warn!("Failed"); None},
        }
    }).collect();

    info!("In total: {}", upper);

    for handle in handles.into_iter().filter_map(|pid| pid) {
        info!("waitpid: {}", handle);
        waitpid(Pid::from_raw(handle), None);
    }
}
