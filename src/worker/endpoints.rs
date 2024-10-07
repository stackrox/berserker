use std::{fmt::Display, net::TcpListener, thread, time};

use core_affinity::CoreId;
use log::info;

use crate::{BaseConfig, WorkerError, WorkloadConfig};

pub struct EndpointWorker {
    config: BaseConfig,
    restart_interval: u64,
    lower: usize,
    upper: usize,
}

impl EndpointWorker {
    pub fn new(
        workload: WorkloadConfig,
        cpu: CoreId,
        process: usize,
        lower: usize,
        upper: usize,
    ) -> Self {
        let WorkloadConfig {
            restart_interval,
            workload: _,
            per_core: _,
            workers: _,
            duration: _,
        } = workload;

        EndpointWorker {
            config: BaseConfig { cpu, process },
            restart_interval,
            lower,
            upper,
        }
    }

    pub fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        // Copy the u64 to prevent moving self into the thread.
        let restart_interval = self.restart_interval;
        let listeners: Vec<_> = (self.lower..self.upper)
            .map(|port| thread::spawn(move || listen(port, restart_interval)))
            .collect();

        for listener in listeners {
            let _res = listener.join().unwrap();
        }

        Ok(())
    }
}

impl Display for EndpointWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}

fn listen(port: usize, sleep: u64) -> std::io::Result<()> {
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(addr)?;

    let _res = listener.incoming();

    thread::sleep(time::Duration::from_secs(sleep));
    Ok(())
}
