pub mod worker;

#[derive(Debug, Copy, Clone)]
pub enum Distribution {
    Zipfian,
    Uniform,
}

#[derive(Debug, Copy, Clone)]
pub enum Workload {
    Endpoints,
    Processes,
    Syscalls,
}

#[derive(Debug, Copy, Clone)]
pub struct WorkloadConfig {
    pub restart_interval: u64,
    pub endpoints_dist: Distribution,
    pub workload: Workload,
    pub zipf_exponent: f64,
    pub n_ports: u64,
    pub uniform_lower: u64,
    pub uniform_upper: u64,
    pub arrival_rate: f64,
    pub departure_rate: f64,
    pub random_process: bool,
}
