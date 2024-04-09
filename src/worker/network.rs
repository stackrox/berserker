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

        Ok(())
    }
}

impl Display for NetworkWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}
