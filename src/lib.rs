use core_affinity::CoreId;
use serde::Deserialize;
use std::fmt::Display;

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
}

fn default_workers() -> usize {
    1
}

fn default_per_core() -> bool {
    true
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
pub enum WorkerError {}

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
        if let Workload::Syscalls { arrival_rate } = workload {
            assert_eq!(arrival_rate, 10.0);
        } else {
            panic!("wrong workload type found");
        }
    }
}
