use core_affinity::CoreId;
use fork::{fork, Fork};
use itertools::iproduct;
use log::{info, warn};
use nix::{
    sys::{
        signal::{kill, Signal},
        wait::waitpid,
    },
    unistd::Pid,
};
use serde::Deserialize;
use std::{fmt::Display, thread};

use crate::worker::Worker;

pub mod worker;

/// Main workload configuration, contains general bits for all types of
/// workloads plus workload specific data.
#[derive(Debug, Copy, Clone, Deserialize)]
pub struct WorkloadConfig {
    /// An amount of time for workload payload to run before restarting.
    pub restart_interval: u64,

    /// Controls per-core mode to handle number of workers. If per-core mode
    /// is enabled, `workers` will be treated as a number of workers per CPU
    /// core. Otherwise it will be treated as a total number of workers.
    #[serde(default = "default_per_core")]
    pub per_core: bool,

    /// How many workers to spin, depending on `per_core` in either per-core
    /// or total mode.
    #[serde(default = "default_workers")]
    pub workers: usize,

    /// Custom workload configuration.
    pub workload: Workload,

    /// For how long to run the worker. Default value is zero, meaning no limit.
    #[serde(default = "default_duration")]
    pub duration: u64,
}

fn default_workers() -> usize {
    1
}

fn default_per_core() -> bool {
    true
}

fn default_duration() -> u64 {
    0
}

/// Workload specific configuration, contains one enum value for each
/// workload type.
#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum Workload {
    /// How to listen on ports.
    Endpoints {
        /// Governing the number of ports open.
        #[serde(flatten)]
        distribution: Distribution,
    },

    /// How to spawn processes.
    Processes {
        /// How often a new process will be spawn.
        arrival_rate: f64,

        /// How long processes are going to live.
        departure_rate: f64,

        /// Spawn a new process with random arguments.
        random_process: bool,
    },

    /// How to invoke syscalls
    Syscalls {
        /// How often to invoke a syscall.
        arrival_rate: f64,
    },

    /// How to open network connections
    Network {
        /// Whether the instance functions as a server or client
        server: bool,

        /// Which ip address to use for the server to listen on,
        /// or for the client to connect to
        address: (u8, u8, u8, u8),

        /// Port for the server to listen on, or for the client
        /// to connect to.
        target_port: u16,

        /// Rate of opening new connections
        arrival_rate: f64,

        /// Rate of closing connections
        departure_rate: f64,

        /// Starting number of connections
        nconnections: u32,

        /// How often send data via new connections, in milliseconds.
        /// The interval is applied for all connections, e.g. an interval
        /// of 100 ms for 100 connections means that every 100 ms one out
        /// of 100 connections will be allowed to send some data.
        /// This parameter allows to control the overhead of sending data,
        /// so that it will not impact connections monitoring.
        #[serde(default = "default_network_send_interval")]
        send_interval: u128,
    },
}

fn default_network_send_interval() -> u128 {
    100
}

/// Distribution for number of ports to listen on
#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(tag = "distribution")]
pub enum Distribution {
    /// Few processes are opening large number of ports, the rest are only few.
    #[serde(alias = "zipf")]
    Zipfian { n_ports: u64, exponent: f64 },

    /// Every process opens more or less the same number of ports.
    #[serde(alias = "uniform")]
    Uniform { lower: u64, upper: u64 },
}

#[derive(Debug)]
pub enum WorkerError {
    Internal,
}

impl Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "worker error found")
    }
}

/// General information for each worker, on which CPU is it running
/// and what is the process number.
#[derive(Debug, Copy, Clone)]
struct BaseConfig {
    cpu: CoreId,
    process: usize,
}

impl Display for BaseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Process {} from {}", self.process, self.cpu.id)
    }
}

pub fn run(config: WorkloadConfig) {
    let duration_timer = std::time::SystemTime::now();
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
                Worker::new(config, cpu, process, &mut lower, &mut upper);

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
                thread::sleep(std::time::Duration::from_secs(1));
                let elapsed = duration_timer.elapsed().unwrap().as_secs();

                if elapsed > config.duration {
                    for handle in processes.iter().flatten() {
                        info!("Terminating: {}", *handle);
                        match kill(Pid::from_raw(*handle), Signal::SIGTERM) {
                            Ok(()) => {
                                continue;
                            }
                            Err(_) => {
                                continue;
                            }
                        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use config::{Config, File, FileFormat};

    #[test]
    fn test_processes() {
        let input = r#"
            restart_interval = 10

            [workload]
            type = "processes"
            arrival_rate = 10.0
            departure_rate = 200.0
            random_process = true
        "#;

        let config = Config::builder()
            .add_source(File::from_str(input, FileFormat::Toml))
            .build()
            .expect("failed to parse configuration")
            .try_deserialize::<WorkloadConfig>()
            .expect("failed to deserialize into WorkloadConfig");

        let WorkloadConfig {
            restart_interval,
            workload,
            ..
        } = config;
        assert_eq!(restart_interval, 10);
        if let Workload::Processes {
            arrival_rate,
            departure_rate,
            random_process,
        } = workload
        {
            assert_eq!(arrival_rate, 10.0);
            assert_eq!(departure_rate, 200.0);
            assert!(random_process);
        } else {
            panic!("wrong workload type found");
        }
    }

    #[test]
    fn test_endpoints_zipf() {
        let input = r#"
            restart_interval = 10

            [workload]
            type = "endpoints"
            distribution = "zipf"
            n_ports = 200
            exponent = 1.4
        "#;

        let config = Config::builder()
            .add_source(File::from_str(input, FileFormat::Toml))
            .build()
            .expect("failed to parse configuration")
            .try_deserialize::<WorkloadConfig>()
            .expect("failed to deserialize into WorkloadConfig");

        let WorkloadConfig {
            restart_interval,
            workload,
            ..
        } = config;
        assert_eq!(restart_interval, 10);

        if let Workload::Endpoints { distribution, .. } = workload {
            if let Distribution::Zipfian { n_ports, exponent } = distribution {
                assert_eq!(n_ports, 200);
                assert_eq!(exponent, 1.4);
            } else {
                panic!("wrong distribution type found");
            }
        } else {
            panic!("wrong workload type found");
        }
    }

    #[test]
    fn test_endpoints_uniform() {
        let input = r#"
            restart_interval = 10

            [workload]
            type = "endpoints"
            distribution = "uniform"
            upper = 100
            lower = 1
        "#;

        let config = Config::builder()
            .add_source(File::from_str(input, FileFormat::Toml))
            .build()
            .expect("failed to parse configuration")
            .try_deserialize::<WorkloadConfig>()
            .expect("failed to deserialize into WorkloadConfig");

        let WorkloadConfig {
            restart_interval,
            workload,
            ..
        } = config;
        assert_eq!(restart_interval, 10);

        if let Workload::Endpoints { distribution } = workload {
            if let Distribution::Uniform { lower, upper } = distribution {
                assert_eq!(lower, 1);
                assert_eq!(upper, 100);
            } else {
                panic!("wrong distribution type found");
            }
        } else {
            panic!("wrong workload type found");
        }
    }

    #[test]
    fn test_syscalls() {
        let input = r#"
            restart_interval = 10

            [workload]
            type = "syscalls"
            arrival_rate = 10.0
        "#;

        let config = Config::builder()
            .add_source(File::from_str(input, FileFormat::Toml))
            .build()
            .expect("failed to parse configuration")
            .try_deserialize::<WorkloadConfig>()
            .expect("failed to deserialize into WorkloadConfig");

        let WorkloadConfig {
            restart_interval,
            workload,
            ..
        } = config;
        assert_eq!(restart_interval, 10);
        if let Workload::Syscalls { arrival_rate } = workload {
            assert_eq!(arrival_rate, 10.0);
        } else {
            panic!("wrong workload type found");
        }
    }
}
