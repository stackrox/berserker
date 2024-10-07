use serde::Deserialize;

use self::{endpoints::Endpoints, network::Network};

pub(crate) mod endpoints;
pub(crate) mod network;

/// Main workload configuration, contains general bits for all types of
/// workloads plus workload specific data.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkloadConfig {
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
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum Workload {
    /// How to listen on ports.
    Endpoints(Endpoints),

    /// How to spawn processes.
    Processes(Processes),

    /// How to invoke syscalls
    Syscalls(Syscalls),

    /// How to open network connections
    Network(Network),
}

#[derive(Debug, Clone, Deserialize)]
pub struct Processes {
    /// How often a new process will be spawn.
    pub arrival_rate: f64,

    /// How long processes are going to live.
    pub departure_rate: f64,

    /// Spawn a new process with random arguments.
    pub random_process: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Syscalls {
    /// How often to invoke a syscall.
    pub arrival_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::{Config, File, FileFormat};

    #[test]
    fn test_processes() {
        let input = r#"
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

        let WorkloadConfig { workload, .. } = config;
        if let Workload::Processes(Processes {
            arrival_rate,
            departure_rate,
            random_process,
            ..
        }) = workload
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
            [workload]
            type = "endpoints"
            restart_interval = 10
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

        let WorkloadConfig { workload, .. } = config;

        if let Workload::Endpoints(Endpoints {
            restart_interval,
            distribution,
        }) = workload
        {
            assert_eq!(restart_interval, 10);

            if let endpoints::Distribution::Zipfian { n_ports, exponent } =
                distribution
            {
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
            [workload]
            type = "endpoints"
            restart_interval = 10
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

        let WorkloadConfig { workload, .. } = config;

        if let Workload::Endpoints(Endpoints {
            restart_interval,
            distribution,
        }) = workload
        {
            assert_eq!(restart_interval, 10);
            if let endpoints::Distribution::Uniform { lower, upper } =
                distribution
            {
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

        let WorkloadConfig { workload, .. } = config;
        if let Workload::Syscalls(Syscalls { arrival_rate, .. }) = workload {
            assert_eq!(arrival_rate, 10.0);
        } else {
            panic!("wrong workload type found");
        }
    }
}
