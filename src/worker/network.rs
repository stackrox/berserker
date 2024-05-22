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
        debug!("Starting server at {:?}:{:?}", addr, target_port);

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

    fn start_client(
        &self,
        addr: Ipv4Address,
        target_port: u16,
    ) -> Result<(), WorkerError> {
        let Workload::Network {
            server: _,
            address: _,
            target_port: _,
            arrival_rate: _,
            departure_rate: _,
            nconnections,
            send_interval,
        } = self.workload.workload
        else {
            unreachable!()
        };

        debug!("Starting client at {:?}:{:?}", addr, target_port);

        let (mut iface, mut device, fd) = self.setup_tuntap(addr);
        let cx = iface.context();

        // Open static set of connections, that are going to live throughout
        // the whole run
        let mut sockets = SocketSet::new(vec![]);

        for _i in 0..nconnections {
            let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
            let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
            let tcp_socket = tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer);

            sockets.add(tcp_socket);
        }

        for (i, socket) in sockets
            .iter_mut()
            .filter_map(|(_h, s)| tcp::Socket::downcast_mut(s))
            .enumerate()
        {
            let index = i;
            let (local_addr, local_port) =
                self.get_local_addr_port(addr, index);
            info!("connecting from {}:{}", local_addr, local_port);
            socket
                .connect(cx, (addr, target_port), (local_addr, local_port))
                .unwrap();
        }

        // Use global timer to throttle sending the data. It means there will
        // be some irregularity about data sending betwen various connections,
        // but to make it more precise we need to bookkeeping for every
        // connection, which may waste memory and introduce unstability on its
        // own.
        let mut send_timer = SystemTime::now();

        // The main loop, where connection state will be updated, and dynamic
        // connections will be opened/closed
        loop {
            let timestamp = Instant::now();
            iface.poll(timestamp, &mut device, &mut sockets);

            // Iterate through all sockets, update the state for each one
            for (i, (h, s)) in sockets.iter_mut().enumerate() {
                let socket = tcp::Socket::downcast_mut(s)
                    .ok_or(WorkerError::Internal)?;

                info!("Process socket {}, {}", i, socket.state());
                if socket.can_recv() {
                    socket
                        .recv(|data| {
                            trace!(
                                "{}",
                                str::from_utf8(data)
                                    .unwrap_or("(invalid utf8)")
                            );
                            (data.len(), ())
                        })
                        .unwrap();
                }

                if socket.may_send() {
                    let elapsed = send_timer.elapsed().unwrap().as_millis();

                    // Throttle sending data via connection, since the main
                    // purpose is to excercise connection monitoring.
                    // Sending data too frequently we risk producing too much
                    // load and making connetion monitoring less reliable.
                    if elapsed > send_interval {
                        // reset the timer
                        send_timer = SystemTime::now();

                        let response = format!("hello {}\n", i);
                        let binary = response.as_bytes();
                        trace!(
                            "sending request from idx {} addr {}, data {:?}",
                            i,
                            socket.local_endpoint().unwrap().addr,
                            binary
                        );
                        socket.send_slice(binary).expect("cannot send");
                    }
                }
            }

            // We cant wait only for iface.poll_delay(timestamp, &sockets)
            // interval, since the loop could stuck without any activity
            // making no progress. To prevent that specify a minimum waiting
            // duration of 100 milliseconds.
            let duration = iface
                .poll_delay(timestamp, &sockets)
                .min(Some(smoltcp::time::Duration::from_millis(100)));

            phy_wait(fd, duration).expect("wait error");
        }
    }

    /// Setup a tun device for communication, wrapped into a Tracer
    /// and a FaultInjector.
    fn setup_tuntap(
        &self,
        addr: Ipv4Address,
    ) -> (Interface, FaultInjector<Tracer<TunTapInterface>>, i32) {
        let device_name = "tun0";
        let device = TunTapInterface::new(&device_name, Medium::Ip).unwrap();
        let fd = device.as_raw_fd();

        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();

        let device = Tracer::new(device, |_timestamp, printer| {
            trace!("{}", printer);
        });

        let mut device = FaultInjector::new(device, seed);

        // Create interface
        let mut config = match device.capabilities().medium {
            Medium::Ethernet => Config::new(
                EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]).into(),
            ),
            Medium::Ip => Config::new(smoltcp::wire::HardwareAddress::Ip),
            Medium::Ieee802154 => todo!(),
        };
        config.random_seed = rand::random();

        let mut iface = Interface::new(config, &mut device, Instant::now());
        iface.set_any_ip(true);
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs
                .push(IpCidr::new(IpAddress::Ipv4(addr), 16))
                .unwrap();
        });

        iface.routes_mut().add_default_ipv4_route(addr).unwrap();

        (iface, device, fd)
    }

    /// Map socket index to a local port and address. The address octets are
    /// incremented every 100 sockets, whithin this interval the local port
    /// is incremented.
    fn get_local_addr_port(
        &self,
        addr: Ipv4Address,
        index: usize,
    ) -> (IpAddress, u16) {
        // 254 (a2 octet) * 254 (a3 octet) * 100 (port)
        // gives us maximum 6451600 connections that could be opened
        let local_port = 49152 + (index % 100) as u16;
        debug!("addr {}, index {}", addr, index);

        let local_addr = IpAddress::v4(
            addr.0[0],
            addr.0[1],
            (((index / 100) + 2) / 255) as u8,
            (((index / 100) + 2) % 255) as u8,
        );

        return (local_addr, local_port);
    }
}

impl Worker for NetworkWorker {
    fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        let Workload::Network {
            server,
            address,
            target_port,
            arrival_rate: _,
            departure_rate: _,
            nconnections: _,
            send_interval: _,
        } = self.workload.workload
        else {
            unreachable!()
        };

        let ip_addr = Ipv4Address([address.0, address.1, address.2, address.3]);

        if server {
            let _ = self.start_server(ip_addr, target_port);
        } else {
            let _ = self.start_client(ip_addr, target_port);
        }

        Ok(())
    }
}

impl Display for NetworkWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}
