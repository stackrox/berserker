use std::fmt::Display;

use core_affinity::CoreId;
use log::info;

use crate::{BaseConfig, Worker, WorkerError, WorkloadConfig};

struct TemplateWorkload {
    restart_interval: u64,
}

pub struct TemplateWorker {
    config: BaseConfig,
    workload: TemplateWorkload,
}

impl TemplateWorker {
    pub fn new(workload: WorkloadConfig, cpu: CoreId, process: usize) -> Self {
        let WorkloadConfig {
            restart_interval,
            workload: _,
        } = workload;

        TemplateWorker {
            config: BaseConfig { cpu, process },
            workload: TemplateWorkload { restart_interval },
        }
    }
}

impl Worker for TemplateWorker {
    fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        let TemplateWorkload { restart_interval } = self.workload;

        // Do something here
        info!("{restart_interval}");

        Ok(())
    }
}

impl Display for TemplateWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}
