use std::{net::TcpListener, thread, time};

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
        let BaseConfig {
            cpu,
            process,
            lower,
            upper,
        } = self.config;
        info!("Process {} from {}: {}-{}", process, cpu.id, lower, upper);

        let restart_interval = self.workload.restart_interval;

        let listeners: Vec<_> = (lower..upper)
            .map(|port| thread::spawn(move || listen(port, restart_interval)))
            .collect();

        for listener in listeners {
            let _res = listener.join().unwrap();
        }

        Ok(())
    }
}

fn listen(port: usize, sleep: u64) -> std::io::Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(addr)?;

    let _res = listener.incoming();

    thread::sleep(time::Duration::from_secs(sleep));
    Ok(())
}
