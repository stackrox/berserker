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
    thread,
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
            workload,
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

            // As a simplest solution to keep a connection open, spawn a
            // thread.  It's not the best one though, as we waste resources.
            // For the purpose of only keeping connections open we could e.g.
            // spawn only two threads, where the first one receives connections
            // and adds streams into the list of active, and the second iterates
            // through streams and replies. This way the connections will have
            // high latency, but for the purpose of networking workload it
            // doesn't matter.
            thread::spawn(move || loop {
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
            });
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
            arrival_rate,
            departure_rate,
            connections_static,
            connections_dyn_max,
            conns_per_addr,
            send_interval,
            preempt,
        } = self.workload.workload
        else {
            unreachable!()
        };

        debug!("Starting client, target {:?}:{:?}", addr, target_port);

        let (mut iface, mut device, fd) = self.setup_tuntap(addr);
        let cx = iface.context();

        // Dynamic sockets are going to be responsible for connections that
        // will be opened/closed during the test. Every record contains:
        // * socket handle (just an index inside smoltcp)
        // * time when the connection was opened
        // * connection lifetime
        let mut dynamic_sockets = HashMap::new();

        // Open static set of connections, that are going to live throughout
        // the whole run
        let mut sockets = SocketSet::new(vec![]);

        for _i in 0..connections_static {
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
            let index = i as u32;
            let (local_addr, local_port) =
                get_local_addr_port(addr, conns_per_addr, index);
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

        // Timer and waiting interval for the next new dynamic connection
        let mut arrivals = SystemTime::now();
        let mut interval: f64 =
            thread_rng().sample(Exp::new(arrival_rate).unwrap());

        // Current number of opened connections, both dynamic and static
        let mut total_conns = connections_static;

        // The main loop, where connection state will be updated, and dynamic
        // connections will be opened/closed
        loop {
            // Vector of sockets to close at the end of each loop
            let mut close_sockets = vec![];

            let timestamp = Instant::now();
            iface.poll(timestamp, &mut device, &mut sockets);

            let elapsed = arrivals.elapsed().unwrap().as_millis();
            if elapsed > (interval * 1000.0).round() as u128 {
                // Time for a new connection, add a socket, it state is going
                // to be updated during the next loop round
                total_conns += 1;

                let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
                let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
                let mut socket = tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer);

                let index = total_conns;
                let (local_addr, local_port) =
                    get_local_addr_port(addr, conns_per_addr, total_conns);

                let lifetime: f64 =
                    thread_rng().sample(Exp::new(departure_rate).unwrap());

                // If we've reached the connections limit
                if dynamic_sockets.len() == connections_dyn_max as usize
                    && preempt
                {
                    let idx =
                        thread_rng().gen_range(0..connections_dyn_max as usize);
                    let (key, _) = sockets.iter().nth(idx).unwrap();
                    dynamic_sockets.remove(&key);
                    close_sockets.push(key);
                }

                // either we've just removed a socket and want to preempt
                // or, we've have space and we're processing normally
                if dynamic_sockets.len() < connections_dyn_max as usize {
                    socket
                        .connect(
                            iface.context(),
                            (addr, target_port),
                            (local_addr, local_port),
                        )
                        .unwrap();

                    let handle = sockets.add(socket);
                    dynamic_sockets
                        .insert(handle, (SystemTime::now(), lifetime));
                }

                info!(
                    "New connecting from {}:{}, lifetime {}, index {}",
                    local_addr,
                    local_port,
                    lifetime,
                    index - 1
                );

                // set new interval for the next new connection
                interval = thread_rng().sample(Exp::new(arrival_rate).unwrap());
                arrivals = SystemTime::now();
            }

            // Iterate through all sockets, update the state for each one
            for (i, (h, s)) in sockets.iter_mut().enumerate() {
                let socket = tcp::Socket::downcast_mut(s)
                    .ok_or(WorkerError::Internal)?;

                info!("Process socket {}, {}", i, socket.state());
                match dynamic_sockets.get(&h) {
                    Some((timer, life)) => {
                        // A dynamic connection, verify lifetime
                        debug!("Dynamic socket {}", i);
                        if timer.elapsed().unwrap().as_millis()
                            > (life * 1000.0).round() as u128
                        {
                            info!("Close socket {}", i);
                            socket.close();
                            dynamic_sockets.remove(&h);
                            close_sockets.push(h);
                            continue;
                        }
                    }
                    None => {
                        // Static connection, continue
                        debug!("Static socket {}", i);
                    }
                }

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
                    let elapsed =
                        send_timer.elapsed().unwrap().as_millis() as u64;

                    // Throttle sending data via connection, since the main
                    // purpose is to excercise connection monitoring.
                    // Sending data too frequently we risk producing too much
                    // load and making connetion monitoring less reliable.
                    if elapsed > send_interval {
                        // reset the timer
                        send_timer = SystemTime::now();

                        let response = format!("hello {i}\n");
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

            for h in close_sockets {
                info!("Close handle {}", h);
                // TODO: reuse sockets
                sockets.remove(h);
                total_conns -= 1;
            }

            info!("Sockets: {}", total_conns);

            // We cant wait only for iface.poll_delay(timestamp, &sockets)
            // interval, since the loop could stuck without any activity
            // making no progress. To prevent that specify a minimum waiting
            // duration of 100 milliseconds.
            let min_duration = smoltcp::time::Duration::from_millis(100);

            let duration = iface
                .poll_delay(timestamp, &sockets)
                .min(Some(min_duration))
                .or(Some(min_duration));

            info!("wait duration {:?}", duration);
            phy_wait(fd, duration).expect("wait error");
        }
    }

    /// Setup a tun device for communication, wrapped into a Tracer
    /// and a FaultInjector.
    fn setup_tuntap(
        &self,
        addr: Ipv4Address,
    ) -> (Interface, FaultInjector<Tracer<TunTapInterface>>, i32) {
        let device_name = "berserker0";
        let device = TunTapInterface::new(device_name, Medium::Ip).unwrap();
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
}

/// Map socket index to a local port and address. The address octets are
/// incremented every conns_per_addr sockets, whithin this interval the local
/// port is incremented. The first port to be taken is 49152, an out of blue
/// large enough number.
///
/// E.g. if the base address is 10.0.0.1, we put 100 connections on the same
/// address and this is 10th connection. In this case we should get
/// 10.0.0.2:49162 as a result, as if we were inserting 10 new addresses
/// starting from 10.0.0.2 (remember, 10.0.0.1 is the base address and is
/// already claimed) incrementing first 100 times the port, then the address.
///
/// addr - starting point in the IP address space. The new address is going to
///         be based on it plus the connection number.
///
/// conns_per_addr - how many connections are going to share the same IP
///         address, and differ only in port value.
///
/// index - current global number of the connection.
fn get_local_addr_port(
    addr: Ipv4Address,
    conns_per_addr: u16,
    index: u32,
) -> (IpAddress, u16) {
    let local_port = 49152 + (index % conns_per_addr as u32) as u16;
    debug!("addr {}, index {}", addr, index);

    // conns_per_addr effectively groups connections together, one address per
    // group with only port being different. addr_index represent current index
    // inside the space of such groups.
    let addr_index = index / conns_per_addr as u32;
    let local_addr = Ipv4Address::from_bits(addr.to_bits() + addr_index + 1);

    (IpAddress::Ipv4(local_addr), local_port)
}

impl Worker for NetworkWorker {
    fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        let Workload::Network {
            server,
            address,
            target_port,
            ..
        } = self.workload.workload
        else {
            unreachable!()
        };

        if server {
            let _ = self.start_server(address, target_port);
        } else {
            let _ = self.start_client(address, target_port);
        }

        Ok(())
    }
}

impl Display for NetworkWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_local_addr_port() {
        let test_cases = vec![
            // (addr, conns_per_addr, index, expected_ip, expected_port)
            //
            // 10 conns per group, 15 -> second group, increment = 2
            (
                Ipv4Address::new(192, 168, 1, 100),
                10,
                15,
                IpAddress::v4(192, 168, 1, 102),
                49157,
            ),
            // 9 conns per group, 15 -> second group, increment = 2
            (
                Ipv4Address::new(192, 168, 1, 255),
                9,
                15,
                IpAddress::v4(192, 168, 2, 1),
                49158,
            ),
            // 12 conns per group, 15 -> second group, increment = 2
            (
                Ipv4Address::new(192, 255, 255, 255),
                12,
                15,
                IpAddress::v4(193, 0, 0, 1),
                49155,
            ),
            // 1 conn per group, 512 -> 512 group, increment = 512
            (
                Ipv4Address::new(192, 168, 1, 100),
                1,
                512,
                IpAddress::v4(192, 168, 3, 101),
                49152,
            ),
            // 1 conn per group, 65636 -> 65636 group, increment = 65636
            (
                Ipv4Address::new(192, 168, 1, 100),
                1,
                65636,
                IpAddress::v4(192, 169, 1, 201),
                49152,
            ),
            // 100 conn per group, 1 ->  group, increment = 1
            (
                Ipv4Address::new(10, 0, 0, 1),
                100,
                1,
                IpAddress::v4(10, 0, 0, 2),
                49153,
            ),
        ];

        for (addr, conns_per_addr, index, expected_ip, expected_port) in
            test_cases
        {
            let (ip, port) = get_local_addr_port(addr, conns_per_addr, index);
            assert_eq!(ip, expected_ip);
            assert_eq!(port, expected_port);
        }
    }
}
