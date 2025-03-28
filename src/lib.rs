use core_affinity::CoreId;
use serde::Deserialize;
use std::fmt::Display;
use syscalls::Sysno;

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

fn default_syscalls_arrival_rate() -> f64 {
    0.0
}

fn default_syscalls_tight_loop() -> bool {
    false
}

fn default_syscalls_syscall_nr() -> u32 {
    Sysno::getpid as u32
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
        #[serde(default = "default_syscalls_arrival_rate")]
        arrival_rate: f64,

        /// Run in a tight loop
        #[serde(default = "default_syscalls_tight_loop")]
        tight_loop: bool,

        /// Which syscall to trigger
        #[serde(default = "default_syscalls_syscall_nr")]
        syscall_nr: u32,
    },

    /// How to open network connections
    Network {
        /// Whether the instance functions as a server or client
        server: bool,

        /// Which ip address to use for the server to listen on,
        /// or for the client to connect to
        #[serde(deserialize_with = "parse_address")]
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

    /// How to load bpf progs.
    Bpf {
        /// Which tracepoint BPF programs will be attached to. Could be taken
        /// from the tracefs, e.g.
        /// /sys/kernel/debug/tracing/events/sched/sched_process_exit/id
        #[serde(default = "default_bpf_tracepoint")]
        tracepoint: u64,

        /// Number of BPF programs to launch
        #[serde(default = "default_bpf_nprogs")]
        nprogs: u64,
    },
}

fn default_bpf_tracepoint() -> u64 {
    306
}

fn default_bpf_nprogs() -> u64 {
    100
}

fn parse_address<'de, D>(deserializer: D) -> Result<(u8, u8, u8, u8), D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum AddressInput {
        Tuple((u8, u8, u8, u8)),
        Array([u8; 4]),
        Str(String),
    }

    let input = AddressInput::deserialize(deserializer)?;

    match input {
        AddressInput::Tuple(t) => Ok(t),
        AddressInput::Array(a) => Ok((a[0], a[1], a[2], a[3])),
        AddressInput::Str(s) => {
            let parts: Vec<u8> = s
                .trim_matches(|c: char| c == '[' || c == ']' || c.is_whitespace())
                .split(',')
                .map(|x| x.trim().parse::<u8>())
                .collect::<Result<_, _>>()
                .map_err(D::Error::custom)?;

            if parts.len() != 4 {
                return Err(D::Error::custom("IP address should have 4 parts"));
            }

            Ok((parts[0], parts[1], parts[2], parts[3]))
        }
    }
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

/// Generic interface for workers of any type
pub trait Worker {
    fn run_payload(&self) -> Result<(), WorkerError>;
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
        if let Workload::Syscalls {
            arrival_rate,
            tight_loop,
            syscall_nr,
        } = workload
        {
            assert_eq!(arrival_rate, 10.0);
        } else {
            panic!("wrong workload type found");
        }
    }
}
