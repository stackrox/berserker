use std::{fmt::Display, net::TcpListener, ops::Range, thread, time};

use log::info;

use crate::{BaseConfig, WorkerError};

struct PortRange {
    start: u16,
    length: u16,
}

impl PortRange {
    fn new(start: u16, length: u16) -> Self {
        PortRange { start, length }
    }

    fn get_range(&self) -> Range<u16> {
        let end = self.start + self.length;
        self.start..end
    }
}

pub struct EndpointWorker {
    config: BaseConfig,
    restart_interval: u64,
    ports: PortRange,
}

impl EndpointWorker {
    pub fn new(
        config: BaseConfig,
        restart_interval: u64,
        start_port: u16,
        n_ports: u16,
    ) -> Self {
        let ports = PortRange::new(start_port, n_ports);

        EndpointWorker {
            config,
            restart_interval,
            ports,
        }
    }

    pub fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        // Copy the u64 to prevent moving self into the thread.
        let restart_interval = self.restart_interval;
        let listeners: Vec<_> = self
            .ports
            .get_range()
            .map(|port| thread::spawn(move || listen(port, restart_interval)))
            .collect();

        for listener in listeners {
            let _res = listener.join().unwrap();
        }

        Ok(())
    }

    pub fn size(&self) -> u16 {
        self.ports.length
    }
}

impl Display for EndpointWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}

fn listen(port: u16, sleep: u64) -> std::io::Result<()> {
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(addr)?;

    let _res = listener.incoming();

    thread::sleep(time::Duration::from_secs(sleep));
    Ok(())
}
