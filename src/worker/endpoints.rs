use std::{fmt::Display, net::TcpListener, thread, time};

use core_affinity::CoreId;
use log::info;

use crate::WorkloadConfig;

use super::{BaseConfig, Worker, WorkerError};

pub struct EndpointWorker {
    config: BaseConfig,
    workload: WorkloadConfig,
}

impl EndpointWorker {
    pub fn new(
        workload: WorkloadConfig,
        cpu: CoreId,
        process: usize,
        lower: usize,
        upper: usize,
    ) -> Self {
        EndpointWorker {
            config: BaseConfig {
                cpu,
                process,
                lower,
                upper,
            },
            workload,
        }
    }
}

impl Worker for EndpointWorker {
    fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        let restart_interval = self.workload.restart_interval;

        let listeners: Vec<_> = (self.config.lower..self.config.upper)
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
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(addr)?;

    let _res = listener.incoming();

    thread::sleep(time::Duration::from_secs(sleep));
    Ok(())
}
