use std::{
    fmt::Display,
    io::{prelude::*, BufReader},
    net::{TcpListener},
};

use core_affinity::CoreId;
use log::{trace, info, debug};
use std::time::{SystemTime, UNIX_EPOCH};
use std::os::unix::io::AsRawFd;
use std::str::{self, FromStr};

use crate::{BaseConfig, Worker, WorkerError, Workload, WorkloadConfig};

use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::phy::{
    wait as phy_wait, Device, Medium, FaultInjector, Tracer, TunTapInterface
};
use smoltcp::socket::tcp;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address};
use smoltcp::socket::AnySocket;

pub struct NetworkWorker {
    config: BaseConfig,
    workload: WorkloadConfig,
}

impl NetworkWorker {
    pub fn new(workload: WorkloadConfig, cpu: CoreId, process: usize) -> Self {
        let WorkloadConfig {
            restart_interval,
            workload: _,
        } = workload;

        NetworkWorker {
            config: BaseConfig { cpu, process },
            workload: workload,
        }
    }

    fn start_server(&self) -> Result<(), WorkerError> {
        let listener = TcpListener::bind(("192.168.0.1", 8080)).unwrap();

        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut buf_reader = BufReader::new(&stream);
            let mut buffer = Vec::new();
            buf_reader.read_to_end(&mut buffer).unwrap();
            trace!("Received {:?}", buffer);

            let response = "hello";
            stream.write_all(response.as_bytes()).unwrap();
        }

        Ok(())
    }

    fn start_client(&self) -> Result<(), WorkerError> {
        let tap = "tap0";
        let device = TunTapInterface::new(&tap, Medium::Ethernet).unwrap();
        let fd = device.as_raw_fd();

        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();

        let device = Tracer::new(device, |_timestamp, _printer| {
            trace!("{}", _printer);
        });

        let mut device = FaultInjector::new(device, seed);
        let address = IpAddress::from_str("192.168.0.1").expect("invalid address format");
        let nr_sockets = 10;

        // Create interface
        let mut config = match device.capabilities().medium {
            Medium::Ethernet => {
                Config::new(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]).into())
            }
            Medium::Ip => Config::new(smoltcp::wire::HardwareAddress::Ip),
            Medium::Ieee802154 => todo!(),
        };
        config.random_seed = rand::random();

        let mut iface = Interface::new(config, &mut device, Instant::now());
        iface.set_any_ip(true);
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs
                .push(IpCidr::new(IpAddress::v4(192, 168, 0, 1), 16))
                .unwrap();
        });

        iface
            .routes_mut()
            .add_default_ipv4_route(Ipv4Address::new(192, 168, 0, 1))
            .unwrap();

        let mut sockets = SocketSet::new(vec![]);

        for i in 0..nr_sockets {
            // Create sockets
            let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
            let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
            let tcp_socket = tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer);

            let tcp_handle = sockets.add(tcp_socket);
        }

        let timestamp = Instant::now();
        iface.poll(timestamp, &mut device, &mut sockets);
        let cx = iface.context();

        for (i, socket) in sockets.iter_mut().filter_map(|(h, s)| tcp::Socket::downcast_mut(s)).enumerate() {
            let index = i + 2;
            let local_port = 49152 + rand::random::<u16>() % 16384;
            let local_addr = IpAddress::v4(192, 168, (index / 255) as u8, (index % 255) as u8);
            info!("connecting from {}", local_addr);
            socket
                .connect(cx,
                         (address, 8080),
                         (local_addr, local_port))
                .unwrap();
        }

        loop {
            let timestamp = Instant::now();
            iface.poll(timestamp, &mut device, &mut sockets);

            for (i, socket) in sockets.iter_mut().filter_map(|(h, s)| tcp::Socket::downcast_mut(s)).enumerate() {
                if socket.can_recv() {
                    socket
                        .recv(|data| {
                            println!("{}", str::from_utf8(data).unwrap_or("(invalid utf8)"));
                            (data.len(), ())
                        })
                        .unwrap();
                }

                if socket.may_send() {
                    info!("sending request from {}",
                           socket.local_endpoint().unwrap().addr);
                    socket
                        .send_slice(b"hello")
                        .expect("cannot send");
                }
            }

            phy_wait(fd, iface.poll_delay(timestamp, &sockets)).expect("wait error");
        }

        Ok(())
    }
}

impl Worker for NetworkWorker {
    fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        let Workload::Network {
            server,
        } = self.workload.workload
        else {
            unreachable!()
        };

        if server {
            self.start_server();
        } else {
            self.start_client();
        }

        Ok(())
    }
}

impl Display for NetworkWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}
