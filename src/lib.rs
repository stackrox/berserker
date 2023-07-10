use serde::Deserialize;

pub mod worker;

#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(tag = "distribution")]
pub enum Distribution {
    #[serde(alias = "zipf")]
    Zipfian { n_ports: u64, exponent: f64 },
    #[serde(alias = "uniform")]
    Uniform { lower: u64, upper: u64 },
}

#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum Workload {
    Endpoints {
        #[serde(flatten)]
        distribution: Distribution,
    },
    Processes {
        arrival_rate: f64,
        departure_rate: f64,
        random_process: bool,
    },
    Syscalls {
        arrival_rate: f64,
    },
}

#[derive(Debug, Copy, Clone, Deserialize)]
pub struct WorkloadConfig {
    pub restart_interval: u64,
    pub workload: Workload,
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
        } = config;
        assert_eq!(restart_interval, 10);

        if let Workload::Endpoints { distribution } = workload {
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
        } = config;
        assert_eq!(restart_interval, 10);
        if let Workload::Syscalls { arrival_rate } = workload {
            assert_eq!(arrival_rate, 10.0);
        } else {
            panic!("wrong workload type found");
        }
    }
}
