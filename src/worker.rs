use std::{io::Result, net::TcpListener, process::Command, thread, time};

use core_affinity::CoreId;
use fork::{fork, Fork};
use log::{info, warn};
use nix::{sys::wait::waitpid, unistd::Pid};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rand_distr::Exp;
use syscalls::{syscall, Sysno};

use crate::WorkloadConfig;

#[derive(Debug, Copy, Clone)]
pub struct WorkerConfig {
    workload: WorkloadConfig,
    cpu: CoreId,
    process: usize,
    lower: usize,
    upper: usize,
}

impl WorkerConfig {
    pub fn new(
        workload: WorkloadConfig,
        cpu: CoreId,
        process: usize,
        lower: usize,
        upper: usize,
    ) -> Self {
        WorkerConfig {
            workload,
            cpu,
            process,
            lower,
            upper,
        }
    }

    pub fn spawn_process(self, lifetime: u64) -> Result<()> {
        if self.workload.random_process {
            let uniq_arg: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(7)
                .map(char::from)
                .collect();
            let _res = Command::new("stub").arg(uniq_arg).output().unwrap();
            Ok(())
        } else {
            match fork() {
                Ok(Fork::Parent(child)) => {
                    info!("Parent: child {}", child);
                    waitpid(Pid::from_raw(child), None).unwrap();
                    Ok(())
                }
                Ok(Fork::Child) => {
                    info!(
                        "{}-{}: Child start, {}",
                        self.cpu.id, self.process, lifetime
                    );
                    thread::sleep(time::Duration::from_millis(lifetime));
                    info!("{}-{}: Child stop", self.cpu.id, self.process);
                    Ok(())
                }
                Err(_) => {
                    warn!("Failed");
                    Ok(())
                }
            }
        }
    }

    // Spawn processes with a specified rate
    pub fn process_payload(self) -> std::io::Result<()> {
        info!(
            "Process {} from {}: {}-{}",
            self.process, self.cpu.id, self.lower, self.upper
        );

        loop {
            let lifetime: f64 =
                thread_rng().sample(Exp::new(self.workload.departure_rate).unwrap());

            thread::spawn(move || self.spawn_process((lifetime * 1000.0).round() as u64));

            let interval: f64 = thread_rng().sample(Exp::new(self.workload.arrival_rate).unwrap());
            info!(
                "{}-{}: Interval {}, rounded {}, lifetime {}, rounded {}",
                self.cpu.id,
                self.process,
                interval,
                (interval * 1000.0).round() as u64,
                lifetime,
                (lifetime * 1000.0).round() as u64
            );
            thread::sleep(time::Duration::from_millis(
                (interval * 1000.0).round() as u64
            ));
            info!("{}-{}: Continue", self.cpu.id, self.process);
        }
    }

    pub fn listen_payload(self) -> std::io::Result<()> {
        info!(
            "Process {} from {}: {}-{}",
            self.process, self.cpu.id, self.lower, self.upper
        );

        let listeners: Vec<_> = (self.lower..self.upper)
            .map(|port| thread::spawn(move || listen(port, self.workload.restart_interval)))
            .collect();

        for listener in listeners {
            let _res = listener.join().unwrap();
        }

        Ok(())
    }

    pub fn syscalls_payload(self) -> Result<()> {
        info!(
            "Process {} from {}: {}-{}",
            self.process, self.cpu.id, self.lower, self.upper
        );

        loop {
            thread::spawn(move || {
                self.do_syscall().unwrap();

                let interval: f64 =
                    thread_rng().sample(Exp::new(self.workload.arrival_rate).unwrap());
                info!(
                    "{}-{}: Interval {}, rounded {}",
                    self.cpu.id,
                    self.process,
                    interval,
                    (interval * 1000.0).round() as u64
                );
                thread::sleep(time::Duration::from_millis(
                    (interval * 1000.0).round() as u64
                ));
                info!("{}-{}: Continue", self.cpu.id, self.process);
            });
        }
    }

    fn do_syscall(self) -> std::io::Result<()> {
        match unsafe { syscall!(Sysno::getpid) } {
            Ok(_) => Ok(()),
            Err(err) => {
                warn!("Syscall failed: {}", err);
                Ok(())
            }
        }
    }
}

fn listen(port: usize, sleep: u64) -> std::io::Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(addr)?;

    let _res = listener.incoming();

    thread::sleep(time::Duration::from_secs(sleep));
    Ok(())
}
