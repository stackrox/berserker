use std::fmt::Display;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Endpoints {
    /// An amount of time for the workload to run before restarting
    pub restart_interval: u64,

    /// Governing the number of ports open.
    #[serde(flatten)]
    pub distribution: Distribution,
}

impl Display for Endpoints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Using {} distribution", self.distribution)
    }
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

impl Display for Distribution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
