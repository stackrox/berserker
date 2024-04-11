use core_affinity::CoreId;
use log::{debug, info, trace};
use rand::{thread_rng, Rng};
use rand_distr::Exp;
use std::collections::HashMap;
use std::os::unix::io::AsRawFd;
use std::str;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    fmt::Display,
    io::{prelude::*, BufReader},
    net::TcpListener,
};

use crate::{BaseConfig, Worker, WorkerError, Workload, WorkloadConfig};

use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::phy::{
    wait as phy_wait, Device, FaultInjector, Medium, Tracer, TunTapInterface,
};
use smoltcp::socket::tcp;
use smoltcp::socket::AnySocket;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address};

pub struct NetworkWorker {
    config: BaseConfig,
    workload: WorkloadConfig,
}

impl NetworkWorker {
    pub fn new(workload: WorkloadConfig, cpu: CoreId, process: usize) -> Self {
        NetworkWorker {
            config: BaseConfig { cpu, process },
            workload: workload,
        }
    }

    /// Start a simple server. The client side is going to be a networking
    /// worker as well, so for convenience of troubleshooting do not error
    /// out if something unexpected happened, log and proceed instead.
    fn start_server(
        &self,
        addr: Ipv4Address,
        target_port: u16,
    ) -> Result<(), WorkerError> {
        let listener =
            TcpListener::bind((addr.to_string(), target_port)).unwrap();

        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            loop {
                let mut buf_reader = BufReader::new(&stream);
                let mut buffer = String::new();

                match buf_reader.read_line(&mut buffer) {
                    Ok(0) => {
                        // EOF, exit
                        break;
                    }
                    Ok(_n) => {
                        trace!("Received {:?}", buffer);

                        let response = "hello\n";
                        match stream.write_all(response.as_bytes()) {
                            Ok(_) => {
                                // Response is sent, handle the next one
                                break;
                            }
                            Err(e) => {
                                trace!("ERROR: sending response, {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        trace!("ERROR: reading a line, {}", e)
                    }
                }
            }
        }

        Ok(())
    }
}

impl Worker for NetworkWorker {
    fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        let Workload::Network {
            server,
            address,
            target_port,
            arrival_rate,
            departure_rate,
            nconnections,
        } = self.workload.workload
        else {
            unreachable!()
        };

        let ip_addr = Ipv4Address([address.0, address.1, address.2, address.3]);

        if server {
            let _ = self.start_server(ip_addr, target_port);
        }

        Ok(())
    }
}

impl Display for NetworkWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}
